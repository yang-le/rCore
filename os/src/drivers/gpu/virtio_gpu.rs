use virtio_drivers::{VirtIOGpu, VirtIOHeader};

use crate::{board::virtio_mmio_bus_addr, drivers::bus::virtio::VirtioHal, sync::UPIntrFreeCell};

use super::GpuDevice;

pub struct VirtIOGpuWrapper {
    gpu: UPIntrFreeCell<VirtIOGpu<'static, VirtioHal>>,
    fb: &'static [u8],
}

impl GpuDevice for VirtIOGpuWrapper {
    fn get_framebuffer(&self) -> &mut [u8] {
        unsafe {
            let ptr = self.fb.as_ptr() as *const _ as *mut u8;
            core::slice::from_raw_parts_mut(ptr, self.fb.len())
        }
    }

    fn flush(&self) {
        self.gpu.exclusive_access().flush().unwrap();
    }
}

impl VirtIOGpuWrapper {
    pub fn new() -> Self {
        unsafe {
            let mut virtio =
                VirtIOGpu::<VirtioHal>::new(&mut *(virtio_mmio_bus_addr(1) as *mut VirtIOHeader))
                    .unwrap();
            let fbuffer = virtio.setup_framebuffer().unwrap();
            let len = fbuffer.len();
            let ptr = fbuffer.as_mut_ptr();
            let fb = core::slice::from_raw_parts_mut(ptr, len);
            Self {
                gpu: UPIntrFreeCell::new(virtio),
                fb,
            }
        }
    }
}
