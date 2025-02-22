use crate::trap::trap_return;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct TaskContext {
    /// return address
    ra: usize,
    /// kernel stack pointer of app
    sp: usize,
    /// callee saved registers: s0..s11
    s: [usize; 12],
}

impl TaskContext {
    pub fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    /// Set task context with ra = trap_return, sp = `kstack_ptr`
    pub fn goto_trap_return(kstack_ptr: usize) -> Self {
        Self {
            ra: trap_return as usize,
            sp: kstack_ptr,
            s: [0; 12],
        }
    }
}
