// src/driver/uart

// 使用 `super` 关键字来引用父模块（`driver` 模块）中的 `SerialPort` Trait。
// 因为 `driver/mod.rs` 已经将 Trait 公开，所以这里可以轻松地找到它。
use super::SerialPort;
#[cfg(feature = "uart_interrupt")]
use crate::trap::interrupts::service::uart_service::UART_SERVICE;
// 从 bsp 获取基地址
use crate::bsp::qemu_virt::{
    DLL, DLM, FCR, IER, LCR, LSR, RHR, THR, UART_BASE, UART0_IRQ, plic_context_addr,
    plic_enable_addr, plic_priority_addr,
};
use core::ptr::{read_volatile, write_volatile};

pub struct Uart {
    base_address: usize,
}

impl Uart {
    pub fn new(base_address: usize) -> Self {
        Self { base_address }
    }
}
#[cfg(feature = "uart_polling")]
// 现在，编译器就能在这里正确地找到 `SerialPort` Trait 了
impl SerialPort for Uart {
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
            write_volatile(lcr_ptr, 0x03u8);
        }
    }

    fn putchar(&mut self, c: u8) -> Result<(), u8> {
        let lsr_ptr = (self.base_address + LSR) as *mut u8;
        let thr_ptr = (self.base_address + THR) as *mut u8;

        unsafe {
            while (read_volatile(lsr_ptr) & (1 << 5)) == 0 {}
            write_volatile(thr_ptr, c);
        }
        Ok(())
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
// #[cfg(feature = "uart_interrupt")]
// static RECEIVE_BUFFER: SpinMutex<RingBuffer<u8, 4096>>  = SpinMutex::<RingBuffer<u8, 4096>>::new(RingBuffer::<u8,4096>::new());
// #[cfg(feature = "uart_interrupt")]
// static TRANSMIT_BUFFER: SpinMutex<RingBuffer<u8, 4096>>  = SpinMutex::<RingBuffer<u8, 4096>>::new(RingBuffer::<u8,4096>::new());
#[cfg(feature = "uart_interrupt")]
impl SerialPort for Uart {
    fn init(&mut self) {
        let ier_ptr = (self.base_address + IER) as *mut u8;
        let fcr_ptr = (self.base_address + FCR) as *mut u8;
        let lcr_ptr = (self.base_address + LCR) as *mut u8;
        let dll_ptr = (self.base_address + DLL) as *mut u8;
        let dlm_ptr = (self.base_address + DLM) as *mut u8;
        let priority_ptr = plic_priority_addr(UART0_IRQ) as *mut u32;
        let hart_id = get_hart_id();
        let enable_ptr = plic_enable_addr(hart_id, UART0_IRQ) as *mut u32;
        let threshold_ptr = plic_context_addr(hart_id) as *mut u32;
        unsafe {
            // 禁用中断
            write_volatile(ier_ptr, 0x00);
            // 配置 PLIC
            write_volatile(priority_ptr, 1); // 设置 UART0 中断优先级为 1
            let current_enable = read_volatile(enable_ptr);
            write_volatile(enable_ptr, current_enable | (1 << UART0_IRQ)); // 使能 UART0 中断
            write_volatile(threshold_ptr, 0); // 设置上下文阈值为 0，允许所有优先级的中断
            //配置UART
            // 开启 FIFO, 清空 FIFO
            write_volatile(fcr_ptr, (1 << 0) | (1 << 1) | (1 << 2));
            let lcr_value = read_volatile(lcr_ptr);
            write_volatile(lcr_ptr, lcr_value | (1 << 7)); // 置位LCR的第7位（DLAB=1），进入波特率配置模式
            write_volatile(dll_ptr, 0x03);
            write_volatile(dlm_ptr, 0x00); // 设置波特率为38400
            //数据格式设置为 “8 位数据位、1 位停止位、无校验”
            write_volatile(lcr_ptr, 0x03u8);
        }
    }

    fn putchar(&mut self, c: u8) -> Result<(), u8> {
        // polling_println!("puchar!");
        self.disable_transmit_interrupt();
        let res = UART_SERVICE.transmit_buffer.lock().push(c);
        self.enable_transmit_interrupt();
        res
    }

    fn getchar(&mut self) -> Option<u8> {
        //TODO
        None
    }
}
#[cfg(feature = "uart_interrupt")]
impl Uart {
    pub fn disable_receive_interrupt(&self) {
        let ier_ptr = (self.base_address + IER) as *mut u8;
        unsafe {
            let current_ier = read_volatile(ier_ptr);
            write_volatile(ier_ptr, current_ier & !0x01); // 禁用接收中断
        }
    }
    fn enable_receive_interrupt(&self) {
        let ier_ptr = (self.base_address + IER) as *mut u8;
        unsafe {
            let current_ier = read_volatile(ier_ptr);
            write_volatile(ier_ptr, current_ier | 0x01); // 使能接收中断
        }
    }
    #[inline(always)]
    pub fn disable_transmit_interrupt(&self) {
        let ier_ptr = (self.base_address + IER) as *mut u8;
        // polling_println!("[DISABLE] Trying to write...");
        unsafe {
            let current_ier = read_volatile(ier_ptr);
            write_volatile(ier_ptr, current_ier & !0x02); // 禁用发送中断
        }
        // polling_println!("[DISABLE] Write successful!");
    }
    #[inline(always)]
    fn enable_transmit_interrupt(&self) {
        let ier_ptr = (self.base_address + IER) as *mut u8;
        unsafe {
            let current_ier = read_volatile(ier_ptr);
            write_volatile(ier_ptr, current_ier | 0x02); // 使能发送中断
        }
    }
    #[inline(always)]
    pub fn write_to_reg(&self, c: u8) {
        let thr_ptr = (self.base_address + THR) as *mut u8;
        unsafe {
            write_volatile(thr_ptr, c);
        }
    }
}

use crate::bsp::get_hart_id;
use crate::data_struct::ring_buf::RingBuffer;

use crate::{polling_print, polling_println};
use core::fmt::{Error, Write};
use spin::mutex::SpinMutex;

impl Write for Uart {
    fn write_str(&mut self, s: &str) -> Result<(), Error> {
        for c in s.bytes() {
            // 这里调用的是 SerialPort Trait 定义的 putchar 方法
            while let Err(_) = self.putchar(c) {}
        }

        Ok(())
    }
}
