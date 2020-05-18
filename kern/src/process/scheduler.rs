use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use core::fmt;

use aarch64::*;

use crate::mutex::Mutex;
use crate::param::{PAGE_MASK, PAGE_SIZE, TICK, USER_IMG_BASE};
use crate::process::{Id, Process, State};
use crate::traps::TrapFrame;
use crate::VMM;
use crate::IRQ;

use crate::irq::timer_handler;

use shim::path::PathBuf;

use crate::console::{kprintln};

use pi::interrupt::{Interrupt, Controller};
use pi::timer;

/// Process scheduler for the entire machine.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Scheduler>>);

impl GlobalScheduler {
    /// Returns an uninitialized wrapper around a local scheduler.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    /// Enter a critical region and execute the provided closure with the
    /// internal scheduler.
    pub fn critical<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Scheduler) -> R,
    {
        let mut guard = self.0.lock();
        f(guard.as_mut().expect("scheduler uninitialized"))
    }


    /// Adds a process to the scheduler's queue and returns that process's ID.
    /// For more details, see the documentation on `Scheduler::add()`.
    pub fn add(&self, process: Process) -> Option<Id> {
        self.critical(move |scheduler| scheduler.add(process))
    }

    /// Performs a context switch using `tf` by setting the state of the current
    /// process to `new_state`, saving `tf` into the current process, and
    /// restoring the next process's trap frame into `tf`. For more details, see
    /// the documentation on `Scheduler::schedule_out()` and `Scheduler::switch_to()`.
    pub fn switch(&self, new_state: State, tf: &mut TrapFrame) -> Id {
        self.critical(|scheduler| scheduler.schedule_out(new_state, tf));
        self.switch_to(tf)
    }

    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        loop {
            let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
            if let Some(id) = rtn {
                return id;
            }
            aarch64::wfe();
        }
    }

    /// Kills currently running process and returns that process's ID.
    /// For more details, see the documentaion on `Scheduler::kill()`.
    #[must_use]
    pub fn kill(&self, tf: &mut TrapFrame) -> Option<Id> {
        self.critical(|scheduler| scheduler.kill(tf))
    }

    /// Starts executing processes in user space using timer interrupt based
    /// preemptive scheduling. This method should not return under normal conditions.
    pub fn start(&self) -> ! {
        let mut tf = &mut TrapFrame::default();
        self.switch_to(tf);

        unsafe {
            asm!("
                // Set SP to TrapFrame
                mov sp, $0

                bl context_restore

                // Set sp back to _start
                adr x0, $1
                mov sp, x0

                // Clear x0
                mov x0, xzr

                // Return to EL0
                eret
                "
                :: "r"(tf), "i"(PAGE_SIZE)
                :: "volatile"
            )
        }

        // Loop infinitely to satisfy compiler
        kprintln!("Looping infinitely");
        loop {
            aarch64::nop();
        }
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler
    pub unsafe fn initialize(&self) {
        // Enable timer interrupts
        let mut int_cnt = Controller::new();
        int_cnt.enable(Interrupt::Timer1);

        // Register timer handler
        IRQ.register(Interrupt::Timer1, Box::new(timer_handler));

        // Start a timer
        timer::tick_in(TICK);

        // Create the scheduler
        let mut scheduler = Scheduler::new();
        *self.0.lock() = Some(scheduler);
        
        /*
        let mut process1 = Process::new().expect("Expected process");

        process1.context.sp = process1.stack.top().as_u64();
        process1.context.elr = USER_IMG_BASE as u64;
        process1.context.spsr = process1.context.spsr & !(0b11 << 2);
        process1.context.spsr = process1.context.spsr & !(0b1 << 7);
        process1.context.ttbr0 = VMM.get_baddr().as_u64();
        process1.context.ttbr1 = process1.vmap.get_baddr().as_u64();

        self.test_phase_3(&mut process1);

        self.add(process1);
        */
        let process1 = Process::load(PathBuf::from("/fib")).expect("Expected process");
        let process2 = Process::load(PathBuf::from("/fib")).expect("Expected process");
        let process3 = Process::load(PathBuf::from("/fib")).expect("Expected process");
        let process4 = Process::load(PathBuf::from("/fib")).expect("Expected process");

        self.add(process1);
        self.add(process2);
        self.add(process3);
        self.add(process4);
    }

    // The following method may be useful for testing Phase 3:
    //
    // * A method to load a extern function to the user process's page table.
    //
    pub fn test_phase_3(&self, proc: &mut Process){
        use crate::vm::{VirtualAddr, PagePerm};
    
        let mut page = proc.vmap.alloc(
            VirtualAddr::from(USER_IMG_BASE as u64), PagePerm::RWX);
    
        let text = unsafe {
            core::slice::from_raw_parts(test_user_process as *const u8, 24)
        };
    
        page[0..24].copy_from_slice(text);
    }
}

#[derive(Debug)]
pub struct Scheduler {
    processes: VecDeque<Process>,
    last_id: Option<Id>,
}

impl Scheduler {
    /// Returns a new `Scheduler` with an empty queue.
    fn new() -> Scheduler {
        return Scheduler {
            processes: VecDeque::new(),
            last_id: None,
        };
    }

    /// Adds a process to the scheduler's queue and returns that process's ID if
    /// a new process can be scheduled. The process ID is newly allocated for
    /// the process and saved in its `trap_frame`. If no further processes can
    /// be scheduled, returns `None`.
    ///
    /// It is the caller's responsibility to ensure that the first time `switch`
    /// is called, that process is executing on the CPU.
    fn add(&mut self, mut process: Process) -> Option<Id> {
        let next_id = match self.last_id {
            Some(last_id) => {
                match last_id.checked_add(1) {
                    Some(sum) => sum,
                    None => return None,
                }
            }
            None => 0,
        };

        // Set process id in trap frame
        process.context.tpidr = next_id;

        // Add to queue
        self.processes.push_back(process);

        let new_id = Some(next_id);

        // Set new last id
        self.last_id = new_id;

        return new_id;
    }

    /// Finds the currently running process, sets the current process's state
    /// to `new_state`, prepares the context switch on `tf` by saving `tf`
    /// into the current process, and push the current process back to the
    /// end of `processes` queue.
    ///
    /// If the `processes` queue is empty or there is no current process,
    /// returns `false`. Otherwise, returns `true`.
    fn schedule_out(&mut self, new_state: State, tf: &mut TrapFrame) -> bool {
        for i in 0..self.processes.len() {
            match self.processes.get(i) {
                Some(process) => {
                    // If current running process
                    if process.context.tpidr == tf.tpidr {
                        let mut current_process = self.processes.remove(i).expect("Expected process");

                        current_process.state = new_state;
                        current_process.context = Box::new(*tf);

                        self.processes.push_back(current_process);
                        return true;
                    }
                }
                None => continue,
            }
        }

        // Did not find a running process
        return false;
    }

    /// Finds the next process to switch to, brings the next process to the
    /// front of the `processes` queue, changes the next process's state to
    /// `Running`, and performs context switch by restoring the next process`s
    /// trap frame into `tf`.
    ///
    /// If there is no process to switch to, returns `None`. Otherwise, returns
    /// `Some` of the next process`s process ID.
    fn switch_to(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        for (i, process) in self.processes.iter_mut().enumerate() {
            if process.is_ready() {
                let mut ready_process = self.processes.remove(i).expect("Expected process");
                *tf = *ready_process.context;

                ready_process.state = State::Running;

                let pid = ready_process.context.tpidr;
                self.processes.push_front(ready_process);

                return Some(pid);
            }
        }
        return None;
    }

    /// Kills currently running process by scheduling out the current process
    /// as `Dead` state. Removes the dead process from the queue, drop the
    /// dead process's instance, and returns the dead process's process ID.
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        if !self.schedule_out(State::Dead, tf) {
            return None;
        }

        for (i, process) in self.processes.iter_mut().enumerate() {
            match process.state {
                State::Dead => {
                    let dead_process = self.processes.remove(i).expect("Expected process");
                    return Some(dead_process.context.tpidr);
                },
                _ => continue,
            }
        }

        return None;
    }
}

pub extern "C" fn  test_user_process() -> ! {
    aarch64::brk!(69);
    loop {
        let ms = 10000;
        let error: u64;
        let elapsed_ms: u64;

        unsafe {
            asm!("mov x0, $2
              svc 1
              mov $0, x0
              mov $1, x7"
                 : "=r"(elapsed_ms), "=r"(error)
                 : "r"(ms)
                 : "x0", "x7"
                 : "volatile");
        }
    }
}

