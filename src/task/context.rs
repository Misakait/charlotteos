#[repr(C)]
pub(crate) struct TaskContext{
    ra: usize, // 返回地址 (Return Address)
    sp: usize, // 栈指针 (Stack Pointer)
    s0: usize, // Saved Register 0 (通常是 Frame Pointer)
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
}
impl TaskContext {
    // 创建一个全零的上下文，用于初始化新任务
    pub const fn zero() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s0: 0, s1: 0, s2: 0, s3: 0, s4: 0, s5: 0,
            s6: 0, s7: 0, s8: 0, s9: 0, s10: 0, s11: 0,
        }
    }
}