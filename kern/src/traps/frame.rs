use core::fmt;

#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
pub struct TrapFrame {
    // FIXME: Fill me in.
    pub tpidr: u64,
    pub sp: u64,
    pub spsr: u64,
    pub elr: u64,
    pub ttbr0: u64,
    pub ttbr1: u64,
    pub q_regs: [u128; 32],
    pub x_regs: [u64; 32],
}

impl TrapFrame  {
    pub fn zeroed() -> TrapFrame {
        return TrapFrame {
            tpidr: 0,
            sp: 0,
            spsr: 0,
            elr: 0,
            ttbr0: 0,
            ttbr1: 0,
            q_regs: [0; 32],
            x_regs: [0; 32],
        }
    }
}

