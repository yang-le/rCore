mod virtio_gpu;

use core::any::Any;

use alloc::sync::Arc;
use lazy_static::lazy_static;
pub use virtio_gpu::VirtIOGpuWrapper;

use crate::board::GpuDeviceImpl;

pub trait GpuDevice: Send + Sync + Any {
    // fn update_cursor(&self);
    #[warn(clippy::mut_from_ref)]
    fn get_framebuffer(&self) -> &mut [u8];
    fn flush(&self);
}

lazy_static! {
    pub static ref GPU_DEVICE: Arc<dyn GpuDevice> = Arc::new(GpuDeviceImpl::new());
}
