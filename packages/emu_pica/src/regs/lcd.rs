use std::mem::offset_of;

use emu_pica_types::Vec3;

use crate::bits::bits;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ColorFill {
    pub raw: u32,
}

impl ColorFill {
    pub fn color_r(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    pub fn color_g(&self) -> u32 {
        bits(self.raw, 8, 8)
    }

    pub fn color_b(&self) -> u32 {
        bits(self.raw, 16, 8)
    }

    pub fn is_enabled(&self) -> bool {
        bits(self.raw, 24, 1) != 0
    }

    pub fn as_vector(&self) -> Vec3<u8> {
        Vec3 {
            x: self.color_r() as u8,
            y: self.color_g() as u8,
            z: self.color_b() as u8,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RegsLcd {
    _padding0: [u32; 0x81],
    pub color_fill_top: ColorFill,
    _padding1: [u32; 0xE],
    pub backlight_top: u32,
    _padding2: [u32; 0x1F0],
    pub color_fill_bottom: ColorFill,
    _padding3: [u32; 0xE],
    pub backlight_bottom: u32,
    _padding4: [u32; 0x16F],
}

const _: () = assert!(offset_of!(RegsLcd, color_fill_top) == 0x81 * 4);
const _: () = assert!(offset_of!(RegsLcd, backlight_top) == 0x90 * 4);
const _: () = assert!(offset_of!(RegsLcd, color_fill_bottom) == 0x281 * 4);
const _: () = assert!(offset_of!(RegsLcd, backlight_bottom) == 0x290 * 4);

impl Default for RegsLcd {
    fn default() -> Self {
        Self {
            _padding0: [0; 0x81],
            color_fill_top: ColorFill::default(),
            _padding1: [0; 0xE],
            backlight_top: 0,
            _padding2: [0; 0x1F0],
            color_fill_bottom: ColorFill::default(),
            _padding3: [0; 0xE],
            backlight_bottom: 0,
            _padding4: [0; 0x16F],
        }
    }
}

impl RegsLcd {
    pub const fn num_ids() -> usize {
        size_of::<Self>() / size_of::<u32>()
    }

    pub fn as_words(&self) -> &[u32; Self::num_ids()] {
        unsafe { &*(self as *const Self).cast() }
    }

    pub fn as_words_mut(&mut self) -> &mut [u32; Self::num_ids()] {
        unsafe { &mut *(self as *mut Self).cast() }
    }
}
