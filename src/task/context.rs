#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskContext {
    // x1, 返回地址
    pub ra: usize,
    // x2, 栈指针
    pub sp: usize,
    // x4, 线程指针
    pub tp: usize,

    // x5-x7, 临时寄存器
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,

    // x8-x9, 被调用者保存的寄存器
    pub s0: usize, //fp帧指针寄存器
    pub s1: usize,
    // x10-x17, 参数寄存器
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,

    //x18-x27, 被调用者保存的寄存器
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,

    // x28-x31, 临时寄存器
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,

    pub mepc: usize,
}

impl TaskContext {
    // 创建一个全零的上下文，用于初始化新任务
    pub const fn zero() -> Self {
        Self {
            ra: 0,
            sp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            a0: 0,
            a1: 0,
            a2: 0,
            a3: 0,
            a4: 0,
            a5: 0,
            a6: 0,
            a7: 0,
            t3: 0,
            t4: 0,
            t5: 0,
            t6: 0,
            mepc: 0,
        }
    }
}
