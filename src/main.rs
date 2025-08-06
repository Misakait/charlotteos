#![no_std]
#![no_main]
// lazy_static 会用到这个特性
mod driver;
mod bsp;
mod lang_items;
mod console;

// use lang_items::*;
// 使用 core::arch::global_asm! 宏来包含整个汇编文件
use core::arch::global_asm;
use driver::{SerialPort, Uart}; // 引入 Trait 和统一的 Uart 类型
use lazy_static::lazy_static;
use spin::Mutex;

// 这行代码会把 entry.S 的内容直接嵌入到编译流程中
global_asm!(include_str!("entry.S"));

// 使用 lazy_static! 来创建我们唯一的、带锁的 UART 实例。
// 这是整个系统中对物理串口硬件的唯一表示。
lazy_static! {
    static ref UART: Mutex<Uart> = {
        // 这段代码只会在第一次访问 UART 时执行一次
        let mut uart = Uart::new();
        // 在创建的同时就完成初始化
        uart.init();
        Mutex::new(uart)
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_main(){
    println!("Hello from Charlotte OS!");
    loop{};    
}
