use alloc::{collections::vec_deque::VecDeque, vec::Vec};
use lazy_static::lazy_static;
use lose_net_stack::IPv4;

use crate::sync::UPIntrFreeCell;

pub struct Socket {
    pub raddr: IPv4,
    pub lport: u16,
    pub rport: u16,
    pub buffers: VecDeque<Vec<u8>>,
    pub seq: u32,
    pub ack: u32,
}

lazy_static! {
    static ref SOCKET_TABLE: UPIntrFreeCell<Vec<Option<Socket>>> =
        unsafe { UPIntrFreeCell::new(Vec::new()) };
}

pub fn get_seq_ack_by_index(index: usize) -> Option<(u32, u32)> {
    let socket_table = SOCKET_TABLE.exclusive_access();
    assert!(index < socket_table.len());
    socket_table
        .get(index)
        .and_then(|x| x.as_ref().map(|x| (x.seq, x.ack)))
}

pub fn set_seq_ack_by_index(index: usize, seq: u32, ack: u32) {
    let mut socket_table = SOCKET_TABLE.exclusive_access();
    assert!(socket_table.len() > index);
    assert!(socket_table[index].is_some());
    let socket = socket_table[index].as_mut().unwrap();
    socket.ack = ack;
    socket.seq = seq;
}

pub fn get_socket(raddr: IPv4, lport: u16, rport: u16) -> Option<usize> {
    let socket_table = SOCKET_TABLE.exclusive_access();
    socket_table.iter().enumerate().find_map(|(i, socket)| {
        if socket.is_some() {
            let socket = socket.as_ref().unwrap();
            if socket.raddr == raddr && socket.lport == lport && socket.rport == rport {
                return Some(i);
            }
        }
        None
    })
}

pub fn add_socket(raddr: IPv4, lport: u16, rport: u16) -> Option<usize> {
    if get_socket(raddr, lport, rport).is_some() {
        return None;
    }
    let mut socket_table = SOCKET_TABLE.exclusive_access();
    let index =
        socket_table.iter().enumerate().find_map(
            |(i, socket)| {
                if socket.is_none() {
                    Some(i)
                } else {
                    None
                }
            },
        );
    let socket = Socket {
        raddr,
        lport,
        rport,
        buffers: VecDeque::new(),
        seq: 0,
        ack: 0,
    };
    if index.is_none() {
        socket_table.push(Some(socket));
        Some(socket_table.len() - 1)
    } else {
        socket_table[index.unwrap()] = Some(socket);
        index
    }
}

pub fn remove_socket(index: usize) {
    let mut socket_table = SOCKET_TABLE.exclusive_access();
    assert!(socket_table.len() > index);
    socket_table[index] = None;
}

pub fn push_data(index: usize, data: Vec<u8>) {
    let mut socket_table = SOCKET_TABLE.exclusive_access();
    assert!(socket_table.len() > index);
    assert!(socket_table[index].is_some());
    socket_table[index]
        .as_mut()
        .unwrap()
        .buffers
        .push_back(data);
}

pub fn pop_data(index: usize) -> Option<Vec<u8>> {
    let mut socket_table = SOCKET_TABLE.exclusive_access();
    assert!(socket_table.len() > index);
    assert!(socket_table[index].is_some());
    socket_table[index].as_mut().unwrap().buffers.pop_front()
}
