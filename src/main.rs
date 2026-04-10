#![no_std]
#![no_main]
extern crate alloc;

mod bsp;
mod config;
mod console;
mod data_struct;
mod driver;
mod lang_items;
mod mm;
mod syslib;
mod system;
mod task;
mod trap;
mod userlib;

use crate::bsp::qemu_virt::UART_BASE;
use crate::config::PHYS_VIRT_OFFSET;
use crate::mm::buddy::{phys_to_virt, virt_to_phys};
use alloc::vec::Vec;

use crate::mm::{
    enable_early_mmu, enable_virtual_memory, init_buddy_system, setup_memory_and_mapping,
    unmap_temp_identity_area,
};
use crate::task::SCHEDULER;
use crate::task::context::TaskContext;
use crate::task::scheduler::Scheduler;
use crate::trap::interrupts::{init_supervisor_interrupts, set_next_timer_tick};
use crate::trap::trap_entry;
use crate::userlib::syscall::{sys_read, sys_shutdown, sys_sleep, sys_task_exit};
use core::arch::{asm, global_asm};
use core::slice;
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
        let mut uart = Uart::new(UART_BASE);
        // 在创建的同时就完成初始化
        uart.init();
        Mutex::new(uart)
    };
}
static mut KERNEL_INIT_CONTEXT: TaskContext = TaskContext::zero();

#[unsafe(no_mangle)]
pub extern "C" fn rust_main(hart_id: usize, dtb_addr: usize) {
    // core::arch::asm!("csrs sstatus, {0}", in(reg) 1 << 13);
    // 清空 BSS 段的工作在汇编完成
    // clear_bss();
    unsafe {
        enable_early_mmu();

        let next_fn_virt_addr = phys_to_virt(virt_rust_main as fn(usize) as *const () as usize);

        core::arch::asm!(
            "add sp, sp, {offset}",
            "add s0, s0, {offset}",
            "jr {target}",
            offset = in(reg) PHYS_VIRT_OFFSET,
            target = in(reg) next_fn_virt_addr,
            in("a0") dtb_addr,
            options(noreturn)
        );
    }
    // unreachable!();
}

fn virt_rust_main(dtb_addr: usize) {
    setup_memory_and_mapping(dtb_addr);
    enable_virtual_memory();
    unmap_temp_identity_area();
    init_buddy_system();
    sbi_println!("Buddy System Allocator initialized");
    unsafe {
        asm!("csrw sscratch, {}", in(reg) &raw mut KERNEL_INIT_CONTEXT);
        let stvec_addr = (trap_entry as usize) & !0x3;
        asm!("csrw stvec, {}", in(reg) stvec_addr);
    }
    // println!("vec ptr: {:#X}", vec.as_ptr() as *const usize as usize);
    // polling_println!("polling");
    sbi_println!("Hello from Charlotte OS!");
    // 初始化调度器并创建 idle 任务
    let _ = Scheduler::init();
    sbi_println!("✓ Scheduler initialized with idle task");

    // 创建测试任务
    {
        let mut scheduler = SCHEDULER.lock();
        scheduler
            .spawn(test_task_a, 8192, 1)
            .expect("Failed to spawn task A");
        scheduler
            .spawn(test_task_b, 8192, 1)
            .expect("Failed to spawn task B");
        scheduler
            .spawn(shell, 8192, 1)
            .expect("Failed to spawn task shell");
    } // 锁在这里释放

    sbi_println!("All tasks created. Starting scheduler...");
    sbi_println!("======================================================");
    unsafe {
        set_next_timer_tick();
        init_supervisor_interrupts();
    }

    // sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
    Scheduler::run_scheduler();

    // loop {}
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::NoReason);
}

// #[unsafe(no_mangle)]
fn test_task_a() {
    user_println!("[Task A] ✓ Start!");
    // let status: usize;
    // unsafe { asm!("csrr {}, sstatus", out(reg) status) }
    // polling_println!("task a sstatus: {:b}", status);
    let mut a = 0;
    for _ in 0..10 {
        // 模拟一些工作负载
        for _ in 0..100000 {
            // println!("[Task A] num is:{} ", a);
            a += 1;
            core::hint::spin_loop();
        }
    }
    // println!("[Task A] ✓ Finished!");
    user_println!("[Task A] ✓ Finished!");
    // sys_task_exit();
}
fn test_task_b() {
    // println!("[Task B] ✓ Start!");
    user_println!("[Task B] ✓ Start!");
    let mut b = 0;
    for _ in 0..10 {
        // 模拟一些工作负载
        for _ in 0..100000 {
            b += 1;
            core::hint::spin_loop();
        }
    }
    sys_sleep(10000);
    // println!("[Task B] ✓ Finished!");
    user_println!("[Task B] ✓ Finished!");
    sys_shutdown();
    // sys_task_exit();
}
fn shell() {
    // println!("shell Start!");
    user_println!("shell Start!");
    // loop {
    let char = sys_read(5000);
    if char > 0 {
        // println!("read a char:{},ascii {}", char as u8 as char, char);
        // user_println!("read a char:{},ascii {}", char as u8 as char, char);
        user_println!("read a ,ascii {}", char);
        // break;
    }
    // }
    // println!("shell ✓ Finished!");
    user_println!("shell ✓ Finished!");
    // sys_task_exit();
}
