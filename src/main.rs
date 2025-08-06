#![no_std]
#![no_main]

mod lang_items;
// use lang_items::*;
// 使用 core::arch::global_asm! 宏来包含整个汇编文件
use core::arch::global_asm;

// 这行代码会把 entry.S 的内容直接嵌入到编译流程中
global_asm!(include_str!("entry.S"));
#[unsafe(no_mangle)]
pub extern "C" fn rust_main(){
    loop{};    
}
