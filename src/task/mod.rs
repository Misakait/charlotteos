pub mod context;
pub mod scheduler;
pub mod switch;
pub mod tcb;

use crate::{data_struct::lock::IrqLock, task::scheduler::Scheduler};
use lazy_static::lazy_static;
lazy_static! {
    pub static ref SCHEDULER: IrqLock<Scheduler> = IrqLock::new(Scheduler::new());
}
// pub static SCHEDULER: IrqLock<Scheduler> = IrqLock::new(Scheduler::new());
