//! Rust语言支持
//!
//!

use crate::{println, sbi::shutdown, task::current_task};
use core::{arch::asm, panic::PanicInfo};
use log::*;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!(
            "Panicked at {}:{} {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        error!("Panicked: {}", info.message().unwrap());
    }
    unsafe {
        backtrace();
    }
    shutdown(true);
}

unsafe fn backtrace() {
    if let Some(current_task) = current_task() {
        let mut fp: usize;
        let stop = current_task.kernel_stack.get_top();
        asm!("mv {}, s0", out(reg) fp);
        println!("Backtrace:");
        for i in 0..32 {
            if fp == stop {
                break;
            }
            println!("#{}: ra={:#x}", i, *((fp - 8) as *const usize));
            fp = *((fp - 16) as *const usize);
        }
    }
}
