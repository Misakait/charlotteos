use crate::driver::Uart; // 引入统一的 Uart 类型
use core::fmt::{self, Write};
use crate::UART;

pub fn _print(args: fmt::Arguments) {
    unsafe{
        UART.lock().write_fmt(args).unwrap();
    }
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
