use core::mem::MaybeUninit;

pub struct RingBuffer<T, const N:usize> {
    buffer: [MaybeUninit<T>;N],
    head: usize,
    tail: usize,
    len: usize,
}
impl<T, const N:usize> RingBuffer<T, N> {
    pub const fn new() -> Self {
        Self {
            buffer:[const { MaybeUninit::uninit() }; N],
            head: 0,
            tail: 0,
            len: 0,
        }
    }
    pub fn push(&mut self,item: T) -> Result<(), T> {
        if self.len == N {
            Err(item)
        } else {
            self.buffer[self.tail].write(item);
            self.tail = (self.tail + 1) % N;
            self.len += 1;
            Ok(())
        }
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let old_head = self.head;
        self.head = (self.head + 1) % N;
        self.len -= 1;
        unsafe {
            let value = self.buffer[old_head].assume_init_read();
            Some(value)
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn is_full(&self) -> bool {
        self.len == N
    }
    pub fn capacity(&self) -> usize {
        N
    }
}