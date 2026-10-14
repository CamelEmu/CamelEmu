use std::cell::RefCell;
use std::rc::Rc;

use crate::regs::RegsInternal;
use crate::{DisplayTransferConfig, FramebufferConfig, MemoryFillConfig};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Regs {
    pub hardware_id: u32,
    _padding0: [u32; 0x3],
    pub memory_fill_config: [MemoryFillConfig; 2],
    pub vram_bank_control: u32,
    pub gpu_busy: u32,
    _padding1: [u32; 0x22],
    pub backlight_control: u32,
    _padding2: [u32; 0xCF],
    pub framebuffer_config: [FramebufferConfig; 2],
    _padding3: [u32; 0x180],
    pub display_transfer_config: DisplayTransferConfig,
    _padding4: [u32; 0xF5],
    pub internal: RegsInternal,
    _padding5: [u32; Regs::NUM_REGS - 0x700],
}

const _: () = assert!(size_of::<Regs>() == Regs::NUM_REGS * size_of::<u32>());

/// The registers of the PICA, shared by the PICA core that writes them and the rasterizer that
/// reads them, as the C++ rasterizer holds a reference to the registers of the PICA core.
pub type SharedRegs = Rc<RefCell<Regs>>;

impl Default for Regs {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Regs {
    pub const NUM_REGS: usize = 0x732;

    pub fn reg_array(&self) -> &[u32; Self::NUM_REGS] {
        unsafe { &*(self as *const Self).cast() }
    }

    pub fn reg_array_mut(&mut self) -> &mut [u32; Self::NUM_REGS] {
        unsafe { &mut *(self as *mut Self).cast() }
    }
}

#[macro_export]
macro_rules! gpu_reg_index {
    ($($field:tt).+) => {
        ::std::mem::offset_of!($crate::pica_regs::Regs, $($field).+) / ::std::mem::size_of::<u32>()
    };
}
