use crate::bits::{assign, bits};

/// Most physical addresses which GPU registers refer to are 8-byte aligned.
/// This function should be used to get the address from a raw register value.
pub const fn decode_address_register(register_value: u32) -> u32 {
    register_value.wrapping_mul(8)
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

    // TODO: Verify component order
    pub fn value_24bit_r(&self) -> u32 {
        bits(self.value_32bit, 0, 8)
    }

    pub fn value_24bit_g(&self) -> u32 {
        bits(self.value_32bit, 8, 8)
    }

    pub fn value_24bit_b(&self) -> u32 {
        bits(self.value_32bit, 16, 8)
    }

    // Setting this field to 1 triggers the memory fill.
    // This field also acts as a status flag, and gets reset to 0 upon completion.
    pub fn trigger(&self) -> bool {
        bits(self.control, 0, 1) != 0
    }

    pub fn set_trigger(&mut self, trigger: bool) {
        assign(&mut self.control, 0, 1, u32::from(trigger));
    }

    // Set to 1 upon completion.
    pub fn finished(&self) -> bool {
        bits(self.control, 1, 1) != 0
    }

    pub fn set_finished(&mut self, finished: bool) {
        assign(&mut self.control, 1, 1, u32::from(finished));
    }

    // If both of these bits are unset, then it will fill the memory with a 16 bit value
    // 1: fill with 24-bit wide values
    pub fn fill_24bit(&self) -> bool {
        bits(self.control, 8, 1) != 0
    }

    // 1: fill with 32-bit wide values
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
    // Distance between two pixel rows, in bytes
    pub stride: u32,
    pub address_right1: u32,
    pub address_right2: u32,
    _padding4: [u32; 0x19],
}

const _: () = assert!(size_of::<FramebufferConfig>() == 0x100);

impl Default for FramebufferConfig {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl FramebufferConfig {
    pub fn width(&self) -> u32 {
        bits(self.size, 0, 16)
    }

    pub fn set_width(&mut self, width: u32) {
        assign(&mut self.size, 0, 16, width);
    }

    pub fn height(&self) -> u32 {
        bits(self.size, 16, 16)
    }

    pub fn set_height(&mut self, height: u32) {
        assign(&mut self.size, 16, 16, height);
    }

    pub fn color_format(&self) -> PixelFormat {
        PixelFormat(bits(self.format, 0, 3))
    }

    pub fn set_color_format(&mut self, format: PixelFormat) {
        assign(&mut self.format, 0, 3, format.0);
    }

    // 0: Use parameters ending with "1"
    // 1: Use parameters ending with "2"
    pub fn second_fb_active(&self) -> bool {
        bits(self.active_fb, 0, 1) != 0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalingMode(pub u32);

impl ScalingMode {
    pub const NO_SCALE: Self = Self(0); // Doesn't scale the image
    pub const SCALE_X: Self = Self(1); // Downscales the image in half in the X axis and applies a box filter
    pub const SCALE_XY: Self = Self(2); // Downscales the image in half in both the X and Y axes and applies a box filter
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TextureCopy {
    pub size: u32, // The lower 4 bits are ignored
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
    // it seems that writing to this field triggers the display transfer
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

    // flips input data vertically
    pub fn flip_vertically(&self) -> bool {
        bits(self.flags, 0, 1) != 0
    }

    // Converts from linear to tiled format
    pub fn input_linear(&self) -> bool {
        bits(self.flags, 1, 1) != 0
    }

    pub fn crop_input_lines(&self) -> bool {
        bits(self.flags, 2, 1) != 0
    }

    // Copies the data without performing any processing and respecting texture copy fields
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

    /// Uses some kind of 32x32 block swizzling mode, instead of the usual 8x8 one.
    // TODO(yuriks): unimplemented
    pub fn block_32(&self) -> bool {
        bits(self.flags, 16, 1) != 0
    }

    // Determines the scaling mode of the transfer
    pub fn scaling(&self) -> ScalingMode {
        ScalingMode(bits(self.flags, 24, 2))
    }

    pub fn triggered(&self) -> bool {
        bits(self.trigger, 0, 1) != 0
    }

    pub fn set_triggered(&mut self, triggered: bool) {
        assign(&mut self.trigger, 0, 1, u32::from(triggered));
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
