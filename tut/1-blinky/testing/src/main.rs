#![feature(asm)]
#![feature(global_asm)]

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

const GPIO_BASE: usize = 0x3F000000 + 0x200000;

const GPIO_FSEL1: *mut u32 = (GPIO_BASE + 0x04) as *mut u32;
const GPIO_SET0: *mut u32 = (GPIO_BASE + 0x1C) as *mut u32;
const GPIO_CLR0: *mut u32 = (GPIO_BASE + 0x28) as *mut u32;

#[inline(never)]
fn spin_sleep_ms(ms: usize) {
  for _ in 0..(ms * 6000) {
    unsafe { asm!("nop" :::: "volatile"); }
  }
}

unsafe fn kmain() -> ! {
  GPIO_FSEL1.write_volatile((1 << 18) | (1 << 27)); // set GPIO as 16 and 19

  loop {
    GPIO_SET0.write_volatile(1 << 16);
    GPIO_CLR0.write_volatile(1 << 19); 
    
    spin_sleep_ms(250);

    GPIO_CLR0.write_volatile(1 << 16);
    GPIO_SET0.write_volatile(1 << 19);

    spin_sleep_ms(250);
  }
}
