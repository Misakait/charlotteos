// src/system.rs

/// 定义了系统级控制行为的 Trait
pub trait SystemControl {
    /// 关闭系统
    fn shutdown(&self) -> !;
    fn reboot(&self) -> !;
}