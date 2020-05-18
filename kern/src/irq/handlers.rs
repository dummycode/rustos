use alloc::string::String;

use crate::console::{kprintln};
use crate::shell;
use crate::SCHEDULER;
use crate::IRQ;
use crate::param::{TICK};
use crate::traps::TrapFrame;
use crate::process::{State};

use pi::interrupt::{Interrupt, Controller};

use pi::timer;

#[no_mangle]
pub fn timer_handler(tf: &mut TrapFrame) {
    timer::tick_in(TICK);

    SCHEDULER.switch(State::Ready, tf);
}
