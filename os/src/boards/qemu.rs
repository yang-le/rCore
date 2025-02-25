//! QEMU开发板相关参数和设置
//!
//!

/// 时钟频率
pub const CLOCK_FREQ: usize = 12500000;

/// 内存大小
pub const MEMORY_END: usize = 0x8800_0000;

/// IO内存映射区域
///
/// # 格式
/// (起始地址, 大小)
pub const MMIO: &[(usize, usize)] = &[(0x10001000, 0x1000)];

/// 块设备驱动
pub type BlockDeviceImpl = crate::drivers::block::VirtIOBlock;
