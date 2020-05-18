#![feature(asm)]
#![no_std]
#![no_main]

mod cr0;

use kernel_api::println;
use kernel_api::syscall::{getpid, time, exit, sleep};
use core::time::Duration;

fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

fn main() {
    let pid = getpid();

    println!("Started process (pid={})...", pid);


    if pid == 2 {
        sleep(Duration::new(2, 500000000));
        println!("Exiting slept process (pid={}) at time {:?}", pid, time());
        exit();
    }

    let rtn = fib(30);

    println!("Ended: Result = {}", rtn);
    println!("Exiting process (pid={})", pid);
    exit();
}
