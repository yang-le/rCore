mod heap_allocator;
mod address;
mod page_table;
mod frame_allocator;
mod memory_set;

pub use memory_set::{MemorySet, KERNEL_SPACE, MapPermission};
pub use address::{PhysPageNum, VirtAddr};
pub use page_table::translated_byte_buffer;

pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activete();
}
