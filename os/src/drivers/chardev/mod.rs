mod ns16550a;

use core::any::Any;

use alloc::sync::Arc;
use lazy_static::lazy_static;
pub use ns16550a::NS16550a;

use crate::board::CharDeviceImpl;

pub trait CharDevice: Send + Sync + Any {
    fn init(&self);
    fn read(&self) -> u8;
    fn write(&self, ch: u8);
    fn handle_irq(&self);
}

lazy_static! {
    pub static ref UART: Arc<dyn CharDevice> = Arc::new(CharDeviceImpl::new());
}
