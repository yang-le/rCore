mod virtio_net;

use core::any::Any;

use alloc::sync::Arc;
use lazy_static::lazy_static;
pub use virtio_net::VirtIONetWrapper;

use crate::board::NetDeviceImpl;

pub trait NetDevice: Send + Sync + Any {
    fn transmit(&self, data: &[u8]);
    fn receive(&self, data: &mut [u8]) -> usize;
}

lazy_static! {
    pub static ref NET_DEVICE: Arc<dyn NetDevice> = Arc::new(NetDeviceImpl::new());
}
