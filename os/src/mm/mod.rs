//! 内存管理
//!
//!

mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

pub use address::{PhysAddr, PhysPageNum, StepByOne, VirtAddr};
pub use frame_allocator::{frame_alloc, frame_dealloc, FrameTracker};
pub use memory_set::{kernel_token, MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{
    translated_byte_buffer, translated_ref, translated_refmut, translated_str, PageTable,
    UserBuffer,
};

/// 内存管理初始化
///
/// # 逻辑概要
/// 1. 堆初始化 [`heap_allocator::init_heap`]
/// 2. 内存页框分配器初始化 [`frame_allocator::init_frame_allocator`]
/// 3. 映射内核地址空间并启用SV39页表
///     1. 映射内核地址空间 [`MemorySet::new_kernel`]
///     2. 启用SV39页表 [`MemorySet::activate`]
pub fn init() {
    heap_allocator::init_heap();
    frame_allocator::init_frame_allocator();
    KERNEL_SPACE.exclusive_access().activate();
}
