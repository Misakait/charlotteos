use crate::bsp::get_hart_id;
use crate::bsp::qemu_virt::{LSR, THR, UART_BASE, plic_claim_complete_addr};
use crate::{polling_println, println};
use core::ptr::{read_volatile, write_volatile};

#[derive(Debug)]
pub enum InterruptRequest {
    UART,
    UNKNOWN,
}
impl InterruptRequest {
    pub fn num_to_irq(num: u32) -> InterruptRequest {
        // println!("num to irq: {}", num);

        match num {
            10 => InterruptRequest::UART,
            _ => InterruptRequest::UNKNOWN,
            // _ => {InterruptRequest::UART}
        }
    }
    pub fn to_num(&self) -> u32 {
        match self {
            InterruptRequest::UART => 10,
            InterruptRequest::UNKNOWN => 717,
        }
    }
}
pub struct PLIC {}
impl PLIC {
    pub fn claim() -> u32 {
        // println!("PLIC claim :{:?}",plic_claim_complete_addr(get_hart_id()) as *mut u32);
        unsafe { read_volatile(plic_claim_complete_addr(get_hart_id()) as *mut u32) }
    }
    pub fn complete(irq: u32) {
        // polling_println!("complete_addr {:?}",plic_claim_complete_addr(get_hart_id()) as *mut u32);
        unsafe { write_volatile(plic_claim_complete_addr(get_hart_id()) as *mut u32, irq) }
        // polling_println!("that");
    }
}
