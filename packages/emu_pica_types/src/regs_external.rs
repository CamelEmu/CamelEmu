pub const fn decode_address_register(register_value: u32) -> u32 {
    register_value.wrapping_mul(8)
}

const fn bits(value: u32, position: u32, width: u32) -> u32 {
    (value >> position) & ((1u32 << width) - 1)
}

/// Components are laid out in reverse byte order, most significant bits first.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelFormat(pub u32);

impl PixelFormat {
    pub const RGBA8: Self = Self(0);
    pub const RGB8: Self = Self(1);
    pub const RGB565: Self = Self(2);
    pub const RGB5A1: Self = Self(3);
    pub const RGBA4: Self = Self(4);

    pub fn bytes_per_pixel(self) -> u32 {
        match self {
            Self::RGBA8 => 4,
            Self::RGB8 => 3,
            Self::RGB565 | Self::RGB5A1 | Self::RGBA4 => 2,
            _ => unreachable!(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryFillConfig {
    pub address_start: u32,
    pub address_end: u32,
    pub value_32bit: u32,
    pub control: u32,
}

const _: () = assert!(size_of::<MemoryFillConfig>() == 0x10);

impl MemoryFillConfig {
    pub fn value_16bit(&self) -> u32 {
        bits(self.value_32bit, 0, 16)
    }

    pub fn value_24bit_r(&self) -> u32 {
        bits(self.value_32bit, 0, 8)
    }

    pub fn value_24bit_g(&self) -> u32 {
        bits(self.value_32bit, 8, 8)
    }

    pub fn value_24bit_b(&self) -> u32 {
        bits(self.value_32bit, 16, 8)
    }

    pub fn trigger(&self) -> bool {
        bits(self.control, 0, 1) != 0
    }

    pub fn finished(&self) -> bool {
        bits(self.control, 1, 1) != 0
    }

    pub fn fill_24bit(&self) -> bool {
        bits(self.control, 8, 1) != 0
    }

    pub fn fill_32bit(&self) -> bool {
        bits(self.control, 9, 1) != 0
    }

    pub fn start_address(&self) -> u32 {
        decode_address_register(self.address_start)
    }

    pub fn end_address(&self) -> u32 {
        decode_address_register(self.address_end)
    }

    pub fn debug_name(&self) -> String {
        format!(
            "from {:#X} to {:#X} with {}-bit value {:#X}",
            self.start_address(),
            self.end_address(),
            if self.fill_32bit() {
                "32"
            } else if self.fill_24bit() {
                "24"
            } else {
                "16"
            },
            self.value_32bit
        )
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct FramebufferConfig {
    _padding0: [u32; 0x17],
    pub size: u32,
    _padding1: [u32; 0x2],
    pub address_left1: u32,
    pub address_left2: u32,
    pub format: u32,
    _padding2: [u32; 0x1],
    pub active_fb: u32,
    _padding3: [u32; 0x5],
    pub stride: u32,
    pub address_right1: u32,
    pub address_right2: u32,
    _padding4: [u32; 0x19],
}

const _: () = assert!(size_of::<FramebufferConfig>() == 0x100);

impl FramebufferConfig {
    pub fn width(&self) -> u32 {
        bits(self.size, 0, 16)
    }

    pub fn height(&self) -> u32 {
        bits(self.size, 16, 16)
    }

    pub fn color_format(&self) -> PixelFormat {
        PixelFormat(bits(self.format, 0, 3))
    }

    pub fn second_fb_active(&self) -> bool {
        bits(self.active_fb, 0, 1) != 0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalingMode(pub u32);

impl ScalingMode {
    pub const NO_SCALE: Self = Self(0);
    pub const SCALE_X: Self = Self(1);
    pub const SCALE_XY: Self = Self(2);
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TextureCopy {
    pub size: u32,
    pub input_size: u32,
    pub output_size: u32,
}

impl TextureCopy {
    pub fn input_width(&self) -> u32 {
        bits(self.input_size, 0, 16)
    }

    pub fn input_gap(&self) -> u32 {
        bits(self.input_size, 16, 16)
    }

    pub fn output_width(&self) -> u32 {
        bits(self.output_size, 0, 16)
    }

    pub fn output_gap(&self) -> u32 {
        bits(self.output_size, 16, 16)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DisplayTransferConfig {
    pub input_address: u32,
    pub output_address: u32,
    pub output_size: u32,
    pub input_size: u32,
    pub flags: u32,
    _padding0: u32,
    pub trigger: u32,
    _padding1: u32,
    pub texture_copy: TextureCopy,
}

const _: () = assert!(size_of::<DisplayTransferConfig>() == 0x2c);

impl DisplayTransferConfig {
    pub fn physical_input_address(&self) -> u32 {
        decode_address_register(self.input_address)
    }

    pub fn physical_output_address(&self) -> u32 {
        decode_address_register(self.output_address)
    }

    pub fn output_width(&self) -> u32 {
        bits(self.output_size, 0, 16)
    }

    pub fn output_height(&self) -> u32 {
        bits(self.output_size, 16, 16)
    }

    pub fn input_width(&self) -> u32 {
        bits(self.input_size, 0, 16)
    }

    pub fn input_height(&self) -> u32 {
        bits(self.input_size, 16, 16)
    }

    pub fn flip_vertically(&self) -> bool {
        bits(self.flags, 0, 1) != 0
    }

    pub fn input_linear(&self) -> bool {
        bits(self.flags, 1, 1) != 0
    }

    pub fn crop_input_lines(&self) -> bool {
        bits(self.flags, 2, 1) != 0
    }

    pub fn is_texture_copy(&self) -> bool {
        bits(self.flags, 3, 1) != 0
    }

    pub fn dont_swizzle(&self) -> bool {
        bits(self.flags, 5, 1) != 0
    }

    pub fn input_format(&self) -> PixelFormat {
        PixelFormat(bits(self.flags, 8, 3))
    }

    pub fn output_format(&self) -> PixelFormat {
        PixelFormat(bits(self.flags, 12, 3))
    }

    pub fn block_32(&self) -> bool {
        bits(self.flags, 16, 1) != 0
    }

    pub fn scaling(&self) -> ScalingMode {
        ScalingMode(bits(self.flags, 24, 2))
    }

    pub fn triggered(&self) -> bool {
        bits(self.trigger, 0, 1) != 0
    }

    pub fn debug_name(&self) -> String {
        let scaling = self.scaling();
        format!(
            "from {:#x} to {:#x} with {} scaling and stride {}, width {}",
            self.physical_input_address(),
            self.physical_output_address(),
            if scaling == ScalingMode::NO_SCALE {
                "no"
            } else if scaling == ScalingMode::SCALE_X {
                "X"
            } else {
                "XY"
            },
            self.input_width(),
            self.output_width()
        )
    }
}
