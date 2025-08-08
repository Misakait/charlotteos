use core::panic::PanicInfo;
use core::ptr::write_volatile;
use crate::bsp::qemu_virt::{FINISHER_FAIL, VIRT_TEST_ADDR};
use crate::println;

const STATUS_CODE: u32 = 7;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    let shutdown_cmd: u32 = (STATUS_CODE << 16) | (FINISHER_FAIL as u32);
    unsafe{
        write_volatile(VIRT_TEST_ADDR as *mut u32,shutdown_cmd);
    }
    loop{}
}
