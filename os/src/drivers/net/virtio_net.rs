use virtio_drivers::{VirtIOHeader, VirtIONet};

use crate::{board::virtio_mmio_bus_addr, drivers::bus::virtio::VirtioHal, sync::UPIntrFreeCell};

use super::NetDevice;

pub struct VirtIONetWrapper(UPIntrFreeCell<VirtIONet<'static, VirtioHal>>);

impl NetDevice for VirtIONetWrapper {
    fn transmit(&self, data: &[u8]) {
        self.0
            .exclusive_access()
            .send(data)
            .expect("can't send data")
    }

    fn receive(&self, data: &mut [u8]) -> usize {
        self.0
            .exclusive_access()
            .recv(data)
            .expect("can't receive data")
    }
}

impl VirtIONetWrapper {
    pub fn new() -> Self {
        unsafe {
            let virtio =
                VirtIONet::<VirtioHal>::new(&mut *(virtio_mmio_bus_addr(4) as *mut VirtIOHeader))
                    .expect("can't create net device by virtio");
            VirtIONetWrapper(UPIntrFreeCell::new(virtio))
        }
    }
}
