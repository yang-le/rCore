use alloc::sync::Arc;

use crate::{
    board::CharDeviceImpl,
    drivers::{
        chardev::UART,
        input::{KEYBOARD_DEVICE, MOUSE_DEVICE},
    },
};

pub fn sys_event_get() -> isize {
    let kb = KEYBOARD_DEVICE.clone();
    let mouse = MOUSE_DEVICE.clone();
    if !kb.is_empty() {
        kb.read_event() as isize
    } else if !mouse.is_empty() {
        mouse.read_event() as isize
    } else {
        0
    }
}

pub fn sys_key_pressed() -> isize {
    let raw = Arc::into_raw(UART.clone());
    let new = unsafe { Arc::from_raw(raw.cast::<CharDeviceImpl>()) };
    let res = !new.read_buffer_is_empty();
    if res {
        1
    } else {
        0
    }
}
