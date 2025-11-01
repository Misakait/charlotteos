use core::arch::asm;

// src/bsp/mod.rs
pub mod qemu_virt;

#[inline]
pub fn get_hart_id() -> usize {
    let tp: usize;
    unsafe {
        asm!{
        "mv {}, tp", out(reg) tp
        }
    }
    tp
}