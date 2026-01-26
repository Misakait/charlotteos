use core::cell::{Ref, RefCell, RefMut};

pub struct SyncRefCell<T> {
    inner: RefCell<T>,
}

unsafe impl<T> Sync for SyncRefCell<T> {}

impl<T> SyncRefCell<T> {
    pub const unsafe fn new(value: T) -> Self {
        Self {
            inner: RefCell::new(value),
        }
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.inner.borrow_mut()
    }

    pub fn borrow(&self) -> Ref<'_, T> {
        self.inner.borrow()
    }
}
