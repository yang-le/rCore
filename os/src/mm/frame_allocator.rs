//! 页框分配器
//!
//! 使用一段指定的物理空间为内核分配物理页框

use super::address::{PhysAddr, PhysPageNum};
use crate::{config::MEMORY_END, sync::UPSafeCell};
use alloc::vec::Vec;
use core::fmt::{self, Debug, Formatter};
use lazy_static::lazy_static;

/// 页框分配器接口
trait FrameAllocator {
    fn new() -> Self;
    /// 分配一个物理页
    fn alloc(&mut self) -> Option<PhysPageNum>;
    /// 回收一个物理页
    fn dealloc(&mut self, ppn: PhysPageNum);
}

/// 简易栈式页框分配器
pub struct StackFrameAllocator {
    /// 当前可用物理页号
    current: usize,
    /// 最大可用物理页号
    end: usize,
    /// 回收栈
    recycled: Vec<usize>,
}

impl FrameAllocator for StackFrameAllocator {
    fn new() -> Self {
        Self {
            current: 0,
            end: 0,
            recycled: Vec::new(),
        }
    }

    /// 分配一个物理页
    ///
    /// # 逻辑概要
    /// 如果回收栈中有页框可用，返回之；
    /// 否则若当前可用物理页号已达到最大可用物理页号，分配失败；
    /// 否则从当前可用物理页处分配一个，并更新当前可用物理页号。
    ///
    /// # 返回值
    /// 返回分配的物理页号，若分配失败返回[`None`]
    fn alloc(&mut self) -> Option<PhysPageNum> {
        if let Some(ppn) = self.recycled.pop() {
            Some(ppn.into())
        } else if self.current == self.end {
            None
        } else {
            self.current += 1;
            Some((self.current - 1).into())
        }
    }

    /// 回收一个物理页
    ///
    /// # 逻辑概要
    /// 检查`ppn`的有效性，确保其不是未分配的或已经回收的；
    /// 然后将其放入回收栈中。
    ///
    /// # 参数
    /// * `ppn` - 要回收的物理页号
    fn dealloc(&mut self, ppn: PhysPageNum) {
        let ppn = ppn.0;
        // validty check
        if ppn >= self.current || self.recycled.iter().any(|v| *v == ppn) {
            panic!("Frame ppn={:#x} has not been allocated!", ppn)
        }
        // recycle
        self.recycled.push(ppn);
    }
}

impl StackFrameAllocator {
    /// 初始化页框分配器
    ///
    /// # 参数
    /// * `l` - 可用起始物理页号
    /// * `r` - 可用终止物理页号
    pub fn init(&mut self, l: PhysPageNum, r: PhysPageNum) {
        self.current = l.0;
        self.end = r.0;
    }
}

/// 实现了[页框分配器](FrameAllocator)的类
type FrameAllocatorImpl = StackFrameAllocator;
lazy_static! {
    /// 全局页框分配器
    pub static ref FRAME_ALLOCATOR: UPSafeCell<FrameAllocatorImpl> =
        UPSafeCell::new(FrameAllocatorImpl::new());
}

/// 初始化[全局页框分配器](`struct@FRAME_ALLOCATOR`)
///
/// 将从内核结束（[上取整页](PhysAddr::ceil)）到最大内存[`MEMORY_END`]（[下取整页](PhysAddr::floor)）的这段空间交给页框分配器
pub fn init_frame_allocator() {
    extern "C" {
        fn ekernel();
    }
    FRAME_ALLOCATOR.exclusive_access().init(
        PhysAddr::from(ekernel as usize).ceil(),
        PhysAddr::from(MEMORY_END).floor(),
    );
}

/// 使用[全局页框分配器](`struct@FRAME_ALLOCATOR`)分配一个物理页
pub fn frame_alloc() -> Option<FrameTracker> {
    FRAME_ALLOCATOR
        .exclusive_access()
        .alloc()
        .map(FrameTracker::new)
}

/// 使用[全局页框分配器](`struct@FRAME_ALLOCATOR`)回收一个物理页
/// # 参数
/// * `ppn` - 要回收的物理页号
pub fn frame_dealloc(ppn: PhysPageNum) {
    FRAME_ALLOCATOR.exclusive_access().dealloc(ppn);
}

/// 页框追踪器
///
/// [构造](`FrameTracker::new`)时清零页框内容
///
/// [析构](`FrameTracker::drop`)时自动回收页框
pub struct FrameTracker {
    /// 被管理的物理页号
    pub ppn: PhysPageNum,
}

impl Debug for FrameTracker {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("FrameTracker: PPN={:#x}", self.ppn.0))
    }
}

impl FrameTracker {
    /// 会额外将管理的物理页清零
    pub fn new(ppn: PhysPageNum) -> Self {
        // page cleaning
        let bytes_array = ppn.get_bytes_array();
        for i in bytes_array {
            *i = 0;
        }
        Self { ppn }
    }
}

impl Drop for FrameTracker {
    /// 回收管理的物理页
    fn drop(&mut self) {
        frame_dealloc(self.ppn);
    }
}

#[doc(hidden)]
#[allow(unused)]
pub fn frame_allocator_test() {
    use log::*;
    let mut v: Vec<FrameTracker> = Vec::new();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        print!("{:?}", frame);
        v.push(frame);
    }
    v.clear();
    for i in 0..5 {
        let frame = frame_alloc().unwrap();
        print!("{:?}", frame);
        v.push(frame);
    }
    drop(v);
    info!("frame_allocator_test passed!");
}
