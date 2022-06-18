use crate::config::*;
use crate::trap::TrapContext;

#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct KernelStack {
    data: [u8; KERNEL_STACK_SIZE],
}

#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct UserStack {
    data: [u8; USER_STACK_SIZE],
}

impl KernelStack {
    pub const fn new() -> Self {
        KernelStack {
            data: [0; KERNEL_STACK_SIZE],
        }
    }

    pub fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + KERNEL_STACK_SIZE
    }

    pub fn push_context(&self, trap_cx: TrapContext) -> usize {
        let trap_cx_ptr = (self.get_sp() - core::mem::size_of::<TrapContext>()) as *mut TrapContext;
        unsafe {
            *trap_cx_ptr = trap_cx;
        }
        trap_cx_ptr as usize
    }
}

impl UserStack {
    pub const fn new() -> Self {
        UserStack {
            data: [0; USER_STACK_SIZE],
        }
    }

    pub fn get_sp(&self) -> usize {
        self.data.as_ptr() as usize + USER_STACK_SIZE
    }
}
