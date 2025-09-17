// src/bsp/qemu_virt.rs
use crate::system::SystemControl;
use core::ptr::write_volatile;
pub const UART_BASE: usize = 0x10_000_000;
pub const CLINT_BASE: usize = 0x2_000_000;
pub const MTIME_OFFSET: usize = 0xBFF8;
// pub const MTIME_OFFSET: usize = 0x7FF8;
pub const MTIME_ADDR: usize = CLINT_BASE + MTIME_OFFSET;
pub const MTIMECMP_OFFSET: usize = 0x4000 ;
pub const MTIMECMP_BASE: usize = CLINT_BASE + MTIMECMP_OFFSET;
pub const fn get_mtimecmp_addr(hart_id: i8) -> usize {
    MTIMECMP_BASE + (hart_id * 8) as usize
}
//默认时钟频率为 10MHz
pub const RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ: usize = 10_000_000;

pub const RHR: usize = 0; //Receive Holding Register (read mode)
// Transmit Holding Register (write mode)
pub const THR :usize = 0;
// LSB of Divisor Latch (write mode)
pub const DLL :usize = 0;
// Interrupt Enable Register (write mode)
pub const IER :usize = 1;
// MSB of Divisor Latch (write mode)
pub const DLM :usize = 1;
// FIFO Control Register (write mode)
pub const FCR :usize = 2;
// Interrupt Status Register (read mode)
pub const ISR :usize = 2;
// Line Control Register
pub const LCR :usize = 3;
// Modem Control Register
pub const MCR :usize = 4;
// Line Status Register
pub const LSR :usize = 5;
// Modem Status Register
pub const MSR :usize = 6;
// ScratchPad Register
pub const SPR :usize = 7;

// QEMU virt 平台的 TEST 设备地址
pub const VIRT_TEST_ADDR: usize = 0x100000;
pub const FINISHER_FAIL: u16 = 0x3333;
pub const FINISHER_PASS: u16 = 0x5555;
pub const FINISHER_RESET: u16 = 0x7777;
// 创建一个代表 QEMU 平台的空结构体
pub struct QemuVirt;

impl SystemControl for QemuVirt {
    /// 实现 QEMU 的关机功能
    fn shutdown(&self) -> ! {
        // 向 TEST 设备的特定寄存器写入一个值来关闭 QEMU
        // 0x5555 是一个约定的“成功退出”代码
        unsafe {
            let addr = VIRT_TEST_ADDR as *mut u32;
            write_volatile(addr, FINISHER_PASS as u32);
        }
        // 如果上面的代码成功，程序不会执行到这里
        // 如果失败了，就进入死循环
        loop {}
    }

    fn reboot(&self) -> ! {
        unsafe {
            let addr = VIRT_TEST_ADDR as *mut u32;
            write_volatile(addr, FINISHER_RESET as u32);
        }
        loop {}
    }
    
}