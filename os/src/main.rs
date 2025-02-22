#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate alloc;

mod lang_items;
mod logging;
mod sbi;

#[macro_use]
mod console;

mod config;
mod loader;
mod mm;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;

#[path = "boards/qemu.rs"]
mod board;

use core::arch::global_asm;
global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

// use log::*;

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    logging::init();
    // info!("Hello, world!");
    mm::init();
    // info!("back to world!");
    // mm::remap_test();
    task::add_initproc();
    trap::init();
    trap::enabled_timer_interrupt();
    timer::set_next_trigger();
    loader::list_apps();
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}
