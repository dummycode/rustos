use alloc::boxed::Box;
use pi::interrupt::Interrupt;

use crate::mutex::Mutex;
use crate::traps::TrapFrame;

pub type IrqHandler = Box<dyn FnMut(&mut TrapFrame) + Send>;
pub type IrqHandlers = [Option<IrqHandler>; Interrupt::MAX];

pub struct Irq(Mutex<Option<IrqHandlers>>);

impl Irq {
    pub const fn uninitialized() -> Irq {
        Irq(Mutex::new(None))
    }

    pub fn initialize(&self) {
        *self.0.lock() = Some([None, None, None, None, None, None, None, None]);
    }

    /// Register an irq handler for an interrupt.
    /// The caller should assure that `initialize()` has been called before calling this function.
    pub fn register(&self, int: Interrupt, handler: IrqHandler) {
        let index = Interrupt::to_index(int);
        self.0.lock().as_mut().expect("Expected mutex")[index] = Some(Box::new(handler));
    }

    /// Executes an irq handler for the givven interrupt.
    /// The caller should assure that `initialize()` has been called before calling this function.
    pub fn invoke(&self, int: Interrupt, tf: &mut TrapFrame) {
        let index = Interrupt::to_index(int);
        self.0.lock().as_mut().expect("Expected mutex")[index].as_mut().expect("Expected handler")(tf);
    }
}
