//! 任务管理
//!
//!

mod context;
mod manager;
mod pid;
mod processor;
mod signal;
mod switch;
mod task;

use crate::fs::{open_file, OpenFlags};
use alloc::sync::Arc;
use lazy_static::lazy_static;
use manager::remove_from_pid2task;
use processor::take_current_task;
use task::TaskStatus;

pub use context::TaskContext;
pub use manager::{add_task, pid2task, wakeup_task};
pub use processor::{current_task, current_trap_cx, current_user_token, run_tasks, schedule};
pub use signal::{SignalAction, SignalFlags, MAX_SIG};
pub use task::TaskControlBlock;

lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new({
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        TaskControlBlock::new(v.as_slice())
    });
}

pub fn add_initproc() {
    add_task(INITPROC.clone());
}

pub fn suspend_current_and_run_next() {
    let task = take_current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);

    // push back to ready queue
    add_task(task);
    schedule(task_cx_ptr);
}

pub fn exit_current_and_run_next(exit_code: i32) {
    let task = take_current_task().unwrap();

    remove_from_pid2task(task.getpid());

    let mut inner = task.inner_exclusive_access();
    inner.task_status = TaskStatus::Zombie;
    inner.exit_code = exit_code;

    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }

    inner.children.clear();
    inner.memory_set.recycle_data_pages();

    drop(inner);
    drop(task);

    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

pub fn block_current_task() -> *mut TaskContext {
    let task = take_current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.task_status = TaskStatus::Blocked;
    &mut task_inner.task_cx as *mut TaskContext
}

pub fn block_current_and_run_next() {
    let task_cx_ptr = block_current_task();
    schedule(task_cx_ptr);
}

pub fn current_add_signal(signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.signal_recv |= signal;
}

pub fn handle_signals() {
    loop {
        check_pending_signals();
        let (frozen, killed) = {
            let task = current_task().unwrap();
            let task_inner = task.inner_exclusive_access();
            (task_inner.frozen, task_inner.killed)
        };
        if !frozen || killed {
            break;
        }
        // wait for SIGCONT
        suspend_current_and_run_next();
    }
}

fn check_pending_signals() {
    for sig in 0..(MAX_SIG + 1) {
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let signal = SignalFlags::from_bits(1 << sig).unwrap();
        if task_inner.signal_recv.contains(signal) && (!task_inner.signal_mask.contains(signal)) {
            let mut masked = true;
            let handling_sig = task_inner.handling_sig;
            if handling_sig == -1 {
                masked = false;
            } else {
                let handling_sig = handling_sig as usize;
                // does current handling sigaction mask this signal?
                if !task_inner.signal_actions.table[handling_sig]
                    .mask
                    .contains(signal)
                {
                    masked = false;
                }
            }
            if !masked {
                drop(task_inner);
                drop(task);
                if signal == SignalFlags::SIGKILL
                    || signal == SignalFlags::SIGSTOP
                    || signal == SignalFlags::SIGCONT
                    || signal == SignalFlags::SIGDEF
                {
                    call_kernel_signal_handler(signal);
                } else {
                    call_user_signal_handler(sig, signal);
                    return;
                }
            }
        }
    }
}

fn call_kernel_signal_handler(signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    match signal {
        SignalFlags::SIGSTOP => {
            task_inner.frozen = true;
            task_inner.signal_recv ^= SignalFlags::SIGSTOP;
        }
        SignalFlags::SIGCONT => {
            task_inner.frozen = false;
            task_inner.signal_recv ^= SignalFlags::SIGCONT;
        }
        _ => {
            task_inner.killed = true;
        }
    }
}

fn call_user_signal_handler(sig: usize, signal: SignalFlags) {
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    let handler = task_inner.signal_actions.table[sig].handler;
    if handler != 0 {
        task_inner.handling_sig = sig as isize;
        task_inner.signal_recv ^= signal;

        let trap_ctx = task_inner.get_trap_cx();
        task_inner.trap_ctx_backup = Some(*trap_ctx);
        trap_ctx.sepc = handler;
        trap_ctx.x[10] = sig; // put args (a0)
    } else {
        log::info!("task/call_user_signal_handler: default action: ignore it or kill process");
    }
}

pub fn check_signal_error_of_current() -> Option<(i32, &'static str)> {
    let task = current_task().unwrap();
    let task_inner = task.inner_exclusive_access();
    task_inner.signal_recv.check_error()
}
