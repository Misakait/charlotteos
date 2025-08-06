// src/driver/uart_polling.rs

// 使用 `super` 关键字来引用父模块（`driver` 模块）中的 `SerialPort` Trait。
// 因为 `driver/mod.rs` 已经将 Trait 公开，所以这里可以轻松地找到它。
use super::SerialPort;

// 从 bsp 获取基地址
use crate::bsp::qemu_virt::UART_BASE; 
use core::ptr::{read_volatile, write_volatile};

const RHR: usize = 0; //Receive Holding Register (read mode)
// Transmit Holding Register (write mode)
const THR :usize = 0;
// LSB of Divisor Latch (write mode)
const DLL :usize = 0;
// Interrupt Enable Register (write mode)
const IER :usize = 1;
// MSB of Divisor Latch (write mode)
const DLM :usize = 1;
// FIFO Control Register (write mode)
const FCR :usize = 2;
// Interrupt Status Register (read mode)
const ISR :usize = 2;
// Line Control Register
const LCR :usize = 3;
// Modem Control Register
const MCR :usize = 4;
// Line Status Register
const LSR :usize = 5;
// Modem Status Register
const MSR :usize = 6;
// ScratchPad Register
const SPR :usize = 7;


pub struct UartPolling {
    base_address: usize,
}

impl UartPolling {
    pub fn new() -> Self {
        Self {
            base_address: UART_BASE,
        }
    }
}

// 现在，编译器就能在这里正确地找到 `SerialPort` Trait 了
impl SerialPort for UartPolling {
    fn init(&mut self) {
        let ier_ptr = (self.base_address + IER) as *mut u8;
        let fcr_ptr = (self.base_address + FCR) as *mut u8;
        let lcr_ptr = (self.base_address + LCR) as *mut u8;
        let dll_ptr = (self.base_address + DLL) as *mut u8;
        let dlm_ptr = (self.base_address + DLM) as *mut u8;
        unsafe {
            // 禁用中断
            write_volatile(ier_ptr, 0x00);
            // 开启 FIFO, 清空 FIFO
            write_volatile(fcr_ptr, (1 << 0) | (1 << 1) | (1 << 2));
            let lcr_value = read_volatile(lcr_ptr);
            write_volatile(lcr_ptr, lcr_value | (1 << 7)); // 置位LCR的第7位（DLAB=1），进入波特率配置模式
            write_volatile(dll_ptr, 0x03);
            write_volatile(dlm_ptr, 0x00); // 设置波特率为38400
            //数据格式设置为 “8 位数据位、1 位停止位、无校验”
            write_volatile(lcr_ptr, 0x03 as u8);
        }
    }

    fn putchar(&mut self, c: u8) {
        let lsr_ptr = (self.base_address + LSR) as *mut u8;
        let thr_ptr = (self.base_address + THR) as *mut u8;

        unsafe {
            while (read_volatile(lsr_ptr) & (1 << 5)) == 0 {}
            write_volatile(thr_ptr, c);
        }
    }

    fn getchar(&mut self) -> Option<u8> {
        let lsr_ptr = (self.base_address + LSR) as *mut u8;
        let rhr_ptr = (self.base_address + RHR) as *mut u8;
        
        unsafe {
            if (read_volatile(lsr_ptr) & 1) == 0 {
                None
            } else {
                Some(read_volatile(rhr_ptr))
            }
        }
    }
}

// 我们之前为 UartPolling 实现的 fmt::Write Trait 也需要在这里引入 SerialPort
use core::fmt::{Error, Write};

impl Write for UartPolling {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        for c in s.bytes() {
            // 这里调用的是 SerialPort Trait 定义的 putchar 方法
            self.putchar(c);
        }
        Ok(())
    }
}
