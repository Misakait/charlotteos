// src/driver/mod.rs

// 1. 声明 `traits` 文件是一个私有子模块
mod traits;

// 2. 使用 `pub use` 将 `traits` 模块里的所有公共内容，
//    重新导出为 `driver` 模块自身的公共内容。
pub use traits::*;

// 3. 根据 feature 开关，继续声明具体的实现子模块
#[cfg(feature = "uart_polling")]
mod uart_polling;
#[cfg(feature = "uart_polling")]
pub use uart_polling::UartPolling as Uart;

