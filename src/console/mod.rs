use crate::UART;
use crate::bsp::qemu_virt::{LSR, THR, UART_BASE};
// use crate::driver::Uart; // 引入统一的 Uart 类型
use core::fmt::{self, Write};
use core::ptr::{read_volatile, write_volatile};

pub fn _print(args: fmt::Arguments) {
    UART.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}
fn polling_putchar(c: u8) {
    let lsr_ptr = (UART_BASE + LSR) as *mut u8;
    let thr_ptr = (UART_BASE + THR) as *mut u8;
    unsafe {
        // 等待发送保持寄存器为空
        while (read_volatile(lsr_ptr) & (1 << 5)) == 0 {}
        // 写入数据
        write_volatile(thr_ptr, c);
    }
}

// 2. 定义一个内部使用的打印函数，它接收格式化参数
#[doc(hidden)]
pub fn _polling_print(args: fmt::Arguments) {
    // 定义一个临时的、实现了 `fmt::Write` 的结构体
    struct PollingWriter;

    impl Write for PollingWriter {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            for byte in s.bytes() {
                polling_putchar(byte);
            }
            Ok(())
        }
    }

    // 使用这个临时的写入器来处理格式化参数
    PollingWriter.write_fmt(args).unwrap();
}

/// 一个基于轮询的、可以在任何上下文（包括中断中）安全使用的打印宏。
#[macro_export]
macro_rules! polling_print {
    ($($arg:tt)*) => ($crate::console::_polling_print(format_args!($($arg)*)));
}

/// 与 polling_print 类似，但在末尾添加一个换行符。
#[macro_export]
macro_rules! polling_println {
    () => ($crate::polling_print!("\n"));
    ($($arg:tt)*) => ($crate::polling_print!("{}\n", format_args!($($arg)*)));
}
