use alloc::boxed::Box;
use shim::io;
use shim::path::Path;
use core::mem;

use aarch64;

use crate::param::*;
use crate::process::{Stack, State};
use crate::traps::TrapFrame;
use crate::vm::*;
use kernel_api::{OsError, OsResult};

use crate::FILESYSTEM;
use fat32::traits::FileSystem as FileSystemTrait;
use shim::io::Read;

/// Type alias for the type of a process ID.
pub type Id = u64;

/// A structure that represents the complete state of a process.
#[derive(Debug)]
pub struct Process {
    /// The saved trap frame of a process.
    pub context: Box<TrapFrame>,
    /// The memory allocation used for the process's stack.
    pub stack: Stack,
    /// The page table describing the Virtual Memory of the process
    pub vmap: Box<UserPageTable>,
    /// The scheduling state of the process.
    pub state: State,
}

impl Process {
    /// Creates a new process with a zeroed `TrapFrame` (the default), a zeroed
    /// stack of the default size, and a state of `Ready`.
    ///
    /// If enough memory could not be allocated to start the process, returns
    /// `None`. Otherwise returns `Some` of the new `Process`.
    pub fn new() -> OsResult<Process> {
        let stack_res = Stack::new();
        let stack;

        match stack_res {
            Some(s) => {
                stack = s;
            },
            None => {
                return Err(OsError::NoMemory);
            }
        }

        let state = State::Ready;
        let tf = Box::new(TrapFrame::zeroed());
        let vmap = Box::new(UserPageTable::new()); 

        return Ok(Process {
            context: tf,
            stack: stack,
            state: state,
            vmap: vmap,
        });
    }

    /// Load a program stored in the given path by calling `do_load()` method.
    /// Set trapframe `context` corresponding to the its page table.
    /// `sp` - the address of stack top
    /// `elr` - the address of image base.
    /// `ttbr0` - the base address of kernel page table
    /// `ttbr1` - the base address of user page table
    /// `spsr` - `F`, `A`, `D` bit should be set.
    ///
    /// Returns Os Error if do_load fails.
    pub fn load<P: AsRef<Path>>(pn: P) -> OsResult<Process> {
        use crate::VMM;

        let mut p = Process::do_load(pn)?;

        //FIXME: Set trapframe for the process.
        p.context.sp = Process::get_stack_top().as_u64();
        p.context.elr = USER_IMG_BASE as u64;
        p.context.spsr = 0x0000_0340;
        p.context.ttbr0 = VMM.get_baddr().as_u64();
        p.context.ttbr1 = p.vmap.get_baddr().as_u64();

        Ok(p)
    }

    /// Creates a process and open a file with given path.
    /// Allocates one page for stack with read/write permission, and N pages with read/write/execute
    /// permission to load file's contents.
    fn do_load<P: AsRef<Path>>(pn: P) -> OsResult<Process> {
        let mut process = Process::new()?;

        // Allocate one page for stack
        let stack = process.vmap.alloc(Process::get_stack_base(), PagePerm::RW);
        
        let mut file = FILESYSTEM.open_file(pn)?;

        if file.size > PAGE_SIZE as u64 {
            unimplemented!("User programs must be less than {} bytes", PAGE_SIZE);
        }

        let mut bytes: u64 = 0;

        // Read a page at a time
        while bytes < file.size {
            let mut page = process.vmap.alloc(VirtualAddr::from(USER_IMG_BASE), PagePerm::RWX);
            let size = file.read(page)?;
            bytes += size as u64;
        }

        return Ok(process);
    }

    /// Returns the highest `VirtualAddr` that is supported by this system.
    pub fn get_max_va() -> VirtualAddr {
        return VirtualAddr::from(core::usize::MAX);
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// memory space.
    pub fn get_image_base() -> VirtualAddr {
        return VirtualAddr::from(USER_IMG_BASE);
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// process's stack.
    pub fn get_stack_base() -> VirtualAddr {
        return VirtualAddr::from(USER_STACK_BASE);
    }

    /// Returns the `VirtualAddr` represents the top of the user process's
    /// stack.
    pub fn get_stack_top() -> VirtualAddr {
        let top = Process::get_stack_base().as_usize() + PAGE_SIZE - 1;
        return VirtualAddr::from((top / 16) * 16);
    }

    /// Returns `true` if this process is ready to be scheduled.
    ///
    /// This functions returns `true` only if one of the following holds:
    ///
    ///   * The state is currently `Ready`.
    ///
    ///   * An event being waited for has arrived.
    ///
    ///     If the process is currently waiting, the corresponding event
    ///     function is polled to determine if the event being waiting for has
    ///     occured. If it has, the state is switched to `Ready` and this
    ///     function returns `true`.
    ///
    /// Returns `false` in all other cases.
    pub fn is_ready(&mut self) -> bool {
        match self.state {
            State::Ready => true,
            State::Waiting(_) => {
                let mut state = mem::replace(&mut self.state, State::Ready);

                match state {
                    State::Waiting(mut func) => {
                        if !func(self) {
                            mem::replace(&mut self.state, State::Waiting(func));
                            return false;
                        }
                        return true;
                    },
                    _ => panic!("What happened here")
                }
            },
            _ => false
        }
    }
}
