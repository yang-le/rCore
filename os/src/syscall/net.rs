use alloc::sync::Arc;
use lose_net_stack::IPv4;

use crate::{
    net::{
        net_interrupt_handle,
        port::{accept, listen, port_acceptable, PortFd},
        udp::UDP,
    },
    task::{current_process, current_task, current_trap_cx},
};

pub fn sys_connect(raddr: u32, lport: u16, rport: u16) -> isize {
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    let fd = inner.alloc_fd();
    let udp_node = UDP::new(IPv4::from_u32(raddr), lport, rport);
    inner.fd_table[fd] = Some(Arc::new(udp_node));
    fd as isize
}

pub fn sys_listen(port: u16) -> isize {
    match listen(port) {
        Some(port_index) => {
            let process = current_process();
            let mut inner = process.inner_exclusive_access();
            let fd = inner.alloc_fd();
            let port_fd = PortFd::new(port_index);
            inner.fd_table[fd] = Some(Arc::new(port_fd));
            port_index as isize
        }
        None => -1,
    }
}

pub fn sys_accept(port_index: usize) -> isize {
    let task = current_task().unwrap();
    accept(port_index, task);
    loop {
        net_interrupt_handle();
        if !port_acceptable(port_index) {
            break;
        }
    }
    let cx = current_trap_cx();
    cx.x[10] as isize
}
