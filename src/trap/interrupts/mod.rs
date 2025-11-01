pub mod service;

use core::arch::asm;
use crate::bsp::qemu_virt::{get_mtimecmp_addr, MTIME_ADDR, RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ};
//因为mie.MTIE在第七位, 1 << 7 = 128
const MIE_MTIE_MASK: usize = 1 << 7;
const MIE_MEIE_MASK: usize = 1 << 11;
#[derive(Debug)]
pub enum InterruptCause {
    MachineTimerInterrupt,
    MachineExternalInterrupt,
    Unknown,
}

impl InterruptCause {
    pub fn from_code(code: usize) -> InterruptCause {
        match code {
            7 => InterruptCause::MachineTimerInterrupt,
            11 => InterruptCause::MachineExternalInterrupt,
            _ => InterruptCause::Unknown,
        }
    }
}

pub unsafe fn init_machine_interrupts() {
    unsafe {
        // 开启 M 模式下的中断总开关 (mstatus.MIE)
        //此处的8为1 << 3
        asm!("csrsi mstatus, 8");
        //使能时钟中断

        asm!(
        // 使用 csrrs (Read and Set) 指令
        // 它会将 mie 的值与我们传入的寄存器值进行 OR 操作
        // 第一个操作数 `_` 表示我们不关心 mie 的旧值，所以把它丢弃
        "csrrs {0}, mie, {1}",
        out(reg) _,             // 对应 {0}
        in(reg) MIE_MTIE_MASK,  // 对应 {1}，编译器会自动将 MIE_MTIE_MASK 放入一个寄存器
        );
        //使能外部中断
        asm!(
        // 使用 csrrs (Read and Set) 指令
        // 它会将 mie 的值与我们传入的寄存器值进行 OR 操作
        // 第一个操作数 `_` 表示我们不关心 mie 的旧值，所以把它丢弃
        "csrrs {0}, mie, {1}",
        out(reg) _,             // 对应 {0}
        in(reg) MIE_MEIE_MASK,  // 对应 {1}，编译器会自动将 MIE_MEIE_MASK 放入一个寄存器
        );
    }
}
pub fn disable_machine_interrupts() {
    unsafe {
        // 关闭 M 模式下的中断总开关 (mstatus.MIE)
        asm!("csrci mstatus, 8");
    }
}
pub fn enable_machine_interrupts() {
    unsafe {
        // 开启 M 模式下的中断总开关 (mstatus.MIE)
        asm!("csrsi mstatus, 8");
    }
}
pub unsafe fn set_mtimecmp(){
    unsafe {
        //0.01即10毫秒
        const TEN_MS_CYCLES: usize = (RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ as f64 * 1.0) as usize;

        // 1. 读取当前mtime值（64位）
        let current_mtime: usize;
        asm!(
        "ld {0}, 0({1})",          // 从mtime地址加载64位值到寄存器
        out(reg) current_mtime,    // 输出：当前mtime值
        in(reg) MTIME_ADDR,             // 输入：mtime的内存地址
        options(nostack, readonly) // 选项：不使用栈，只读操作
        );

        // 2. 计算目标时间：当前时间 + 10ms周期数
        let target_mtime = current_mtime + TEN_MS_CYCLES;
        const MTIMECMP_ADDR: usize = get_mtimecmp_addr(0);
        // 3. 将目标时间写入mtimecmp（64位）
        asm!(
        "sd {0}, 0({1})",          // 将64位值存储到mtimecmp地址
        in(reg) target_mtime,      // 输入：目标时间值
        in(reg) MTIMECMP_ADDR,          // 输入：mtimecmp的内存地址
        options(nostack)           // 选项：不使用栈
        );

    }
}