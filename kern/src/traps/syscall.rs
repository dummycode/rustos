use alloc::boxed::Box;
use core::time::Duration;

use crate::console::{CONSOLE, kprint, kprintln};
use crate::process::{Process, State};
use crate::traps::TrapFrame;
use crate::SCHEDULER;
use kernel_api::*;
use pi::timer;
use crate::param::{TICK};

/// Sleep for `ms` milliseconds.
///
/// This system call takes one parameter: the number of milliseconds to sleep.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the approximate true elapsed time from when `sleep` was called to
/// when `sleep` returned.
pub fn sys_sleep(ms: u32, tf: &mut TrapFrame) {
    let start = timer::current_time().as_millis() as u32;
    let boxed_fn = Box::new(move |p: &mut Process| {
        let time = timer::current_time().as_millis() as u32;
        if time - start >= ms {
            p.context.x_regs[0] = (time - start) as u64;
            return true;
        }
        return false;
    });

    kprintln!("Sleeping process (pid={}) for {}ms", tf.tpidr, ms);

    // Give new process correct time
    timer::tick_in(TICK);

    SCHEDULER.switch(State::Waiting(boxed_fn), tf);
}

/// Returns current time.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns two
/// parameter:
///  - current time as seconds
///  - fractional part of the current time, in nanoseconds.
pub fn sys_time(tf: &mut TrapFrame) {
    let time = timer::current_time();
    tf.x_regs[0] = time.as_secs();
    tf.x_regs[1] = time.subsec_millis() as u64;
}

/// Kills current process.
///
/// This system call does not take paramer and does not return any value.
pub fn sys_exit(tf: &mut TrapFrame) {
    timer::tick_in(TICK);

    SCHEDULER.switch(State::Dead, tf);
}

/// Write to console.
///
/// This system call takes one parameter: a u8 character to print.
///
/// It only returns the usual status value.
pub fn sys_write(b: u8, tf: &mut TrapFrame) {
    kprint!("{}", b as char);
}

/// Returns current process's ID.
///
/// This system call does not take parameter.
///
/// In addition to the usual status value, this system call returns a
/// parameter: the current process's ID.
pub fn sys_getpid(tf: &mut TrapFrame) {
    tf.x_regs[0] = tf.tpidr as u64;
}

pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    use crate::console::kprintln;
    match num {
        1 => sys_sleep(tf.x_regs[0] as u32, tf),
        2 => sys_time(tf),
        3 => sys_exit(tf),
        4 => sys_write(tf.x_regs[0] as u8, tf),
        5 => sys_getpid(tf),
        _ => unimplemented!("Unimplemented syscall"),
    }
}
