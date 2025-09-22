use core::arch::naked_asm;
use crate::println;
use crate::trap::interrupts::{set_mtimecmp, InterruptCause};

pub mod interrupts;



#[unsafe(naked)]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_entry() {
    naked_asm!(
    // 1. 原子地交换 t6 和 mscratch，安全地获取到当前任务的上下文指针
        //    执行后: t6 = &current_task_ctx, mscratch = old_t6
        "csrrw t6, mscratch, t6",

        // 2. 将当前任务的完整上下文，保存到 t6 指向的 TaskContext 中
        "sd ra, 0(t6)",      // ra  (x1)
        "sd sp, 8(t6)",      // sp  (x2)
        "sd tp, 16(t6)",     // tp  (x4)

        "sd t0, 24(t6)",     // t0  (x5)
        "sd t1, 32(t6)",     // t1  (x6)
        "sd t2, 40(t6)",     // t2  (x7)

        "sd s0, 48(t6)",     // s0  (x8)
        "sd s1, 56(t6)",     // s1  (x9)

        "sd a0, 64(t6)",     // a0 (x10)
        "sd a1, 72(t6)",     // a1 (x11)
        "sd a2, 80(t6)",     // a2 (x12)
        "sd a3, 88(t6)",     // a3 (x13)
        "sd a4, 96(t6)",     // a4 (x14)
        "sd a5, 104(t6)",    // a5 (x15)
        "sd a6, 112(t6)",    // a6 (x16)
        "sd a7, 120(t6)",    // a7 (x17)

        "sd s2, 128(t6)",    // s2 (x18)
        "sd s3, 136(t6)",    // s3 (x19)
        "sd s4, 144(t6)",    // s4 (x20)
        "sd s5, 152(t6)",    // s5 (x21)
        "sd s6, 160(t6)",    // s6 (x22)
        "sd s7, 168(t6)",    // s7 (x23)
        "sd s8, 176(t6)",    // s8 (x24)
        "sd s9, 184(t6)",    // s9 (x25)
        "sd s10, 192(t6)",   // s10 (x26)
        "sd s11, 200(t6)",   // s11 (x27)
        "sd t3, 208(t6)",    // t3 (x28)
        "sd t4, 216(t6)",    // t4 (x29)
        "sd t5, 224(t6)",    // t5 (x30)

        "mv t5, t6",      // 备份 t6 (current_task_ctx) 到 t5
        "csrr t6, mscratch", // 取回旧的 t6 (old_t6)
        "sd t6, 232(t5)",    // 保存 old_t6
        "csrw mscratch, t5", // 恢复 mscratch = &current_task_ctx

        "csrr a0, mepc", // 取出 mepc 到 a0，准备传给中断处理函数
        "csrr a1, mcause", // 取出 mcause 到 a1，准备传给中断处理函数
        "call trap_handler", // 调用中断处理函数

        "csrw mepc, a0", // 将 mepc 恢复回去或者修改

        "csrr a0, mscratch", // 取出 &current_task_ctx 到 a0

        "ld ra, 0(a0)",
        "ld sp, 8(a0)",
        "ld tp, 16(a0)",
        "ld t0, 24(a0)",
        "ld t1, 32(a0)",
        "ld t2, 40(a0)",
        "ld s0, 48(a0)",
        "ld s1, 56(a0)",
        // a0会在最后被恢复
        "ld a1, 72(a0)",
        "ld a2, 80(a0)",
        "ld a3, 88(a0)",
        "ld a4, 96(a0)",
        "ld a5, 104(a0)",
        "ld a6, 112(a0)",
        "ld a7, 120(a0)",
        "ld s2, 128(a0)",
        "ld s3, 136(a0)",
        "ld s4, 144(a0)",
        "ld s5, 152(a0)",
        "ld s6, 160(a0)",
        "ld s7, 168(a0)",
        "ld s8, 176(a0)",
        "ld s9, 184(a0)",
        "ld s10, 192(a0)",
        "ld s11, 200(a0)",
        "ld t3, 208(a0)",
        "ld t4, 216(a0)",
        "ld t5, 224(a0)",
        "ld t6, 232(a0)",
        // 最后恢复 a0 ，因为我们之前一直需要用 a0 作为基地址
        "ld a0, 64(a0)",

        // 6. 执行 ret，跳转到新加载的 ra 地址，完成切换
        "mret"
    );
}

#[derive(Debug)]
pub enum TrapCause {
    Interrupt(InterruptCause),
    Exception(ExceptionCause),
}

#[derive(Debug)]
pub enum ExceptionCause {
    Unknown,
}
pub fn parse_trap_cause(mcause: usize) -> TrapCause {
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1) as usize;
    let is_interrupt = (mcause & INTERRUPT_BIT) != 0;
    let code = mcause & !INTERRUPT_BIT;
    if is_interrupt {
        TrapCause::Interrupt(InterruptCause::from_code(code))
    } else {
        TrapCause::Exception(ExceptionCause::Unknown)
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_handler(mepc : usize, mcause: usize) -> usize {
    println!("mepc: {:#x}, mcause: {:#x}", mepc, mcause);
    match parse_trap_cause(mcause) {
        TrapCause::Interrupt(InterruptCause::MachineTimerInterrupt) => {
            println!("Welcome to Time Interrupt!");
            unsafe{set_mtimecmp();}
        }
        TrapCause::Interrupt(InterruptCause::MachineExternalInterrupt) => {
            // 处理外部中断
        }
        TrapCause::Interrupt(InterruptCause::Unknown) => {
            println!("Unknown interrupt");
        }
        TrapCause::Exception(ExceptionCause::Unknown) => {
            println!("Unknown exception");
        }
    }
    mepc
}