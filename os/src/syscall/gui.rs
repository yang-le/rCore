use crate::{
    drivers::gpu::GPU_DEVICE,
    mm::{MapArea, MapPermission, MapType, PhysAddr, VirtAddr},
    task::current_process,
};

const FB_VADDR: usize = 0x1000_0000;

pub fn sys_framebuffer() -> isize {
    let gpu = GPU_DEVICE.clone();
    let fb = gpu.get_framebuffer();
    let len = fb.len();
    let fb_start_pa = PhysAddr::from(fb.as_ptr() as usize);
    assert!(fb_start_pa.aligned());
    let fb_start_ppn = fb_start_pa.floor();
    let fb_start_vpn = VirtAddr::from(FB_VADDR).floor();
    let pn_offset = fb_start_ppn.0 as isize - fb_start_vpn.0 as isize;

    let current_process = current_process();
    let mut inner = current_process.inner_exclusive_access();
    inner.memory_set.push(
        MapArea::new(
            FB_VADDR.into(),
            (FB_VADDR + len).into(),
            MapType::Linear(pn_offset),
            MapPermission::R | MapPermission::W | MapPermission::U,
        ),
        None,
    );
    FB_VADDR as isize
}

pub fn sys_framebuffer_flush() -> isize {
    let gpu = GPU_DEVICE.clone();
    gpu.flush();
    0
}
