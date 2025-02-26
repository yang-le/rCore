//! [rCore](https://rcore-os.cn/rCore-Tutorial-Book-v3) 操作系统
//!
//! # 启动流程
//! 系统上电后，[SBI](https://github.com/riscv-software-src/opensbi)将加载本程序至0x80200000处，并跳转到此处运行。
//! 0x80200000处为汇编程序`entry.asm`：
//!
//! ```assembly
//!     .section .text.entry
//!     .globl _start
//! _start:
//!     la sp, boot_stack_top
//!     call rust_main
//!     
//!     .section .bss.stack
//!     .globl boot_stack_lower_bound
//! boot_stack_lower_bound:
//!     .space 4096 * 16
//!     .globl boot_stack_top
//! boot_stack_top:
//! ```
//!
//! 可以看到`entry.asm`中设置好栈指针`sp`后即直接调用[`rust_main`]进入rust程序。
//!
//! # 内存布局
//! 内核的内存布局由链接脚本`linker-qemu.ld`指定:
//!
//! ```ld
//! OUTPUT_ARCH(riscv)
//! ENTRY(_start)
//! BASE_ADDRESS = 0x80200000;
//!
//! SECTIONS
//! {
//!     . = BASE_ADDRESS;
//!     skernel = .;
//!
//!     stext = .;
//!     .text : {
//!         *(.text.entry)
//!         . = ALIGN(4K);
//!         strampoline = .;
//!         *(.text.trampoline);
//!         . = ALIGN(4K);
//!         *(.text .text.*)
//!     }
//!
//!     . = ALIGN(4K);
//!     etext = .;
//!     srodata = .;
//!     .rodata : {
//!         *(.rodata .rodata.*)
//!         *(.srodata .srodata.*)
//!     }
//!
//!     . = ALIGN(4K);
//!     erodata = .;
//!     sdata = .;
//!     .data : {
//!         *(.data .data.*)
//!         *(.sdata .sdata.*)
//!     }
//!
//!     . = ALIGN(4K);
//!     edata = .;
//!     sbss_with_stack = .;
//!     .bss : {
//!         *(.bss.stack)
//!         sbss = .;
//!         *(.bss .bss.*)
//!         *(.sbss .sbss.*)
//!     }
//!
//!     . = ALIGN(4K);
//!     ebss = .;
//!     ekernel = .;
//!
//!     /DISCARD/ : {
//!         *(.eh_frame)
//!     }
//! }
//! ```
//!
//! 脚本依次指定了代码段、只读数据段、数据段、BSS段（包括内核栈和未初始化/初始化为零的数据），内核于BSS段后结束。
//!
//! 可以使用`nm -C os | awk '$2 == "B" || $2 == "b" {print}'`查看这些区段中的变量。
//! 其中`T|t`为代码段，`R|r`为只读数据段，`D|d`为数据段，`B|b`为BSS段。
//!
//! 脚本还设定了内核的入口为[`entry.asm`](#启动流程)中的`_start`，并指定了系统架构`riscv`和内核起始地址`BASE_ADDRESS`等参数。

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
mod drivers;
mod fs;
mod mm;
mod sync;
mod syscall;
mod task;
mod timer;
mod trap;

#[path = "boards/qemu.rs"]
mod board;

use core::arch::global_asm;

use drivers::chardev::{CharDevice, UART};
global_asm!(include_str!("entry.asm"));

use lazy_static::lazy_static;
use sync::UPIntrFreeCell;

lazy_static! {
    /// 是否启用非阻塞块设备访问
    pub static ref DEV_NON_BLOCKING_ACCESS: UPIntrFreeCell<bool> =
        unsafe { UPIntrFreeCell::new(false) };
}

/// 系统入口函数
///
/// # 逻辑概要
/// 1. 清零`bss` [`clear_bss`]
/// 2. 内存管理初始化 [`mm::init`]
/// 3. 陷入机制初始化 [`trap::init`]
/// 4. 时钟初始化
///     1. [`trap::enabled_timer_interrupt`]
///     2. [`timer::set_next_trigger`]
/// 5. 运行第一个任务
///     1. [`task::add_initproc`]
///     2. [`task::run_tasks`]
///
/// # 返回值
/// 此函数不返回
#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    logging::init();
    mm::init();
    UART.init();
    trap::init();
    trap::enabled_timer_interrupt();
    timer::set_next_trigger();
    board::device_init();
    fs::list_apps();
    task::add_initproc();
    *DEV_NON_BLOCKING_ACCESS.exclusive_access() = true;
    task::run_tasks();
    panic!("Unreachable in rust_main!");
}

/// 清零`bss`段
fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    (sbss as usize..ebss as usize).for_each(|a| unsafe { (a as *mut u8).write_volatile(0) });
}
