use core::fmt;
use shim::const_assert_size;
use shim::io;

use crate::traits::BlockDevice;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CHS {
    // FIXME: Fill me in.
    _unused: [u8; 3],
}

// FIXME: implement Debug for CHS
impl fmt::Debug for CHS {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CHS")
            .finish()
    }
}

const_assert_size!(CHS, 3);

#[repr(C, packed)]
pub struct PartitionEntry {
    // FIXME: Fill me in.
    indicator_flag: u8,
    _starting_chs: CHS,
    pub partition_type: u8,
    _ending_chs: CHS,
    pub relative_sector: u32,
    pub total_sectors: u32,
}

// FIXME: implement Debug for PartitionEntry

const_assert_size!(PartitionEntry, 16);

/// The master boot record (MBR).
#[repr(C, packed)]
pub struct MasterBootRecord {
    // FIXME: Fill me in.
    bootstrap: [u8; 436],
    id: [u8; 10],
    pub entries: [PartitionEntry; 4],
    signature: [u8; 2]
}

// FIXME: implemente Debug for MaterBootRecord
impl fmt::Debug for MasterBootRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterBootRecord")
            .field("id", &self.id)
            .field("signature", &self.signature)
            .finish()
    }
}

const_assert_size!(MasterBootRecord, 512);

#[derive(Debug)]
pub enum Error {
    /// There was an I/O error while reading the MBR.
    Io(io::Error),
    /// Partiion `.0` (0-indexed) contains an invalid or unknown boot indicator.
    UnknownBootIndicator(u8),
    /// The MBR magic signature was invalid.
    BadSignature,
}

impl MasterBootRecord {
    /// Reads and returns the master boot record (MBR) from `device`.
    ///
    /// # Errors
    ///
    /// Returns `BadSignature` if the MBR contains an invalid magic signature.
    /// Returns `UnknownBootIndicator(n)` if partition `n` contains an invalid
    /// boot indicator. Returns `Io(err)` if the I/O error `err` occured while
    /// reading the MBR.
    pub fn from<T: BlockDevice>(mut device: T) -> Result<MasterBootRecord, Error> {
        let mut buf: [u8; 512] = [0; 512];
        let res = device.read_sector(0, &mut buf);

        let size = match res {
            Ok(size) => size,
            Err(err) => return Err(Error::Io(err)),
        };

        let mbr: MasterBootRecord = unsafe { core::mem::transmute(buf) };

        // Reject if MBR's signature is invalid
        if !(mbr.signature[0] == 0x55 && mbr.signature[1] == 0xAA) {
            return Err(Error::BadSignature);
        }

        // Reject if any entry is invalid
        let mut i = 0;
        for entry in mbr.entries.iter() {
            if !(entry.indicator_flag == 0x00 || entry.indicator_flag == 0x80) {
                return Err(Error::UnknownBootIndicator(i));
            }
            i += 1;
        }

        return Ok(mbr);
    }
}
