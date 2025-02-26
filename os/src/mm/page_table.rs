//! 页表
//!
//!

use super::{
    address::{PhysAddr, PhysPageNum, StepByOne, VirtPageNum},
    frame_allocator::{frame_alloc, FrameTracker},
    VirtAddr,
};
use alloc::{string::String, vec::Vec};
use bitflags::*;

bitflags! {
    /// 页表项标志
    pub struct PTEFlags: u8 {
        /// 有效标志
        const V = 1 << 0;
        /// 可读标志
        const R = 1 << 1;
        /// 可写标志
        const W = 1 << 2;
        /// 可执行标志
        const X = 1 << 3;
        /// 用户态可访问标志
        const U = 1 << 4;
        const G = 1 << 5;
        /// 已被访问标志
        const A = 1 << 6;
        /// 已被修改标志
        const D = 1 << 7;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
/// 页表项
pub struct PageTableEntry {
    /// SV39分页模式下，\[53:10\]这44位是物理页号，最低的8位是标志位
    pub bits: usize,
}

impl PageTableEntry {
    /// 使用物理页号和标志位创建一个页表项
    pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
        PageTableEntry {
            bits: ppn.0 << 10 | flags.bits as usize,
        }
    }

    /// 创建一个全零的页表项
    pub fn empty() -> Self {
        PageTableEntry { bits: 0 }
    }

    /// 返回页表项对应的物理页号
    pub fn ppn(&self) -> PhysPageNum {
        (self.bits >> 10).into()
    }

    /// 返回页表项对应的标志位
    pub fn flags(&self) -> PTEFlags {
        PTEFlags::from_bits(self.bits as u8).unwrap()
    }

    /// 页表项是否有效
    pub fn is_valid(&self) -> bool {
        (self.flags() & PTEFlags::V) != PTEFlags::empty()
    }

    /// 页面是否可写
    pub fn writable(&self) -> bool {
        (self.flags() & PTEFlags::W) != PTEFlags::empty()
    }

    /// 页面是否可执行
    pub fn executable(&self) -> bool {
        (self.flags() & PTEFlags::X) != PTEFlags::empty()
    }
}

/// （多级）页表
pub struct PageTable {
    /// 根页表所在的物理页号
    root_ppn: PhysPageNum,
    /// 页表所辖的所有物理页框（包括用于存放页表的页框）
    frames: Vec<FrameTracker>,
}

impl PageTable {
    /// 创建一个仅包含根页表及其物理页框的初始页表
    pub fn new() -> Self {
        let frame = frame_alloc().unwrap();
        PageTable {
            root_ppn: frame.ppn,
            frames: vec![frame],
        }
    }

    /// 将虚拟页号`vpn`按标志位`flags`所指定的方式映射到物理页号`ppn`
    ///
    /// # 逻辑概要
    /// 1. 找到或创建`vpn`所对应的页表项
    /// 2. 若此页表项为有效，说明`vpn`已被映射，报错
    /// 3. 以`ppn`和`flags`更新此页表项，并置其有效位
    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        let pte = self.find_pte_create(vpn).unwrap();
        assert!(!pte.is_valid(), "vpn {:?} is mapped before mapping", vpn);
        *pte = PageTableEntry::new(ppn, flags | PTEFlags::V);
    }

    /// 撤销对虚拟页号`vpn`的映射
    ///
    /// # 逻辑概要
    /// 1. 查找`vpn`所对应的页表项
    /// 2. 若找不到或找到的页表项无效，报错
    /// 3. 清空此页表项
    pub fn unmap(&self, vpn: VirtPageNum) {
        let pte = self.find_pte(vpn).unwrap();
        assert!(pte.is_valid(), "vpn {:?} is invalid before unmapping", vpn);
        *pte = PageTableEntry::empty();
    }

    /// 查找`vpn`对应的页表项，若找不到则创建一个新的
    ///
    /// # 逻辑概要
    /// 1. 将`vpn`分解为三级页表的索引
    /// 2. 从根页表开始逐级查找，若找到的一/二级页表项无效则为其分配物理页框并更新
    /// 3. 返回第三级页表项
    fn find_pte_create(&mut self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                let frame = frame_alloc().unwrap();
                *pte = PageTableEntry::new(frame.ppn, PTEFlags::V);
                self.frames.push(frame);
            }
            ppn = pte.ppn();
        }
        result
    }

    /// 查找`vpn`对应的页表项
    ///
    /// # 逻辑概要
    /// 1. 将`vpn`分解为三级页表的索引
    /// 2. 从根页表开始逐级查找，若找到的一/二级页表项无效则返回[`None`]
    /// 3. 返回第三级页表项
    fn find_pte(&self, vpn: VirtPageNum) -> Option<&mut PageTableEntry> {
        let idxs = vpn.indexes();
        let mut ppn = self.root_ppn;
        let mut result: Option<&mut PageTableEntry> = None;
        for (i, idx) in idxs.iter().enumerate() {
            let pte = &mut ppn.get_pte_array()[*idx];
            if i == 2 {
                result = Some(pte);
                break;
            }
            if !pte.is_valid() {
                return None;
            }
            ppn = pte.ppn();
        }
        result
    }

    /// 从`satp`寄存器的值（低44位，物理页号部分）构建根页表
    ///
    /// `frames`成员初始为空，此函数仅用于查表
    pub fn from_token(satp: usize) -> Self {
        Self {
            root_ppn: PhysPageNum::from(satp & ((1usize << 44) - 1)),
            frames: Vec::new(),
        }
    }

    /// 查表找出`vpn`对应的页表项
    ///
    /// # 返回值
    /// 若找不到返回[`None`]
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.find_pte(vpn).map(|pte| *pte)
    }

    /// 查表将虚拟地址`va`转换为物理地址
    ///
    /// # 逻辑概要
    /// 1. 找到`va`所在虚拟页号`vpn`所对应的页表项
    /// 2. 返回其对应的物理页号加页内偏移
    pub fn translate_va(&self, va: VirtAddr) -> Option<PhysAddr> {
        self.find_pte(va.clone().floor()).map(|pte| {
            let aligned_pa: PhysAddr = pte.ppn().into();
            let offset = va.page_offset();
            let aligned_pa_usize: usize = aligned_pa.into();
            (aligned_pa_usize + offset).into()
        })
    }

    /// 返回可直接写入`satp`寄存器的值，写入此值后即开启SV39分页机制
    pub fn token(&self) -> usize {
        8usize << 60 | self.root_ppn.0
    }
}

/// 使用`token`所对应的根页表将来自用户空间的`ptr`并其后`len`个字节转为内核空间的字节[`Vec`]
///
/// # 逻辑概要
/// - 从`ptr`对应的`vpn`开始逐页转换[`PageTable::translate`]
/// - 使用[`PhysPageNum::get_bytes_array`]取得整页数据
/// - 需注意第一页和最后一页的页内偏移
pub fn translated_byte_buffer(token: usize, ptr: *const u8, len: usize) -> Vec<&'static mut [u8]> {
    let page_table = PageTable::from_token(token);
    let mut start = ptr as usize;
    let end = start + len;
    let mut v = Vec::new();
    while start < end {
        let start_va = VirtAddr::from(start);
        let mut vpn = start_va.floor();
        let ppn = page_table.translate(vpn).unwrap().ppn();
        vpn.step();
        let mut end_va: VirtAddr = vpn.into();
        end_va = end_va.min(VirtAddr::from(end));
        if end_va.aligned() {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..]);
        } else {
            v.push(&mut ppn.get_bytes_array()[start_va.page_offset()..end_va.page_offset()]);
        }
        start = end_va.into();
    }
    v
}

/// 使用`token`所对应的根页表将来自用户空间的`ptr`转为内核空间的[`String`]
///
/// # 逻辑概要
/// 使用[`PageTable::translate_va`]逐字节转换
pub fn translated_str(token: usize, ptr: *const u8) -> String {
    let page_table = PageTable::from_token(token);
    let mut string = String::new();
    let mut va = ptr as usize;
    loop {
        let ch: u8 = *(page_table
            .translate_va(VirtAddr::from(va))
            .unwrap()
            .get_ref());
        if ch == 0 {
            break;
        } else {
            string.push(ch as char);
            va += 1;
        }
    }
    string
}

/// 使用`token`所对应的根页表将来自用户空间的`ptr`转为内核空间的可变引用
pub fn translated_refmut<T>(token: usize, ptr: *mut T) -> &'static mut T {
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .get_mut()
}

/// 使用`token`所对应的根页表将来自用户空间的`ptr`转为内核空间的不可变引用
pub fn translated_ref<T>(token: usize, ptr: *const T) -> &'static T {
    let page_table = PageTable::from_token(token);
    let va = ptr as usize;
    page_table
        .translate_va(VirtAddr::from(va))
        .unwrap()
        .get_ref()
}

/// 用户空间Buffer
///
/// 此结构将[`translated_byte_buffer`]返回的结果封装为方便的迭代器模式
///
/// 参见[`UserBufferIterator`]
pub struct UserBuffer {
    /// 存放由[`translated_byte_buffer`]返回的结果
    pub buffers: Vec<&'static mut [u8]>,
}

impl UserBuffer {
    /// 以[`translated_byte_buffer`]返回的结果构造
    pub fn new(buffers: Vec<&'static mut [u8]>) -> Self {
        Self { buffers }
    }

    /// 返回所持有`buffers`的长度总和
    pub fn len(&self) -> usize {
        let mut total: usize = 0;
        for b in self.buffers.iter() {
            total += b.len();
        }
        total
    }
}

impl IntoIterator for UserBuffer {
    type Item = *mut u8;

    type IntoIter = UserBufferIterator;

    fn into_iter(self) -> Self::IntoIter {
        UserBufferIterator {
            buffers: self.buffers,
            current_buffer: 0,
            current_idx: 0,
        }
    }
}

/// [`UserBuffer`]的迭代器
pub struct UserBufferIterator {
    buffers: Vec<&'static mut [u8]>,
    current_buffer: usize,
    current_idx: usize,
}

impl Iterator for UserBufferIterator {
    type Item = *mut u8;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_buffer >= self.buffers.len() {
            None
        } else {
            let r = &mut self.buffers[self.current_buffer][self.current_idx] as *mut _;
            if self.current_idx + 1 == self.buffers[self.current_buffer].len() {
                self.current_idx = 0;
                self.current_buffer += 1;
            } else {
                self.current_idx += 1;
            }
            Some(r)
        }
    }
}
