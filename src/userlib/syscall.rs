use core::arch::asm;

pub fn sys_sleep(ms: usize) {
    unsafe {
        asm!(
            "ecall",
            in("a7") 17,
            in("a0") ms,
            options(nostack)
        );
    }
}
pub fn sys_read(ms: usize) -> isize {
    let ret: isize;
    unsafe {
        asm!(
            "ecall",
            in("a7") 27,
            inout("a0") ms => ret,
            options(nostack)
        );
    }
    ret
}
