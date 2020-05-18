use core::fmt;

use alloc::string::String;

use crate::traits;

/// A date as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Date(pub u16);

/// Time as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Time(pub u16);

/// File attributes as represented in FAT32 on-disk structures.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Attributes(pub u8);

/// A structure containing a date and time.
#[derive(Default, Copy, Clone, PartialEq, Eq)]
pub struct Timestamp {
    pub date: Date,
    pub time: Time,
}

impl Timestamp {
    pub fn new(date: Date, time: Time) -> Timestamp {
        return Timestamp {
            date,
            time,
        }
    }
}

/// Metadata for a directory entry.
#[derive(Default, Clone)]
pub struct Metadata {
    created_at: Timestamp,
    accessed_at: Timestamp,
    modified_at: Timestamp,
    attr: Attributes,
}

impl Metadata {
    pub fn new(created_at: Timestamp, accessed_at: Timestamp, modified_at: Timestamp, attr: Attributes) -> Metadata {
        return Metadata {
            created_at,
            accessed_at,
            modified_at,
            attr,
        };
    }

    pub fn now() -> Metadata {
        let timestamp = Timestamp::new(Date(0), Time(0));
        let attr = Attributes(0);

        return Metadata::new(timestamp, timestamp, timestamp, attr);
    }
}

/// Implement Timestamp trait for Timestamp
impl traits::Timestamp for Timestamp {
    fn year(&self) -> usize {
        return 1980 + ((self.date.0 >> 9) & 0x7F) as usize;
    }

    fn month(&self) -> u8 {
        return ((self.date.0 >> 5) & 0xF) as u8;
    }

    fn day(&self) -> u8 {
        return ((self.date.0) & 0x1F) as u8;
    }

    fn hour(&self) -> u8 {
        return ((self.time.0 >> 11) & 0x1F) as u8;
    }

    fn minute(&self) -> u8 {
        return ((self.time.0 >> 5) & 0x3F) as u8;
    }

    fn second(&self) -> u8 {
        return ((self.time.0) & 0x1F) as u8 * 2;
    }
}

/// Implement Metadata trait for Metadata
impl traits::Metadata for Metadata {
    type Timestamp = Timestamp;

    fn read_only(&self) -> bool {
        return self.attr.0 & 0x01 == 0x01;
    }

    fn hidden(&self) -> bool {
        return self.attr.0 & 0x02 == 0x02;
    }

    fn created(&self) -> Self::Timestamp {
        return self.created_at;
    }

    fn accessed(&self) -> Self::Timestamp {
        return self.accessed_at;
    }

    fn modified(&self) -> Self::Timestamp {
        return self.modified_at;
    }
}

/// Debug implementation for a Timestamp
impl fmt::Debug for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use traits::Timestamp;

        write!(
            f,
            "{}/{}/{} {}:{}:{}",
            self.day(),
            self.month(),
            self.year(),
            self.hour(),
            self.minute(),
            self.second()
        )
    }
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use traits::Metadata;

        f.debug_struct("Metadata")
            .field("read_only", &self.read_only())
            .field("hidden", &self.hidden())
            .field("created_at", &self.created())
            .field("accessed_at", &self.accessed())
            .field("modified_at", &self.modified())
            .finish()
    }
}
