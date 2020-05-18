use crate::traits;
use crate::vfat::{Dir, File, Metadata, VFatHandle};
use core::fmt;

// You can change this definition if you want
#[derive(Debug)]
pub enum Entry<HANDLE: VFatHandle> {
    EntryFile(File<HANDLE>),
    EntryDir(Dir<HANDLE>),
}

impl<HANDLE: VFatHandle> traits::Entry for Entry<HANDLE> {
    // FIXME: Implement `traits::Entry` for `Entry`.
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Metadata = Metadata;
    
    fn name(&self) -> &str {
        match self {
            Entry::EntryFile(file) => &file.name,
            Entry::EntryDir(dir) => &dir.name,
        }
    }

    fn metadata(&self) -> &Self::Metadata {
        match self {
            Entry::EntryFile(file) => &file.metadata,
            Entry::EntryDir(dir) => &dir.metadata,
        }
    }

    fn as_file(&self) -> Option<&Self::File> {
        match self {
            Entry::EntryFile(file) => Some(file),
            _ => None
        }
    }

    fn as_dir(&self) -> Option<&Self::Dir> {
        match self {
            Entry::EntryDir(dir) => Some(dir),
            _ => None
        }
    }

    fn into_file(self) -> Option<Self::File> {
        match self {
            Entry::EntryFile(file) => Some(file),
            _ => None
        }
    }

    fn into_dir(self) -> Option<Self::Dir> {
        match self {
            Entry::EntryDir(dir) => Some(dir),
            _ => None
        }
    }
}
