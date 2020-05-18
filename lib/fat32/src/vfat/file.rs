use alloc::string::String;

use shim::io::{self, SeekFrom, Error, ErrorKind};

use crate::traits;
use crate::vfat::{VFat, Cluster, Metadata, VFatHandle};

#[derive(Debug)]
pub struct File<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub metadata: Metadata,
    starting_cluster: Cluster,
    curr_cluster: Option<Cluster>,
    curr_offset: u64,
    pub size: u64,
    pub name: String,
}

impl<HANDLE: VFatHandle> File<HANDLE> {
    pub fn new(vfat: HANDLE, metadata: Metadata, starting_cluster: Cluster, size: u64, name: String) -> File<HANDLE> {
        return File {
            vfat,
            metadata,
            starting_cluster,
            curr_cluster: Some(starting_cluster),
            curr_offset: 0,
            size,
            name,
        };
    }
}

// FIXME: Implement `traits::File` (and its supertraits) for `File`.
impl<HANDLE: VFatHandle> io::Read for File<HANDLE> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let cluster_size = self.vfat.lock(|vfat| { vfat.cluster_size() });

        let remaining = self.size - self.curr_offset;

        // Min of remaining and buf length
        let max_size: u64 = if remaining > buf.len() as u64 { buf.len() as u64 } else { remaining };

        // Store bytes per sector
        let bytes_per_sector = self.vfat.lock(|vfat| vfat.bytes_per_sector());

        let mut total_size: u64 = 0;
        while total_size < max_size {
            if self.curr_cluster.is_none() {
                break;
            }

            let curr_cluster = self.curr_cluster.unwrap();

            let offset = self.curr_offset % cluster_size;

            let size = self.vfat.lock(|vfat| vfat.read_cluster(curr_cluster, offset as usize, &mut buf[total_size as usize..max_size as usize]))? as u64;

            self.curr_offset += size;
            total_size += size;

            if size ==  cluster_size - offset {
                // At end of the cluster, get next cluster
                self.curr_cluster = self.vfat.lock(|vfat| vfat.next_cluster(curr_cluster));
            }
        }

        return Ok(total_size as usize);
    }
}

/// Probably gonna need this for the project
impl<HANDLE: VFatHandle> io::Write for File<HANDLE> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unimplemented!("OS Pals' project baby")
    }

    fn flush(&mut self) -> io::Result<()> {
        unimplemented!("Yessir")
    }
}


impl<HANDLE: VFatHandle> io::Seek for File<HANDLE> {
    /// Seek to offset `pos` in the file.
    ///
    /// A seek to the end of the file is allowed. A seek _beyond_ the end of the
    /// file returns an `InvalidInput` error.
    ///
    /// If the seek operation completes successfully, this method returns the
    /// new position from the start of the stream. That position can be used
    /// later with SeekFrom::Start.
    ///
    /// # Errors
    ///
    /// Seeking before the start of a file or beyond the end of the file results
    /// in an `InvalidInput` error.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(offset) => {
                if offset >= self.size {
                    return Err(Error::new(ErrorKind::InvalidInput, "Invalid offset"));
                }
                self.curr_offset = offset;
                return Ok(self.curr_offset);
            },
            SeekFrom::End(offset) => {
                let offset = self.size as i64 + offset;
                if offset < 0 {
                    return Err(Error::new(ErrorKind::InvalidInput, "Cannot seek to before start of file"));
                }

                self.curr_offset = offset as u64;
                return Ok(self.curr_offset);
            },
            SeekFrom::Current(offset) => {
                let offset = self.curr_offset as i64 + offset;
                if offset < 0 {
                    return Err(Error::new(ErrorKind::InvalidInput, "Cannot seek to before start of file"));
                }

                self.curr_offset = offset as u64;
                return Ok(self.curr_offset);
            }
        }
    }
}

impl<HANDLE: VFatHandle> traits::File for File<HANDLE> {
    fn sync(&mut self) -> io::Result<()> {
        unimplemented!("Sync is not implemented");
    }

    fn size(&self) -> u64 {
        return self.size;
    }
}
