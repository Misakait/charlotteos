use crate::bsp::qemu_virt::{ISR, LSR, RHR, THR, UART_BASE};
use crate::data_struct::ring_buf::RingBuffer;
use crate::{UART, polling_print, polling_println};
use core::fmt::Write;
use core::ptr::{read_volatile, write_volatile};
use spin::mutex::SpinMutex;

const UART_FIFO_CAPACITY: usize = 16;
#[cfg(feature = "uart_interrupt")]
pub static UART_SERVICE: UartService = UartService::new();
#[cfg(feature = "uart_interrupt")]
pub struct UartService {
    pub receive_buffer: SpinMutex<RingBuffer<u8, 4096>>,
    pub transmit_buffer: SpinMutex<RingBuffer<u8, 4096>>,
}
#[cfg(feature = "uart_interrupt")]
impl UartService {
    const fn new() -> Self {
        UartService {
            receive_buffer: SpinMutex::<RingBuffer<u8, 4096>>::new(RingBuffer::<u8, 4096>::new()),
            transmit_buffer: SpinMutex::<RingBuffer<u8, 4096>>::new(RingBuffer::<u8, 4096>::new()),
        }
    }
    pub fn send_data(&self) {
        // polling_println!("send_data");
        let mut tr = self.transmit_buffer.lock();
        // polling_print!("get the lock");
        // let uart = UART.lock();
        // polling_print!("get the uart lock");
        for _ in 0..UART_FIFO_CAPACITY {
            // 尝试从软件缓冲区取出一个字符
            if let Some(character) = tr.pop() {
                let thr_ptr = (UART_BASE + THR) as *mut u8;
                unsafe {
                    write_volatile(thr_ptr, character);
                }
                // uart.write_to_reg(character);
            } else {
                break;
            }
        }
        if tr.is_empty() {
            use crate::bsp::qemu_virt::IER;

            // polling_print!("empty");
            // uart.disable_transmit_interrupt();
            let ier_ptr = (UART_BASE + IER) as *mut u8;
            unsafe {
                let current_ier = read_volatile(ier_ptr);
                write_volatile(ier_ptr, current_ier & !0x02); // 禁用发送中断
            }
            // polling_println!("[send_data] Returning...");
        }
    }
}
const ISR_CAUSE_MASK: u8 = 0b0000_1110; // 我们只关心 Bit 1, 2, 3
const ISR_RX_AVAILABLE: u8 = 0b0000_0100; // RXRDY (接收数据)
const ISR_TX_EMPTY: u8 = 0b0000_0010; // TXRDY (发送空)
const ISR_LINE_STATUS: u8 = 0b0000_0110; // LSR (线路状态)

// 这是你的中断分诊函数
pub fn uart_interrupt_handler() {
    let isr_ptr = (UART_BASE + ISR) as *mut u8;
    let isr_val = unsafe { read_volatile(isr_ptr) };

    // 文档中 Bit 0 的描述: 1 = no interrupt pending
    if (isr_val & 0x01) == 1 {
        return; // 是一个伪中断，直接返回
    }

    // 提取出表示中断原因的比特位 (D3, D2, D1)
    let cause = isr_val & ISR_CAUSE_MASK;
    match cause {
        ISR_TX_EMPTY => {
            #[cfg(feature = "uart_interrupt")]
            UART_SERVICE.send_data();
        }
        ISR_RX_AVAILABLE => {
            // 这是接收中断，【必须】读取 RHR 来清除中断
            let rbr_ptr = (UART_BASE + RHR) as *mut u8;
            unsafe {
                let _received_char = read_volatile(rbr_ptr);
            }
        }
        ISR_LINE_STATUS => {
            // 这是线路状态中断，【必须】读取 LSR 来清除中断
            let lsr_ptr = (UART_BASE + LSR) as *mut u8;
            unsafe {
                let _ = read_volatile(lsr_ptr);
            }
        }
        _ => {}
    }
}
