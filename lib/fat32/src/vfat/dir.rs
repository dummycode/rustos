use alloc::string::String;
use alloc::vec::Vec;

use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io;
use shim::newioerr;

use crate::traits;
use crate::util::VecExt;
use crate::vfat::{Attributes, Date, Metadata, Time, Timestamp};
use crate::vfat::{Cluster, Entry, File, VFatHandle};
use crate::vfat::error::{Error};

use core::char::{decode_utf16, REPLACEMENT_CHARACTER};

#[derive(Debug)]
pub struct Dir<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    start: Cluster,
    curr: Cluster,
    pub metadata: Metadata,
    pub name: String,
}

impl<HANDLE: VFatHandle> Dir<HANDLE> {
    pub fn new(vfat: HANDLE, start: Cluster, metadata: Metadata, name: String) -> Dir<HANDLE> {
        return Dir {
            vfat,
            start,
            curr: start,
            metadata,
            name,
        };
    }

    pub fn root(vfat: HANDLE, root_cluster: Cluster) -> Dir<HANDLE> {
        return Dir::new(vfat, root_cluster, Metadata::now(), String::from("/"));
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatRegularDirEntry {
    // FIXME: Fill me in.
    file_name: [u8; 8],
    file_ext: [u8; 3],
    attributes: u8,
    _reserved: [u8; 1],
    _tenths: [u8; 1],
    created_at_time: Time,
    created_at_date: Date,
    accessed_at: Date,
    high_bits_first_cluster_number: u16,
    modified_at_time: Time,
    modified_at_date: Date,
    low_bits_first_cluster_number: u16,
    size: u32,
}

const_assert_size!(VFatRegularDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatLfnDirEntry {
    sequence_number: u8,
    first_file_name: [u16; 5],
    _3: [u8; 1],
    _4: [u8; 1],
    _5: [u8; 1],
    second_file_name: [u16; 6],
    _7: [u8; 2],
    third_file_name: [u16; 2],
}

const_assert_size!(VFatLfnDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatUnknownDirEntry {
    // FIXME: Fill me in.
    file_name: [u8; 8],
    _1: [u8; 3],
    attributes: u8,
    _2: [u8; 20],
}

const_assert_size!(VFatUnknownDirEntry, 32);

pub union VFatDirEntry {
    unknown: VFatUnknownDirEntry,
    regular: VFatRegularDirEntry,
    long_filename: VFatLfnDirEntry,
}

pub struct EntryIterator<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    entries: Vec<VFatDirEntry>,
    curr_index: usize,
}

impl<HANDLE: VFatHandle> EntryIterator<HANDLE> {
    pub fn len(&self) -> usize {
        return self.entries.len();
    }
}

/// Null terminated string
fn parse_null_string(buf: &[u8]) -> String {
    let end = buf.iter()
        .position(|&c| c == 0x00 || c == 0x20)
        .unwrap_or(buf.len());

    match String::from_utf8(buf[..end].to_vec()) {
        Ok(name) => {
            return name;
        },
        Err(err) => panic!("Shite name"),
    };
}

/// Parse utf16 string
fn parse_utf16_string(buf: &[u16]) -> String {
    let end = buf.iter()
        .position(|&c| c == 0x00 || c == 0xFF)
        .unwrap_or(buf.len());

    let part = buf[..end].to_vec();

    return decode_utf16(part)
        .map(|r| r.unwrap_or('?'))
        .collect::<String>();
}

/// Implement iterator trait for our EntryIterator struct
impl<HANDLE: VFatHandle> Iterator for EntryIterator<HANDLE> {
    type Item = Entry<HANDLE>;

    /// Get next item in iterator
    fn next(&mut self) -> Option<Self::Item> {
        // String to store the file name
        let mut lfn: Vec<(u8, String)> = Vec::new();
        let mut in_lfn = false;
        let mut lfn_len = 0;

        while self.curr_index < self.entries.len() {
            // Get entry at curr_index
            let entry: &VFatDirEntry = &self.entries[self.curr_index];
            self.curr_index += 1;

            let unknown_entry = unsafe { entry.unknown };
            if unknown_entry.file_name[0] == 0x00 {
                return None;
            } else if unknown_entry.file_name[0] == 0xE5 {
                // This one is deleted, continue to next
                continue;
            }


            match unknown_entry.attributes {
                0x0F => {
                    // Long file name
                    let lfn_entry = unsafe { entry.long_filename };

                    if lfn_entry.sequence_number | 0x10 != 0 {
                        // First entry!
                        in_lfn = true;
                    }

                    if lfn_entry.sequence_number | 0x00 == 0 {
                        // Last entry!
                        in_lfn = false;
                    }

                    if in_lfn {
                        let mut first: String = parse_utf16_string(&{lfn_entry.first_file_name});
                        let second: String = parse_utf16_string(&{lfn_entry.second_file_name});
                        let third: String = parse_utf16_string(&{lfn_entry.third_file_name});

                        if first.len() == 5 {
                            first.push_str(&second);
                        }
                        if first.len() == 11 {
                            first.push_str(&third);
                        }

                        lfn.push((lfn_entry.sequence_number, first));
                        lfn_len += 1;
                    }
                    // Keep going until regular entry
                    continue;
                },
                _ => {
                    // Regular directory
                    let re = unsafe { entry.regular };

                    let metadata = Metadata::new( 
                        Timestamp::new(re.created_at_date, re.created_at_time),
                        Timestamp::new(re.accessed_at, Time(0)),
                        Timestamp::new(re.modified_at_date, re.modified_at_time),
                        Attributes(re.attributes),
                    );
                    let starting_cluster = Cluster::from((re.high_bits_first_cluster_number as u32) << 16 | re.low_bits_first_cluster_number as u32);

                    let mut name = match lfn_len {
                        0 => {
                            let mut string = parse_null_string(&re.file_name);
                            let mut extension = parse_null_string(&re.file_ext);

                            if extension.len() > 0 {
                                string.push_str(".");
                                string.push_str(&extension);
                            }

                            string
                        },
                        _ => {
                            // Sort by sequence number
                            lfn.sort_by_key(|k| k.0);
                            let parts: Vec<String> = lfn.into_iter().map(|p| p.1).collect();

                            // Build final name
                            let mut name: String = String::new();
                            for part in parts.iter() {
                                name.push_str(&part);
                            }
                            name
                        },
                    };

                    if re.attributes & 0x10 != 0 {
                        return Some(
                            Entry::EntryDir(
                                Dir::new(
                                    self.vfat.clone(),
                                    starting_cluster,
                                    metadata,
                                    name,
                                )
                            )
                        );
                    }
                    return Some(
                        Entry::EntryFile(
                            File::new(
                                self.vfat.clone(),
                                metadata,
                                starting_cluster,
                                re.size as u64,
                                name,
                            )
                        )
                    );
                },
                _ => {
                    // println!("{}", unknown_entry.attributes);
                    panic!("Why are we here")
                }
            }
        }
        return None;
    }
}

impl<HANDLE: VFatHandle> Dir<HANDLE> {
    /// Finds the entry named `name` in `self` and returns it. Comparison is
    /// case-insensitive.
    ///
    /// # Errors
    ///
    /// If no entry with name `name` exists in `self`, an error of `NotFound` is
    /// returned.
    ///
    /// If `name` contains invalid UTF-8 characters, an error of `InvalidInput`
    /// is returned.
    pub fn find<P: AsRef<OsStr>>(&self, name: P) -> io::Result<Entry<HANDLE>> {
        use traits::Dir;
        use traits::Entry;

        let name_str = match name.as_ref().to_str() {
            Some(name_str) => name_str,
            None => return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid name")),
        };

        let entries = self.entries()?;
        let entries_len = entries.len();
        for entry in entries {
            if entry.name().eq_ignore_ascii_case(name_str) {
                return Ok(entry);
            }
        }

        return Err(io::Error::new(io::ErrorKind::NotFound, "File not found"));
    }
}

impl<HANDLE: VFatHandle> traits::Dir for Dir<HANDLE> {
    type Entry = Entry<HANDLE>;
    type Iter = EntryIterator<HANDLE>;

    fn entries(&self) -> io::Result<Self::Iter> {
        let mut entries_data: Vec<u8> = Vec::new();
        self.vfat.lock(|vfat| vfat.read_chain(self.start, &mut entries_data));

        let entries: Vec<VFatDirEntry> = unsafe { entries_data.cast() };

        return Ok(
            EntryIterator {
                vfat: self.vfat.clone(),
                entries: entries,
                curr_index: 0,
            }
        );
    }
}
