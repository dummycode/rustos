use core::fmt;
use shim::const_assert_size;

use crate::traits::BlockDevice;
use crate::vfat::Error;

#[repr(C, packed)]
pub struct BiosParameterBlock {
    // FIXME: Fill me in.
    _1: [u8; 3],
    _2: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub num_reserved_sectors: u16,
    pub num_fats: u8,
    _7: [u8; 2],
    _8: [u8; 2],
    _9: [u8; 1],
    _10: [u8; 2],
    _11: [u8; 2],
    _12: [u8; 2],
    _13: [u8; 4],
    pub num_logical_sectors: u32,
    pub sectors_per_fat: u32,
    _16: [u8; 2],
    _17: [u8; 2],
    pub root_cluster_num: u32,
    _19: [u8; 2],
    _20: [u8; 2],
    _21: [u8; 12],
    _22: [u8; 1],
    _23: [u8; 1],
    signature1: u8,
    _25: [u8; 4],
    _26: [u8; 11],
    _27: [u8; 8],
    _28: [u8; 420],
    signature2: u16,
}

const_assert_size!(BiosParameterBlock, 512);

impl BiosParameterBlock {
    /// Reads the FAT32 extended BIOS parameter block from sector `sector` of
    /// device `device`.
    ///
    /// # Errors
    ///
    /// If the EBPB signature is invalid, returns an error of `BadSignature`.
    pub fn from<T: BlockDevice>(mut device: T, sector: u64) -> Result<BiosParameterBlock, Error> {
        let mut buf: [u8; 512] = [0; 512];
        let res = device.read_sector(sector, &mut buf);

        let size = match res {
            Ok(size) => size,
            Err(err) => return Err(Error::Io(err)),
        };

        let ebpb: BiosParameterBlock = unsafe { core::mem::transmute(buf) };

        // Reject if EBPB's signature is invalid
        if ebpb.signature2 != 0xAA55 {
            return Err(Error::BadSignature);
        }

        return Ok(ebpb);
    }
}

impl fmt::Debug for BiosParameterBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BiosParameterBlock")
            .field("signature1", &self.signature1)
            .field("signature2", &self.signature2)
            .finish()
    }
}
