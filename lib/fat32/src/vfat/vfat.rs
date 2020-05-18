use core::fmt::Debug;
use core::marker::PhantomData;
use core::mem::size_of;

use alloc::vec::Vec;

use shim::io;
use shim::path;
use shim::path::Path;

use crate::mbr::{MasterBootRecord, PartitionEntry};
use crate::traits::{BlockDevice, FileSystem};
use crate::traits::Dir as DirTrait;
use crate::vfat::{BiosParameterBlock, CachedPartition, Partition};
use crate::vfat::{Cluster, Dir, Entry, Error, FatEntry, File, Status};

/// A generic trait that handles a critical section as a closure
pub trait VFatHandle: Clone + Debug + Send + Sync {
    fn new(val: VFat<Self>) -> Self;
    fn lock<R>(&self, f: impl FnOnce(&mut VFat<Self>) -> R) -> R;
}

#[derive(Debug)]
pub struct VFat<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    device: CachedPartition,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    sectors_per_fat: u32,
    fat_start_sector: u64,
    data_start_sector: u64,
    rootdir_cluster: Cluster,
    cluster_size: u64,
    total_fat_sectors: u64,
}

impl<HANDLE: VFatHandle> VFat<HANDLE> {
    pub fn from<T>(mut device: T) -> Result<HANDLE, Error>
        where
            T: BlockDevice + 'static,
    {
        let mbr: MasterBootRecord = MasterBootRecord::from(&mut device)?;

        // Select first entry as vfat entry
        let mut vfat_entry: Option<&PartitionEntry> = None;
        for entry in mbr.entries.iter() {
            if entry.partition_type == 0xB || entry.partition_type == 0xC {
                vfat_entry = Some(&entry);
                break;
            }
        }

        if vfat_entry.is_none() {
            panic!("Didn't find vfat entry!");
        }

        let vfat_entry: &PartitionEntry = vfat_entry.unwrap();
        let fat_base: u64 = vfat_entry.relative_sector as u64;

        let ebpb: BiosParameterBlock = BiosParameterBlock::from(&mut device, fat_base)?;

        let fat_start_sector = ebpb.num_reserved_sectors as u64;

        let total_fat_sectors: u64 = ebpb.sectors_per_fat as u64 * ebpb.num_fats as u64;
        let data_start_sector: u64 = fat_start_sector + total_fat_sectors;

        // TODO logical num sectors could be wrong
        let partition: Partition = Partition {
            start: fat_base, // TODO wrong?
            num_sectors: ebpb.num_logical_sectors as u64,
            sector_size: ebpb.bytes_per_sector as u64,
        };

        let vfat = VFat {
            phantom: PhantomData,
            device: CachedPartition::new(device, partition),
            bytes_per_sector: ebpb.bytes_per_sector,
            sectors_per_cluster: ebpb.sectors_per_cluster,
            sectors_per_fat: ebpb.sectors_per_fat,
            fat_start_sector: fat_start_sector,
            data_start_sector: data_start_sector,
            rootdir_cluster: Cluster::from(ebpb.root_cluster_num),
            cluster_size: ebpb.bytes_per_sector as u64 * ebpb.sectors_per_cluster as u64,
            total_fat_sectors: total_fat_sectors,
        };

        return Ok(VFatHandle::new(vfat));
    }

    /// A method to read from an offset of a cluster into a buffer.
    pub fn read_cluster(&mut self, cluster: Cluster, offset: usize, buf: &mut [u8]) -> io::Result<usize> {
        let rem_cluster_size = self.cluster_size as usize - offset;
        let max_size: usize = if buf.len() > rem_cluster_size { rem_cluster_size } else { buf.len() };

        let sector_index = offset / self.bytes_per_sector() as usize;
        let mut sector_offset = offset % self.bytes_per_sector() as usize;

        let mut curr_sector = self.data_start_sector + (cluster.index() * self.sectors_per_cluster as u64) + sector_index as u64;

        let mut total_size = 0;
        while total_size < max_size {
            let content = self.device.get(curr_sector)?;

            let left_in_sector = self.bytes_per_sector as usize - sector_offset;
            let size = if buf.len() - total_size > left_in_sector {
                left_in_sector
            } else {
                buf.len() - total_size
            };

            buf[total_size..total_size + size as usize].copy_from_slice(&content[sector_offset..sector_offset + size]);

            // Only offset on first copy
            if total_size == 0 {
                sector_offset = 0;
            }

            total_size += size;
            curr_sector += 1;
        }

        return Ok(total_size);
    }

    /// A method to read all of the clusters chained from a starting cluster
    /// into a vector.
    pub fn read_chain(&mut self, start: Cluster, buf: &mut Vec<u8>) -> io::Result<usize> {
        let mut curr = start;

        let mut total_size = 0;
        loop {
            buf.resize(total_size + self.cluster_size as usize, 0);

            let sector_size = self.read_cluster(curr, 0, &mut buf[total_size..])?;

            total_size += sector_size;

            let next = self.next_cluster(curr);
            match next {
                Some(next) => {
                    curr = next;
                },
                None => break,
            }
        }

        return Ok(total_size);
    }

    /// A method to return a reference to a `FatEntry` for a cluster where the
    /// reference points directly into a cached sector.
    fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let entries_per_fat = self.bytes_per_sector as u64 / size_of::<FatEntry>() as u64;
        let fat_index = cluster.num() / entries_per_fat;
        let fat_offset = cluster.num() % entries_per_fat;

        let data = self.device.get(self.fat_start_sector + fat_index)?;
        let entries: &[FatEntry] = unsafe { core::mem::transmute(data) };

        return Ok(&entries[fat_offset as usize]);
    }

    /// Get the next cluster for a cluster, returns None if EOC
    pub fn next_cluster(&mut self, cluster: Cluster) -> Option<Cluster> {
        let fat_entry = self.fat_entry(cluster).expect("Expected valid fat entry");

        match fat_entry.status() {
            Status::Data(next) => return Some(next),
            Status::Eoc(_n) => return None,
            _ => return None,
        }
    }

    /// Getter for number of bytes per sector
    pub fn bytes_per_sector(&self) -> u64 {
        return self.bytes_per_sector as u64;
    }

    /// Getter for cluster size
    pub fn cluster_size(&self) -> u64 {
        return self.cluster_size as u64;
    }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = File<HANDLE>;
    type Entry = Entry<HANDLE>;
    type Dir = Dir<HANDLE>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        let rootdir_cluster: Cluster = self.lock(|vfat| vfat.rootdir_cluster);
        let rootdir: Dir<HANDLE> = Dir::root(self.clone(), rootdir_cluster);

        let path = path.as_ref();
        let mut components = path.components();

        let start = match components.next() {
            Some(path::Component::RootDir) => Entry::EntryDir(rootdir),
            Some(_) => panic!("Must start at root!"),
            None => panic!("Path cannot be empty!"),
        };

        let mut curr_path = components.next();
        let mut curr_entry: Entry<HANDLE> = start;

        while curr_path != None {
            match &curr_entry {
                Entry::EntryDir(dir) => {
                    // Handle dir, all good
                    let next = dir.find(curr_path.unwrap().as_os_str())?;
                    curr_entry = next;
                },
                Entry::EntryFile(file) => {
                    return Err(io::Error::new(io::ErrorKind::NotFound, "File not found, file in path"));
                }
            }
            curr_path = components.next();
        }

        return Ok(curr_entry);
    }
}
