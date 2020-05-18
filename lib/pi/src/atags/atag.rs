use crate::atags::raw;

use core::slice;
use core::str;

pub use crate::atags::raw::{Core, Mem};

/// An ATAG.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Atag {
  Core(raw::Core),
  Mem(raw::Mem),
  Cmd(&'static str),
  Unknown(u32),
  None,
}

impl Atag {
  /// Returns `Some` if this is a `Core` ATAG. Otherwise returns `None`.
  pub fn core(self) -> Option<Core> {
    match self {
      Atag::Core(core) => { return Some(core); },
      _ => { return None }
    }
  }

  /// Returns `Some` if this is a `Mem` ATAG. Otherwise returns `None`.
  pub fn mem(self) -> Option<Mem> {
    match self {
      Atag::Mem(mem) => { return Some(mem); },
      _ => { return None }
    }
  }

  /// Returns `Some` with the command line string if this is a `Cmd` ATAG.
  /// Otherwise returns `None`.
  pub fn cmd(self) -> Option<&'static str> {
    match self {
      Atag::Cmd(str) => { return Some(str); },
      _ => { return None }
    }
  }
}

// FIXME: Implement `From<&raw::Atag> for `Atag`.
impl From<&'static raw::Atag> for Atag {
  fn from(atag: &'static raw::Atag) -> Atag {
    // FIXME: Complete the implementation below.

    unsafe {
      match (atag.tag, &atag.kind) {
        (raw::Atag::CORE, &raw::Kind { core }) => Atag::Core(core),
        (raw::Atag::MEM, &raw::Kind { mem }) => Atag::Mem(mem),
        (raw::Atag::CMDLINE, &raw::Kind { ref cmd }) => {
          let mut start = cmd as *const raw::Cmd as *const u8;

          let mut ptr = cmd as *const raw::Cmd as *mut u8;
          let mut cmd_len = 0;

          // sum while not null terminator 
          while *ptr != 0 {
            ptr = ptr.add(1);
            cmd_len += 1;
          }
          
          let my_slice = slice::from_raw_parts(start, cmd_len);
          let my_str = str::from_utf8_unchecked(my_slice);

          return Atag::Cmd(my_str);
        },
        (raw::Atag::NONE, _) => Atag::None,
        (id, _) => Atag::Unknown(id),
      }
    }
  }
}
