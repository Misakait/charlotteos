use core::arch::asm;

use crate::{
    bsp::qemu_virt::{MTIME_ADDR, RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ, VIRT_TEST_ADDR},
    polling_println,
    task::{SCHEDULER, context::TaskContext, scheduler::Scheduler},
    trap::interrupts::get_mtime,
};

pub fn schedule(tcb: &mut TaskContext) -> usize {
    tcb.mepc = tcb.mepc + 4;

    let next_ctx_ptr = Scheduler::schedule_on_interrupt();
    unsafe {
        asm!("csrw mscratch, {}", in(reg) next_ctx_ptr);
        (*next_ctx_ptr).mepc
    }
}

pub fn sleep(tcb: &mut TaskContext) -> usize {
    unsafe {
        tcb.mepc = tcb.mepc + 4;
        let sleep_ms = tcb.a0;
        const ONE_MS_CYCLES: usize = (RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ as f64 * 0.001) as usize;
        let current_mtime = get_mtime();
        let target_mtime = current_mtime + sleep_ms * ONE_MS_CYCLES;
        let mut scheduler = SCHEDULER.lock();
        let next_ctx = scheduler.set_current_task_sleep(target_mtime);
        asm!("csrw mscratch, {}", in(reg) next_ctx);
        return (*next_ctx).mepc;
    }
}
