// src/bsp/qemu_virt.rs
pub const UART_BASE: usize = 0x10000000;
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