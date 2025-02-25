//! QEMU开发板相关参数和设置
//!
//!

use crate::drivers::{
    chardev::{CharDevice, UART},
    plic::{IntrTargetPriority, PLIC},
};

/// 时钟频率
pub const CLOCK_FREQ: usize = 12500000;

/// 内存大小
pub const MEMORY_END: usize = 0x8800_0000;

/// IO内存映射区域
///
/// # 格式
/// (起始地址, 大小)
pub const MMIO: &[(usize, usize)] = &[
    (0xc000000, 0x210000), // VIRT_PLIC
    (0x10000000, 0x100),   // VIRT_UART0
    (0x10001000, 0x1000),  // VIRTIO
];

/// 块设备驱动
pub type BlockDeviceImpl = crate::drivers::block::VirtIOBlock;
pub type CharDeviceImpl = crate::drivers::chardev::NS16550a<VIRT_UART>;

pub const VIRT_PLIC: usize = 0xC00_0000;
pub const VIRT_UART: usize = 0x1000_0000;

pub fn device_init() {
    use riscv::register::sie;
    let plic = unsafe { PLIC::new(VIRT_PLIC) };
    let hart_id: usize = 0;
    plic.set_threshold(hart_id, IntrTargetPriority::Supervisor, 0);
    plic.set_threshold(hart_id, IntrTargetPriority::Machine, 1);

    // irq nums: 5 keyboard, 6 mouse, 8 block, 10 uart
    for intr_src_id in [10usize] {
        plic.enable(hart_id, IntrTargetPriority::Supervisor, intr_src_id);
        plic.set_priority(intr_src_id, 1);
    }
    unsafe {
        sie::set_sext();
    }
}

pub fn irq_handler() {
    let plic = unsafe { PLIC::new(VIRT_PLIC) };
    let intr_src_id = plic.claim(0, IntrTargetPriority::Supervisor);
    match intr_src_id {
        10 => UART.handle_irq(),
        _ => panic!("unsupported IRQ {}", intr_src_id),
    }
    plic.complete(0, IntrTargetPriority::Supervisor, intr_src_id);
}
