use core::ops::{Deref, DerefMut};

use spin::{Mutex, MutexGuard};

use crate::trap::interrupts::{read_and_disable_machine_interrupts, restore_interrupts};

pub struct IrqLock<T> {
    inner: Mutex<T>,
}
impl<T> IrqLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            inner: Mutex::new(data),
        }
    }
    pub fn lock(&self) -> IrqLockGuard<'_, T> {
        // 需要保存中断状态
        let saved_status = read_and_disable_machine_interrupts();
        let guard = self.inner.lock();
        IrqLockGuard {
            _guard: guard,
            saved_status,
        }
    }
}
unsafe impl<T: Send> Sync for IrqLock<T> {}
unsafe impl<T: Send> Send for IrqLock<T> {}

pub struct IrqLockGuard<'a, T> {
    _guard: MutexGuard<'a, T>, // 保持锁的所有权
    saved_status: usize,       // 保存的中断状态
}
impl<'a, T> Deref for IrqLockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self._guard.deref()
    }
}

impl<'a, T> DerefMut for IrqLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self._guard.deref_mut()
    }
}

// 实现 Drop 以自动恢复中断
impl<'a, T> Drop for IrqLockGuard<'a, T> {
    fn drop(&mut self) {
        // 锁 (_guard) 会在这里先被 drop，释放自旋锁
        // 然后我们恢复中断状态
        restore_interrupts(self.saved_status);
    }
}
