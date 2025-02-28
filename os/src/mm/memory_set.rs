//! 内存空间
//!
//!

use core::arch::asm;

use crate::{
    board::MMIO,
    config::{MEMORY_END, PAGE_SIZE, TRAMPOLINE},
    sync::UPIntrFreeCell,
};
use alloc::{collections::btree_map::BTreeMap, sync::Arc, vec::Vec};
use bitflags::bitflags;
use lazy_static::lazy_static;
use riscv::register::satp;

use super::{
    address::*,
    frame_allocator::{frame_alloc, FrameTracker},
    page_table::{PTEFlags, PageTable, PageTableEntry},
};

/// 一块被映射的内存区域
pub struct MapArea {
    /// 被映射的虚拟页号范围
    vpn_range: VPNRange,
    /// 此内存区域关联的物理页框
    data_frames: BTreeMap<VirtPageNum, FrameTracker>,
    /// 映射类型
    map_type: MapType,
    /// 页面权限
    map_perm: MapPermission,
}

/// 映射类型
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MapType {
    /// 恒等映射
    Identical,
    /// 分配页框的映射
    Framed,
}

bitflags! {
    /// 页面权限
    pub struct MapPermission: u8 {
        /// 可读权限
        const R = 1 << 1;
        /// 可写权限
        const W = 1 << 2;
        /// 可执行权限
        const X = 1 << 3;
        /// 用户态可访问权限
        const U = 1 << 4;
    }
}

/// 地址空间
///
/// 一组被映射的内存区域
pub struct MemorySet {
    /// 根页表，将其物理页号写入`satp`寄存器后即开启分页模式
    page_table: PageTable,
    /// 此地址空间下的所有内存区域
    areas: Vec<MapArea>,
}

impl MemorySet {
    /// 创建一个新的地址空间
    ///
    /// 此函数仅分配根页表对应的物理页
    pub fn new_bare() -> Self {
        Self {
            page_table: PageTable::new(),
            areas: Vec::new(),
        }
    }

    /// 返回[`PageTable::token`]
    pub fn token(&self) -> usize {
        self.page_table.token()
    }

    /// 向地址空间中加入内存映射区域`map_area`并向其中复制数据`data`
    ///
    /// # 逻辑概要
    /// 1. 建立对`map_area`区域的映射[`MapArea::map`]
    /// 2. 如果`data`非[`None`]，复制数据到映射好的区域中[`MapArea::copy_data`]
    /// 3. 将此区域插入[`MemorySet::areas`]
    fn push(&mut self, mut map_area: MapArea, data: Option<&[u8]>) {
        map_area.map(&mut self.page_table);
        if let Some(data) = data {
            map_area.copy_data(&self.page_table, data);
        }
        self.areas.push(map_area);
    }

    /// 向地址空间中插入分配页框的映射区域
    ///
    /// # 逻辑概要
    /// 1. 先以给定的参数构造[`MapArea`]
    /// 2. 调用[`MemorySet::push`]
    pub fn insert_framed_area(
        &mut self,
        start_va: VirtAddr,
        end_va: VirtAddr,
        permission: MapPermission,
    ) {
        self.push(
            MapArea::new(start_va, end_va, MapType::Framed, permission),
            None,
        );
    }

    /// 构造内核的地址空间
    ///
    /// # 逻辑概要
    /// 1. 创建一个新的地址空间
    /// 2. 映射跳板区(RX)[`MemorySet::map_trampoline`]
    /// 3. 以恒等映射分别映射内核的代码区(RX)、只读数据区(R)、数据区(RW)和`BSS`区域(RW)[`MemorySet::push`]
    /// 4. 恒等映射内核结束到内存结束的所有物理内存(RW)
    /// 5. 恒等映射`MMIO`区域(RW)
    pub fn new_kernel() -> Self {
        use log::*;
        extern "C" {
            fn stext();
            fn etext();
            fn srodata();
            fn erodata();
            fn sdata();
            fn edata();
            fn sbss_with_stack();
            fn ebss();
            fn ekernel();
        }

        let mut memory_set = Self::new_bare();
        memory_set.map_trampoline();

        debug!(".text [{:#x}, {:#x})", stext as usize, etext as usize);
        debug!(".rodata [{:#x}, {:#x})", srodata as usize, erodata as usize);
        debug!(".data [{:#x}, {:#x})", sdata as usize, edata as usize);
        debug!(
            ".bss [{:#x}, {:#x})",
            sbss_with_stack as usize, ebss as usize
        );

        trace!("mapping .text section");
        memory_set.push(
            MapArea::new(
                (stext as usize).into(),
                (etext as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::X,
            ),
            None,
        );
        trace!("mapping .rodata section");
        memory_set.push(
            MapArea::new(
                (srodata as usize).into(),
                (erodata as usize).into(),
                MapType::Identical,
                MapPermission::R,
            ),
            None,
        );
        trace!("mapping .data section");
        memory_set.push(
            MapArea::new(
                (sdata as usize).into(),
                (edata as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        trace!("mapping .bss section");
        memory_set.push(
            MapArea::new(
                (sbss_with_stack as usize).into(),
                (ebss as usize).into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        trace!("mapping physical memory");
        memory_set.push(
            MapArea::new(
                (ekernel as usize).into(),
                MEMORY_END.into(),
                MapType::Identical,
                MapPermission::R | MapPermission::W,
            ),
            None,
        );
        trace!("mapping memory-mapped registers");
        for pair in MMIO {
            memory_set.push(
                MapArea::new(
                    pair.0.into(),
                    (pair.0 + pair.1).into(),
                    MapType::Identical,
                    MapPermission::R | MapPermission::W,
                ),
                None,
            );
        }
        memory_set
    }

    /// 从`ELF`数据构造地址空间
    ///
    /// # 逻辑概要
    /// 1. 创建一个新的地址空间
    /// 2. 映射跳板区(RX) [`MemorySet::map_trampoline`]
    /// 3. 解析`ELF`各段的权限并进行映射和数据复制 [`MemorySet::push`]
    ///
    /// # 返回值
    /// 返回构造的地址空间，用户栈基址和程序入口
    pub fn from_elf(elf_data: &[u8]) -> (Self, usize, usize) {
        let mut memory_set = Self::new_bare();
        memory_set.map_trampoline();
        let elf = xmas_elf::ElfFile::new(elf_data).unwrap();
        let elf_header = elf.header;
        let magic = elf_header.pt1.magic;
        assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "Invlaid elf!");
        let ph_count = elf_header.pt2.ph_count();
        let mut max_end_vpn = VirtPageNum(0);
        for i in 0..ph_count {
            let ph = elf.program_header(i).unwrap();
            if ph.get_type().unwrap() == xmas_elf::program::Type::Load {
                let start_va: VirtAddr = (ph.virtual_addr() as usize).into();
                let end_va: VirtAddr = ((ph.virtual_addr() + ph.mem_size()) as usize).into();
                let mut map_perm = MapPermission::U;
                let ph_flags = ph.flags();
                if ph_flags.is_read() {
                    map_perm |= MapPermission::R;
                }
                if ph_flags.is_write() {
                    map_perm |= MapPermission::W;
                }
                if ph_flags.is_execute() {
                    map_perm |= MapPermission::X;
                }
                let map_area = MapArea::new(start_va, end_va, MapType::Framed, map_perm);
                max_end_vpn = map_area.vpn_range.get_end();
                memory_set.push(
                    map_area,
                    Some(&elf.input[ph.offset() as usize..(ph.offset() + ph.file_size()) as usize]),
                );
            }
        }
        let max_end_va: VirtAddr = max_end_vpn.into();
        let mut user_stack_base: usize = max_end_va.into();
        user_stack_base += PAGE_SIZE;
        (
            memory_set,
            user_stack_base,
            elf.header.pt2.entry_point() as usize,
        )
    }

    /// 从已存在的用户空间构造
    /// # 逻辑概要
    /// 1. 创建一个新的地址空间
    /// 2. 映射跳板区(RX) [`MemorySet::map_trampoline`]
    /// 3. 从`user_space`的[`MemorySet::areas`]中
    ///     1. 逐个构造`area`并向新空间[`push`](`MemorySet::push`)
    ///     2. 从[`MapArea::vpn_range`]中逐个转为物理页号并进行数据复制
    pub fn from_existed_user(user_space: &MemorySet) -> MemorySet {
        let mut memory_set = Self::new_bare();
        memory_set.map_trampoline();
        for area in user_space.areas.iter() {
            let new_area = MapArea::from_another(area);
            memory_set.push(new_area, None);

            // copy data from another space
            for vpn in area.vpn_range {
                let src_ppn = user_space.translate(vpn).unwrap().ppn();
                let dst_ppn = memory_set.translate(vpn).unwrap().ppn();
                dst_ppn
                    .get_bytes_array()
                    .copy_from_slice(src_ppn.get_bytes_array());
            }
        }
        memory_set
    }

    /// 映射跳板区(RX)
    ///
    /// 注意此区域虽为内核代码段中的固定区域，但不是恒等映射，
    /// 也不是由[全局页框分配器](`struct@super::frame_allocator::FRAME_ALLOCATOR`)分配的，
    /// 故不使用[`MapArea`]构造，而是直接调用[`PageTable::map`]来映射。
    fn map_trampoline(&mut self) {
        extern "C" {
            fn strampoline();
        }

        self.page_table.map(
            VirtAddr::from(TRAMPOLINE).into(),
            PhysAddr::from(strampoline as usize).into(),
            PTEFlags::R | PTEFlags::X,
        );
    }

    /// 激活地址空间
    ///
    /// 将[`MemorySet::token`]写入`satp`寄存器并调用`asm!("sfence.vma")`刷新地址转换相关硬件
    pub fn activate(&self) {
        let satp = self.page_table.token();
        satp::write(satp);
        unsafe {
            asm!("sfence.vma");
        }
    }

    /// 查表找出`vpn`对应的页表项
    ///
    /// 参见[`super::PageTable::translate`]
    pub fn translate(&self, vpn: VirtPageNum) -> Option<PageTableEntry> {
        self.page_table.translate(vpn)
    }

    /// 回收所有下辖页面
    ///
    /// 通过调用[`MemorySet::areas`]的[`Vec::clear`]方法触发[`FrameTracker::drop`]
    pub fn recycle_data_pages(&mut self) {
        self.areas.clear();
    }

    /// 回收以`start_vpn`为起始地址的内存区域
    ///
    /// 查找该内存区域，解除映射并从[`MemorySet::areas`]中移除
    pub fn remove_area_with_start_vpn(&mut self, start_vpn: VirtPageNum) {
        if let Some((idx, area)) = self
            .areas
            .iter_mut()
            .enumerate()
            .find(|(_, area)| area.vpn_range.get_start() == start_vpn)
        {
            area.unmap(&mut self.page_table);
            self.areas.remove(idx);
        }
    }
}

impl MapArea {
    /// 以起始和结束虚拟地址以及给定的映射类型和权限信息构造
    pub fn new(
        start_va: VirtAddr,
        end_va: VirtAddr,
        map_type: MapType,
        map_perm: MapPermission,
    ) -> Self {
        let start_vpn: VirtPageNum = start_va.floor();
        let end_vpn: VirtPageNum = end_va.ceil();
        Self {
            vpn_range: VPNRange::new(start_vpn, end_vpn),
            data_frames: BTreeMap::new(),
            map_type,
            map_perm,
        }
    }

    /// 在`page_table`中构建此内存区域的映射
    ///
    /// 调用[`MapArea::map_one`]逐个映射虚拟页面
    pub fn map(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.map_one(page_table, vpn);
        }
    }

    /// 从`page_table`中移除此内存区域的映射
    ///
    /// 调用[`MapArea::unmap_one`]逐个移除虚拟页面的映射
    pub fn unmap(&mut self, page_table: &mut PageTable) {
        for vpn in self.vpn_range {
            self.unmap_one(page_table, vpn);
        }
    }

    /// 将`data`复制到此内存区域的开头
    ///
    /// 要求此内存区域不为恒等映射
    pub fn copy_data(&mut self, page_table: &PageTable, data: &[u8]) {
        assert_eq!(self.map_type, MapType::Framed);
        let mut start: usize = 0;
        let mut current_vpn = self.vpn_range.get_start();
        let len = data.len();
        loop {
            let src = &data[start..len.min(start + PAGE_SIZE)];
            let dst = &mut page_table
                .translate(current_vpn)
                .unwrap()
                .ppn()
                .get_bytes_array()[..src.len()];
            dst.copy_from_slice(src);
            start += PAGE_SIZE;
            if start >= len {
                break;
            }
            current_vpn.step();
        }
    }

    /// 在`page_table`中建立对虚拟页面`vpn`的映射
    ///
    /// # 逻辑概要
    /// 1. 若为恒等映射，可直接得出`ppn`
    /// 2. 若为分配页框的映射，分配一个新的物理页框并插入[`MapArea::data_frames`]中
    /// 3. 调用[`PageTable::map`]
    pub fn map_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        let ppn: PhysPageNum;
        match self.map_type {
            MapType::Identical => {
                ppn = PhysPageNum(vpn.0);
            }
            MapType::Framed => {
                let frame = frame_alloc().unwrap();
                ppn = frame.ppn;
                self.data_frames.insert(vpn, frame);
            }
        }
        let pte_flags = PTEFlags::from_bits(self.map_perm.bits).unwrap();
        page_table.map(vpn, ppn, pte_flags);
    }

    /// 从`page_table`中移除对虚拟页面`vpn`的映射
    ///
    /// # 逻辑概要
    /// 1. 若不为恒等映射，从[`MapArea::data_frames`]中移除对应的物理页框
    /// 2. 调用[`PageTable::unmap`]
    pub fn unmap_one(&mut self, page_table: &mut PageTable, vpn: VirtPageNum) {
        if self.map_type == MapType::Framed {
            self.data_frames.remove(&vpn);
        }
        page_table.unmap(vpn);
    }

    /// 从另一`MapArea`构造
    pub fn from_another(another: &MapArea) -> Self {
        Self {
            vpn_range: VPNRange::new(another.vpn_range.get_start(), another.vpn_range.get_end()),
            data_frames: BTreeMap::new(),
            map_type: another.map_type,
            map_perm: another.map_perm,
        }
    }
}

lazy_static! {
    /// 内核地址空间
    pub static ref KERNEL_SPACE: Arc<UPIntrFreeCell<MemorySet>> =
        Arc::new(unsafe { UPIntrFreeCell::new(MemorySet::new_kernel()) });
}

/// 取得内核空间对应的`satp`寄存器值
///
/// 参见[`MemorySet::token`]、[`PageTable::from_token`]
pub fn kernel_token() -> usize {
    KERNEL_SPACE.exclusive_access().token()
}

#[doc(hidden)]
#[allow(unused)]
pub fn remap_test() {
    use log::*;
    extern "C" {
        fn stext();
        fn etext();
        fn srodata();
        fn erodata();
        fn sdata();
        fn edata();
    }

    let kernel_space = KERNEL_SPACE.exclusive_access();
    let mid_text: VirtAddr = ((stext as usize + etext as usize) / 2).into();
    let mid_rodata: VirtAddr = ((srodata as usize + erodata as usize) / 2).into();
    let mid_data: VirtAddr = ((sdata as usize + edata as usize) / 2).into();
    assert!(!kernel_space
        .page_table
        .translate(mid_text.floor())
        .unwrap()
        .writable());
    assert!(!kernel_space
        .page_table
        .translate(mid_rodata.floor())
        .unwrap()
        .writable());
    assert!(!kernel_space
        .page_table
        .translate(mid_data.floor())
        .unwrap()
        .executable());
    info!("remap_test passed!");
}
