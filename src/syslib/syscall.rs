use core::{arch::asm, future::Ready, mem::transmute, usize};

use crate::{
    UART,
    bsp::qemu_virt::{MTIME_ADDR, RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ, VIRT_TEST_ADDR},
    driver::SerialPort,
    polling_println,
    task::{SCHEDULER, context::TaskContext, scheduler::Scheduler},
    trap::interrupts::{get_mtime, service::uart_service::UART_SERVICE},
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
        const ONE_MS_CYCLES: usize = (RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ / 1000) as usize;
        let current_mtime = get_mtime();
        let target_mtime = current_mtime + sleep_ms * ONE_MS_CYCLES;
        let mut scheduler = SCHEDULER.lock();
        let next_ctx = scheduler.set_current_task_sleep(target_mtime);
        asm!("csrw mscratch, {}", in(reg) next_ctx);
        return (*next_ctx).mepc;
    }
}

pub fn uart_read(ctx: &mut TaskContext) -> usize {
    let timeout_ms = ctx.a0; // 约定：-1 (usize::MAX) 代表无限阻塞
    ctx.mepc += 4;
    let mut receive_buffer = UART_SERVICE.receive_buffer.lock();
    if let Some(c) = receive_buffer.pop() {
        ctx.a0 = c as usize;
        // ctx.mepc += 4;
        return ctx.mepc;
    }
    drop(receive_buffer);

    if timeout_ms == 0 {
        // 非阻塞模式：没数据直接返回失败
        ctx.a0 = usize::MAX;
        return ctx.mepc;
    }

    // 阻塞或超时模式
    let task_id = SCHEDULER.lock().get_current_task_id();
    UART_SERVICE.wait_queue.lock().push_back(task_id);
    // 如果入队前发生了中断，此时wait队列没有该任务，无法改成ready,必须在入队后再次检查是否有数据
    let mut receive_buffer = UART_SERVICE.receive_buffer.lock();
    if !receive_buffer.is_empty() {
        UART_SERVICE.wait_queue.lock().pop_back(); // 撤销入队
        let c = receive_buffer.pop().unwrap();
        ctx.a0 = c as usize;
        return ctx.mepc;
    }
    drop(receive_buffer);
    let mut scheduler = SCHEDULER.lock();
    // 设置为-1代表进入阻塞状态
    ctx.a0 = usize::MAX;
    // 此时timeout_ms > 0
    if timeout_ms < usize::MAX {
        // 带超时：计算唤醒时间并加入定时器堆
        const ONE_MS_CYCLES: usize = RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ / 1000;
        let current_mtime = get_mtime();
        let wake_time = current_mtime + timeout_ms * ONE_MS_CYCLES;

        let next_ctx = scheduler.set_current_task_sleep(wake_time);
        unsafe {
            asm!("csrw mscratch, {}", in(reg) next_ctx);
            return (*next_ctx).mepc;
        }
    } else {
        // 无限阻塞
        unsafe {
            let next_ctx = scheduler.block_current_task();
            asm!("csrw mscratch, {}", in(reg) next_ctx);
            return (*next_ctx).mepc;
        }
    }
}
