use alloc::collections::vec_deque::VecDeque;
use virtio_drivers::{VirtIOHeader, VirtIOInput};

use crate::{
    drivers::bus::virtio::VirtioHal,
    sync::{Condvar, UPIntrFreeCell},
    task::schedule,
};

use super::InputDevice;

struct VirtIOInputInner {
    virtio_input: VirtIOInput<'static, VirtioHal>,
    events: VecDeque<u64>,
}

pub struct VirtIOInputWrapper<const BASE_ADDR: usize> {
    inner: UPIntrFreeCell<VirtIOInputInner>,
    condvar: Condvar,
}

impl<const BASE_ADDR: usize> VirtIOInputWrapper<BASE_ADDR> {
    pub fn new() -> Self {
        let inner = VirtIOInputInner {
            virtio_input: unsafe {
                VirtIOInput::<VirtioHal>::new(&mut *(BASE_ADDR as *mut VirtIOHeader)).unwrap()
            },
            events: VecDeque::new(),
        };
        Self {
            inner: unsafe { UPIntrFreeCell::new(inner) },
            condvar: Condvar::new(),
        }
    }
}

impl<const BASE_ADDR: usize> InputDevice for VirtIOInputWrapper<BASE_ADDR> {
    fn read_event(&self) -> u64 {
        loop {
            let mut inner = self.inner.exclusive_access();
            if let Some(event) = inner.events.pop_front() {
                return event;
            } else {
                let task_cx_ptr = self.condvar.wait_no_sched();
                drop(inner);
                schedule(task_cx_ptr);
            }
        }
    }

    fn handle_irq(&self) {
        let mut count = 0;
        let mut result = 0;
        self.inner.exclusive_session(|inner| {
            inner.virtio_input.ack_interrupt();
            while let Some(event) = inner.virtio_input.pop_pending_event() {
                count += 1;
                result = (event.event_type as u64) << 48
                    | (event.code as u64) << 32
                    | (event.value) as u64;
                inner.events.push_back(result);
            }
        });
        if count > 0 {
            self.condvar.signal();
        }
    }

    fn is_empty(&self) -> bool {
        self.inner.exclusive_access().events.is_empty()
    }
}
