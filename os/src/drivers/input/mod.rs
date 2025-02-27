mod virtio_input;

use core::any::Any;

use alloc::sync::Arc;
use lazy_static::lazy_static;
pub use virtio_input::VirtIOInputWrapper;

use crate::board::{KeyboardDeviceImpl, MouseDeviceImpl};

pub trait InputDevice: Send + Sync + Any {
    fn read_event(&self) -> u64;
    fn handle_irq(&self);
    fn is_empty(&self) -> bool;
}

lazy_static! {
    pub static ref KEYBOARD_DEVICE: Arc<dyn InputDevice> = Arc::new(KeyboardDeviceImpl::new());
    pub static ref MOUSE_DEVICE: Arc<dyn InputDevice> = Arc::new(MouseDeviceImpl::new());
}
