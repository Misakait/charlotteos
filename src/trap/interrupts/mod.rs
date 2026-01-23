pub mod service;

use crate::bsp::qemu_virt::RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ;
use core::arch::asm;
use sbi_rt::set_timer;
// SIE.STIE 在第 5 位, SIE.SEIE 在第 9 位
const SIE_STIE_MASK: usize = 1 << 5;
const SIE_SEIE_MASK: usize = 1 << 9;
const SSTATUS_SIE_MASK: usize = 1 << 1;
#[derive(Debug)]
pub enum InterruptCause {
    SupervisorTimerInterrupt,
    SupervisorExternalInterrupt,
    Unknown,
}

impl InterruptCause {
    pub fn from_code(code: usize) -> InterruptCause {
        match code {
            5 => InterruptCause::SupervisorTimerInterrupt,
            9 => InterruptCause::SupervisorExternalInterrupt,
            _ => InterruptCause::Unknown,
        }
    }
}

pub unsafe fn init_supervisor_interrupts() {
    unsafe {
        // 使能 S 模式时钟中断
        asm!(
        // 使用 csrrs (Read and Set) 指令
        // 它会将 sie 的值与我们传入的寄存器值进行 OR 操作
        // 第一个操作数 `_` 表示我们不关心 sie 的旧值，所以把它丢弃
        "csrrs {0}, sie, {1}",
        out(reg) _,             // 对应 {0}
        in(reg) SIE_STIE_MASK,  // 对应 {1}，编译器会自动将 SIE_STIE_MASK 放入一个寄存器
        );

        // 使能 S 模式外部中断
        asm!(
        // 使用 csrrs (Read and Set) 指令
        // 它会将 sie 的值与我们传入的寄存器值进行 OR 操作
        // 第一个操作数 `_` 表示我们不关心 sie 的旧值，所以把它丢弃
        "csrrs {0}, sie, {1}",
        out(reg) _,             // 对应 {0}
        in(reg) SIE_SEIE_MASK,  // 对应 {1}，编译器会自动将 SIE_SEIE_MASK 放入一个寄存器
        );

        // 开启 S 模式下的中断总开关 (sstatus.SIE)
        // 此处的2为 1 << 1
        // asm!("csrsi sstatus, 2");
    }
}
pub fn disable_supervisor_interrupts() {
    unsafe {
        // 关闭 S 模式下的中断总开关 (sstatus.SIE)
        asm!("csrci sstatus, 2");
    }
}
#[inline]
pub fn read_and_disable_supervisor_interrupts() -> usize {
    let mut sstatus: usize;
    unsafe {
        // csrrci rd csr imm读取csr到rd并置imm位的bit为零
        asm!("csrrci {}, sstatus, {}", out(reg) sstatus, const 2);
    }
    sstatus
}
#[inline]
pub fn restore_interrupts(saved_status: usize) {
    // 如果之前是开中断的 (SIE=1)，则重新开启
    if (saved_status & SSTATUS_SIE_MASK) != 0 {
        unsafe {
            asm!("csrsi sstatus, {}", const SSTATUS_SIE_MASK);
        }
    }
}
pub fn enable_supervisor_interrupts() {
    unsafe {
        // 开启 S 模式下的中断总开关 (sstatus.SIE)
        asm!("csrsi sstatus, 2");
    }
}
pub fn get_time_ms() -> usize {
    let current_time = get_time();
    const TICKS_PER_MS: usize = RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ as usize / 1000;
    current_time / TICKS_PER_MS
}
pub fn get_time() -> usize {
    let current_time: usize;
    unsafe {
        // asm!("csrr {}, time", out(reg) current_time, options(nostack, readonly));
        asm!("rdtime {}", out(reg) current_time, options(nostack, readonly));
        current_time
    }
}
pub unsafe fn set_next_timer_tick() {
    // 0.01 即 10 毫秒
    const TEN_MS_CYCLES: usize = (RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ / 100) as usize;
    // const TEN_MS_CYCLES: usize = (RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ) as usize;
    let current_time = get_time();
    let target_time = current_time + TEN_MS_CYCLES;
    let _ = set_timer(target_time as u64);
}
