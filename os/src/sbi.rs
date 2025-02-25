//! SBI接口
//!
//!

use crate::mm::{kernel_token, PageTable};

pub fn console_putchar(c: usize) {
    sbi_rt::console_write_byte(c as u8);
}

pub fn console_getchar() -> usize {
    let c: [u8; 1] = [0; 1];
    sbi_rt::console_read(sbi_rt::Physical::new(
        1,
        PageTable::from_token(kernel_token())
            .translate_va((c.as_ptr() as usize).into())
            .unwrap()
            .0,
        0,
    ));
    c[0] as usize
}

pub fn shutdown(failure: bool) -> ! {
    use sbi_rt::{system_reset, NoReason, Shutdown, SystemFailure};
    if !failure {
        system_reset(Shutdown, NoReason);
    } else {
        system_reset(Shutdown, SystemFailure);
    }
    unreachable!()
}

pub fn set_timer(timer: usize) {
    sbi_rt::set_timer(timer as _);
}
