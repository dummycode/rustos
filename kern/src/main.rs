#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![feature(ptr_internals)]
#![feature(raw_vec_internals)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

extern crate alloc;

pub mod allocator;
pub mod console;
pub mod fs;
pub mod mutex;
pub mod shell;
pub mod param;
pub mod process;
pub mod traps;
pub mod vm;
pub mod irq;

use console::kprintln;

use allocator::Allocator;
use fs::FileSystem;
use process::GlobalScheduler;
use traps::irq::Irq;
use vm::VMManager;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();
pub static SCHEDULER: GlobalScheduler = GlobalScheduler::uninitialized();
pub static VMM: VMManager = VMManager::uninitialized();
pub static IRQ: Irq = Irq::uninitialized();

use core::time::Duration;
use pi::timer;

use alloc::string::String;

use shim::io::Read;

use aarch64;

use shim::path::Path;

fn kmain() -> ! {
    // Sleep for one (1) second so we have time to call screen
    timer::spin_sleep(Duration::new(1, 0));

    kprintln!("Hello and welcome to hhOS 1.0.0");

    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
        IRQ.initialize();
        VMM.initialize();
        SCHEDULER.initialize();
        SCHEDULER.start();
    }

    kprintln!("Spinning forever...");
    loop {
        aarch64::nop();
    }
}

