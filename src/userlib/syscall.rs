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
