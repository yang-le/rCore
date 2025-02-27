use crate::drivers::chardev::UART;

use super::File;

pub struct Stdin;
pub struct Stdout;

impl File for Stdin {
    fn readable(&self) -> bool {
        true
    }

    fn writable(&self) -> bool {
        false
    }

    fn read(&self, mut buf: crate::mm::UserBuffer) -> usize {
        assert_eq!(buf.len(), 1);
        let ch = UART.read();
        unsafe {
            buf.buffers[0].as_mut_ptr().write_volatile(ch);
        }
        1
    }

    fn write(&self, _buf: crate::mm::UserBuffer) -> usize {
        panic!("Cannot write to stdin!");
    }
}

impl File for Stdout {
    fn readable(&self) -> bool {
        false
    }

    fn writable(&self) -> bool {
        true
    }

    fn read(&self, _buf: crate::mm::UserBuffer) -> usize {
        panic!("Connot read from stdout!");
    }

    fn write(&self, buf: crate::mm::UserBuffer) -> usize {
        for buffer in buf.buffers.iter() {
            print!("{}", core::str::from_utf8(buffer).unwrap());
        }
        buf.len()
    }
}
