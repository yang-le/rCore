#![no_std]
#![no_main]
#![feature(panic_info_message)]

#[macro_use]
mod console;
mod config;
mod stack;
mod lang_items;
mod loader;
mod sbi;
mod sync;
mod timer;
pub mod batch;
pub mod syscall;
pub mod task;
pub mod trap;

use core::arch::global_asm;
global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

extern "C" {
    fn stext();
    fn etext();
    fn srodata();
    fn erodata();
    fn sdata();
    fn edata();
    fn sbss();
    fn ebss();
}

#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    
    info!("Hello, world!");
    info!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
    info!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
    info!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
    info!(".bss [{:#x}, {:#x})", sbss as usize, ebss as usize);

    trap::init();
    // batch::init();
    // batch::run_next_app();
    loader::load_apps();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::run_first_task();
    panic!("Unreachable in rust_main!");
}

fn clear_bss() {
    (sbss as usize..ebss as usize).for_each(|a| {
        unsafe { (a as *mut u8).write_volatile(0) }
    })
}