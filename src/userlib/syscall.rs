use core::arch::asm;

const SYS_WRITE_BYTE: usize = 1;
const SYS_SLEEP: usize = 17;
const SYS_READ: usize = 27;

pub fn sys_sleep(ms: usize) {
    unsafe {
        asm!(
            "ecall",
            in("a7") SYS_SLEEP,
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
            in("a7") SYS_READ,
            inout("a0") ms => ret,
            options(nostack)
        );
    }
    ret
    // if ret < 0 || ret > 0x7f {
    //     -1
    // } else {
    //     ret
    // }
}

pub fn sys_write_byte(byte: u8) {
    unsafe {
        asm!(
            "ecall",
            in("a7") SYS_WRITE_BYTE,
            in("a0") byte as usize,
            options(nostack)
        );
    }
}
