// src/driver/trait.rs
pub trait SerialPort {
    fn init(&mut self);
    fn putchar(&mut self, c: u8);
    fn getchar(&mut self) -> Option<u8>;
}
