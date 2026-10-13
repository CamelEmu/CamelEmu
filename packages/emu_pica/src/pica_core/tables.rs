use emu_pica_types::Vec4;

use crate::bits::{bits, sbits};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexValueEntry {
    pub raw: u32,
}

impl ProcTexValueEntry {
    // LUT value, encoded as 12-bit fixed point, with 12 fraction bits
    pub fn value(&self) -> u32 {
        bits(self.raw, 0, 12) // 0.0.12 fixed point
    }

    // Difference between two entry values. Used for efficient interpolation.
    // 0.0.12 fixed point with two's complement. The range is [-0.5, 0.5).
    // Note: the type of this is different from the one of lighting LUT
    pub fn difference(&self) -> i32 {
        sbits(self.raw, 12, 12)
    }

    pub fn to_float(&self) -> f32 {
        self.value() as f32 / 4095.0
    }

    pub fn diff_to_float(&self) -> f32 {
        self.difference() as f32 / 4095.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexColorEntry {
    pub raw: u32,
}

impl ProcTexColorEntry {
    pub fn r(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    pub fn g(&self) -> u32 {
        bits(self.raw, 8, 8)
    }

    pub fn b(&self) -> u32 {
        bits(self.raw, 16, 8)
    }

    pub fn a(&self) -> u32 {
        bits(self.raw, 24, 8)
    }

    pub fn to_vector(&self) -> Vec4<u8> {
        Vec4 {
            x: self.r() as u8,
            y: self.g() as u8,
            z: self.b() as u8,
            w: self.a() as u8,
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexColorDifferenceEntry {
    pub raw: u32,
}

impl ProcTexColorDifferenceEntry {
    pub fn r(&self) -> i32 {
        sbits(self.raw, 0, 8) // half of the difference between two ColorEntry
    }

    pub fn g(&self) -> i32 {
        sbits(self.raw, 8, 8)
    }

    pub fn b(&self) -> i32 {
        sbits(self.raw, 16, 8)
    }

    pub fn a(&self) -> i32 {
        sbits(self.raw, 24, 8)
    }

    pub fn to_vector(&self) -> Vec4<i32> {
        Vec4 {
            x: self.r() * 2,
            y: self.g() * 2,
            z: self.b() * 2,
            w: self.a() * 2,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ProcTex {
    pub noise_table: [ProcTexValueEntry; 128],
    pub color_map_table: [ProcTexValueEntry; 128],
    pub alpha_map_table: [ProcTexValueEntry; 128],
    pub color_table: [ProcTexColorEntry; 256],
    pub color_diff_table: [ProcTexColorDifferenceEntry; 256],
}

impl Default for ProcTex {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LightingLutEntry {
    // Used for raw access
    pub raw: u32,
}

impl LightingLutEntry {
    // LUT value, encoded as 12-bit fixed point, with 12 fraction bits
    pub fn value(&self) -> u32 {
        bits(self.raw, 0, 12) // 0.0.12 fixed point
    }

    // Used for efficient interpolation.
    pub fn difference(&self) -> u32 {
        bits(self.raw, 12, 11) // 0.0.11 fixed point
    }

    pub fn neg_difference(&self) -> u32 {
        bits(self.raw, 23, 1)
    }

    pub fn to_float(&self) -> f32 {
        self.value() as f32 / 4095.0
    }

    pub fn diff_to_float(&self) -> f32 {
        let diff = self.difference() as f32 / 2047.0;
        if self.neg_difference() != 0 {
            -diff
        } else {
            diff
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Lighting {
    pub luts: [[LightingLutEntry; 256]; 24],
}

impl Default for Lighting {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FogLutEntry {
    // Used for raw access
    pub raw: u32,
}

impl FogLutEntry {
    pub fn difference(&self) -> i32 {
        sbits(self.raw, 0, 13) // 1.1.11 fixed point
    }

    pub fn value(&self) -> u32 {
        bits(self.raw, 13, 11) // 0.0.11 fixed point
    }

    pub fn to_float(&self) -> f32 {
        self.value() as f32 / 2047.0
    }

    pub fn diff_to_float(&self) -> f32 {
        self.difference() as f32 / 2047.0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Fog {
    pub lut: [FogLutEntry; 128],
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            lut: [FogLutEntry::default(); 128],
        }
    }
}
