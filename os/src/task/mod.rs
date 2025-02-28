//! 任务管理
//!
//!

mod context;
mod id;
mod manager;
mod process;
mod processor;
mod signal;
mod switch;
mod task;

use crate::{
    fs::{open_file, OpenFlags},
    sbi::shutdown,
};
use alloc::{sync::Arc, vec::Vec};
use id::{TaskUserRes, IDLE_PID};
use lazy_static::lazy_static;
use log::*;
use manager::{remove_from_pid2task, remove_task};
use processor::take_current_task;

pub use context::TaskContext;
pub use manager::{add_task, pid2process, wakeup_task};
pub use process::ProcessControlBlock;
pub use processor::{
    current_kstack_top, current_process, current_task, current_trap_cx, current_trap_cx_user_va,
    current_user_token, run_tasks, schedule,
};
pub use signal::{SignalAction, SignalFlags, MAX_SIG};
pub use task::TaskControlBlock;
pub use task::TaskStatus;

lazy_static! {
    pub static ref INITPROC: Arc<ProcessControlBlock> = {
        let inode = open_file("initproc", OpenFlags::RDONLY).unwrap();
        let v = inode.read_all();
        ProcessControlBlock::new(v.as_slice())
    };
}

pub fn add_initproc() {
    let _initproc = INITPROC.clone();
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
    let mut task_inner = task.inner_exclusive_access();
    let process = task.process.upgrade().unwrap();
    let tid = task_inner.res.as_ref().unwrap().tid;
    task_inner.exit_code = Some(exit_code);
    task_inner.res = None;
    drop(task_inner);
    drop(task);

    if tid == 0 {
        let pid = process.getpid();
        if pid == IDLE_PID {
            warn!("Idle process exit with exit_code {} ...", exit_code);
            if exit_code != 0 {
                shutdown(true);
            } else {
                shutdown(false);
            }
        }
        remove_from_pid2task(pid);
        let mut process_inner = process.inner_exclusive_access();
        process_inner.is_zombie = true;
        process_inner.exit_code = exit_code;

        {
            let mut initproc_inner = INITPROC.inner_exclusive_access();
            for child in process_inner.children.iter() {
                child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
                initproc_inner.children.push(child.clone());
            }
        }

        let mut recycle_res = Vec::<TaskUserRes>::new();
        for task in process_inner.tasks.iter().filter(|t| t.is_some()) {
            let task = task.as_ref().unwrap();
            remove_inactive_task(Arc::clone(task));
            let mut task_inner = task.inner_exclusive_access();
            if let Some(res) = task_inner.res.take() {
                recycle_res.push(res);
            }
        }

        drop(process_inner);
        // this requires access to process_inner, so drop it first
        recycle_res.clear();

        let mut process_inner = process.inner_exclusive_access();
        process_inner.children.clear();
        process_inner.memory_set.recycle_data_pages();
        process_inner.fd_table.clear();
        while process_inner.tasks.len() > 1 {
            process_inner.tasks.pop();
        }
    }
    drop(process);

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

pub fn remove_inactive_task(task: Arc<TaskControlBlock>) {
    remove_task(Arc::clone(&task));
    // remove_timer(Arc::clone(&task));
}

pub fn current_add_signal(signal: SignalFlags) {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.signal_recv |= signal;
}

pub fn handle_signals() {
    loop {
        check_pending_signals();
        let (frozen, killed) = {
            let process = current_process();
            let process_inner = process.inner_exclusive_access();
            (process_inner.frozen, process_inner.killed)
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
        let process = current_process();
        let process_inner = process.inner_exclusive_access();
        let signal = SignalFlags::from_bits(1 << sig).unwrap();
        if process_inner.signal_recv.contains(signal)
            && (!process_inner.signal_mask.contains(signal))
        {
            let mut masked = true;
            let handling_sig = process_inner.handling_sig;
            if handling_sig == -1 {
                masked = false;
            } else {
                let handling_sig = handling_sig as usize;
                // does current handling sigaction mask this signal?
                if !process_inner.signal_actions[handling_sig]
                    .mask
                    .contains(signal)
                {
                    masked = false;
                }
            }
            if !masked {
                drop(process_inner);
                drop(process);
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
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    match signal {
        SignalFlags::SIGSTOP => {
            process_inner.frozen = true;
            process_inner.signal_recv ^= SignalFlags::SIGSTOP;
        }
        SignalFlags::SIGCONT => {
            process_inner.frozen = false;
            process_inner.signal_recv ^= SignalFlags::SIGCONT;
        }
        _ => {
            process_inner.killed = true;
        }
    }
}

fn call_user_signal_handler(sig: usize, signal: SignalFlags) {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let handler = process_inner.signal_actions[sig].handler;
    if handler != 0 {
        process_inner.handling_sig = sig as isize;
        process_inner.signal_recv ^= signal;

        let task = process_inner.tasks[0].as_ref().unwrap();
        let trap_ctx = task.inner_exclusive_access().get_trap_cx();
        process_inner.trap_ctx_backup = Some(*trap_ctx);
        trap_ctx.sepc = handler;
        trap_ctx.x[10] = sig; // put args (a0)
    } else {
        log::info!("task/call_user_signal_handler: default action: ignore it or kill process");
    }
}

pub fn check_signal_error_of_current() -> Option<(i32, &'static str)> {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    process_inner.signal_recv.check_error()
}
