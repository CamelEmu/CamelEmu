use crate::bits::{bits, pica_enum};

pica_enum!(FragmentOperationMode {
    DEFAULT = 0,
    GAS = 1,
    SHADOW = 3,
});

pica_enum!(LogicOp {
    CLEAR = 0,
    AND = 1,
    AND_REVERSE = 2,
    COPY = 3,
    SET = 4,
    COPY_INVERTED = 5,
    NO_OP = 6,
    INVERT = 7,
    NAND = 8,
    OR = 9,
    NOR = 10,
    XOR = 11,
    EQUIV = 12,
    AND_INVERTED = 13,
    OR_REVERSE = 14,
    OR_INVERTED = 15,
});

pica_enum!(BlendEquation {
    ADD = 0,
    SUBTRACT = 1,
    REVERSE_SUBTRACT = 2,
    MIN = 3,
    MAX = 4,
});

pica_enum!(BlendFactor {
    ZERO = 0,
    ONE = 1,
    SOURCE_COLOR = 2,
    ONE_MINUS_SOURCE_COLOR = 3,
    DEST_COLOR = 4,
    ONE_MINUS_DEST_COLOR = 5,
    SOURCE_ALPHA = 6,
    ONE_MINUS_SOURCE_ALPHA = 7,
    DEST_ALPHA = 8,
    ONE_MINUS_DEST_ALPHA = 9,
    CONSTANT_COLOR = 10,
    ONE_MINUS_CONSTANT_COLOR = 11,
    CONSTANT_ALPHA = 12,
    ONE_MINUS_CONSTANT_ALPHA = 13,
    SOURCE_ALPHA_SATURATE = 14,
});

pica_enum!(CompareFunc {
    NEVER = 0,
    ALWAYS = 1,
    EQUAL = 2,
    NOT_EQUAL = 3,
    LESS_THAN = 4,
    LESS_THAN_OR_EQUAL = 5,
    GREATER_THAN = 6,
    GREATER_THAN_OR_EQUAL = 7,
});

pica_enum!(StencilAction {
    KEEP = 0,
    ZERO = 1,
    REPLACE = 2,
    INCREMENT = 3,
    DECREMENT = 4,
    INVERT = 5,
    INCREMENT_WRAP = 6,
    DECREMENT_WRAP = 7,
});

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OutputMerger {
    pub mode_raw: u32,
    pub alpha_blending: u32,
    pub logic_op_raw: u32,
    pub blend_const: u32,
    pub alpha_test: u32,
    pub stencil_test: StencilTest,
    pub depth_color_mask: u32,
    _padding0: [u32; 0x8],
}

impl OutputMerger {
    pub fn fragment_operation_mode(&self) -> FragmentOperationMode {
        FragmentOperationMode(bits(self.mode_raw, 0, 2))
    }

    // If false, logic blending is used
    pub fn alphablend_enable(&self) -> u32 {
        bits(self.mode_raw, 8, 1)
    }

    pub fn blend_equation_rgb(&self) -> BlendEquation {
        BlendEquation(bits(self.alpha_blending, 0, 3))
    }

    pub fn blend_equation_a(&self) -> BlendEquation {
        BlendEquation(bits(self.alpha_blending, 8, 3))
    }

    pub fn factor_source_rgb(&self) -> BlendFactor {
        BlendFactor(bits(self.alpha_blending, 16, 4))
    }

    pub fn factor_dest_rgb(&self) -> BlendFactor {
        BlendFactor(bits(self.alpha_blending, 20, 4))
    }

    pub fn factor_source_a(&self) -> BlendFactor {
        BlendFactor(bits(self.alpha_blending, 24, 4))
    }

    pub fn factor_dest_a(&self) -> BlendFactor {
        BlendFactor(bits(self.alpha_blending, 28, 4))
    }

    pub fn logic_op(&self) -> LogicOp {
        LogicOp(bits(self.logic_op_raw, 0, 4))
    }

    pub fn blend_const_r(&self) -> u32 {
        bits(self.blend_const, 0, 8)
    }

    pub fn blend_const_g(&self) -> u32 {
        bits(self.blend_const, 8, 8)
    }

    pub fn blend_const_b(&self) -> u32 {
        bits(self.blend_const, 16, 8)
    }

    pub fn blend_const_a(&self) -> u32 {
        bits(self.blend_const, 24, 8)
    }

    pub fn alpha_test_enable(&self) -> u32 {
        bits(self.alpha_test, 0, 1)
    }

    pub fn alpha_test_func(&self) -> CompareFunc {
        CompareFunc(bits(self.alpha_test, 4, 3))
    }

    pub fn alpha_test_ref(&self) -> u32 {
        bits(self.alpha_test, 8, 8)
    }

    pub fn depth_test_enable(&self) -> u32 {
        bits(self.depth_color_mask, 0, 1)
    }

    pub fn depth_test_func(&self) -> CompareFunc {
        CompareFunc(bits(self.depth_color_mask, 4, 3))
    }

    pub fn red_enable(&self) -> u32 {
        bits(self.depth_color_mask, 8, 1)
    }

    pub fn green_enable(&self) -> u32 {
        bits(self.depth_color_mask, 9, 1)
    }

    pub fn blue_enable(&self) -> u32 {
        bits(self.depth_color_mask, 10, 1)
    }

    pub fn alpha_enable(&self) -> u32 {
        bits(self.depth_color_mask, 11, 1)
    }

    pub fn depth_write_enable(&self) -> u32 {
        bits(self.depth_color_mask, 12, 1)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct StencilTest {
    // Raw value of this register
    pub raw_func: u32,
    // Raw value of this register
    pub raw_op: u32,
}

impl StencilTest {
    // If true, enable stencil testing
    pub fn enable(&self) -> u32 {
        bits(self.raw_func, 0, 1)
    }

    // Comparison operation for stencil testing
    pub fn func(&self) -> CompareFunc {
        CompareFunc(bits(self.raw_func, 4, 3))
    }

    // Mask used to control writing to the stencil buffer
    pub fn write_mask(&self) -> u32 {
        bits(self.raw_func, 8, 8)
    }

    // Value to compare against for stencil testing
    pub fn reference_value(&self) -> u32 {
        bits(self.raw_func, 16, 8)
    }

    // Mask to apply on stencil test inputs
    pub fn input_mask(&self) -> u32 {
        bits(self.raw_func, 24, 8)
    }

    // Action to perform when the stencil test fails
    pub fn action_stencil_fail(&self) -> StencilAction {
        StencilAction(bits(self.raw_op, 0, 3))
    }

    // Action to perform when stencil testing passed but depth testing fails
    pub fn action_depth_fail(&self) -> StencilAction {
        StencilAction(bits(self.raw_op, 4, 3))
    }

    // Action to perform when both stencil and depth testing pass
    pub fn action_depth_pass(&self) -> StencilAction {
        StencilAction(bits(self.raw_op, 8, 3))
    }
}

pica_enum!(
    // Components are laid out in reverse byte order, most significant bits first.
    ColorFormat {
        RGBA8 = 0,
        RGB8 = 1,
        RGB5A1 = 2,
        RGB565 = 3,
        RGBA4 = 4,
    }
);

pica_enum!(DepthFormat {
    D16 = 0,
    D24 = 2,
    D24S8 = 3,
});

// Returns the number of bytes in the specified color format
pub fn bytes_per_color_pixel(format: ColorFormat) -> u32 {
    match format {
        ColorFormat::RGBA8 => 4,
        ColorFormat::RGB8 => 3,
        ColorFormat::RGB5A1 | ColorFormat::RGB565 | ColorFormat::RGBA4 => 2,
        _ => {
            log::error!(target: "HW_GPU", "Unknown color format {}", format.0);
            log::error!(target: "Debug", "Unimplemented code!");
            0
        }
    }
}

// Returns the number of bytes in the specified depth format
pub fn bytes_per_depth_pixel(format: DepthFormat) -> u32 {
    match format {
        DepthFormat::D16 => 2,
        DepthFormat::D24 => 3,
        DepthFormat::D24S8 => 4,
        _ => panic!("Assertion Failed!\nUnknown depth format {}", format.0),
    }
}

// Returns the number of bits per depth component of the specified depth format
pub fn depth_bits_per_pixel(format: DepthFormat) -> u32 {
    match format {
        DepthFormat::D16 => 16,
        DepthFormat::D24 | DepthFormat::D24S8 => 24,
        _ => unreachable!("Unknown depth format {}", format.0),
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FramebufferConfig {
    _padding0: [u32; 0x3],
    pub allow_color_write_raw: u32,
    _padding1: [u32; 0x1],
    pub allow_depth_stencil_write_raw: u32,
    pub depth_format_raw: u32,
    pub color_format_raw: u32,
    _padding2: [u32; 0x4],
    pub depth_buffer_address_raw: u32,
    pub color_buffer_address_raw: u32,
    // Apparently, the framebuffer width is stored as expected,
    // while the height is stored as the actual height minus one.
    // Hence, don't access these fields directly but use the accessors
    // GetWidth() and GetHeight() instead.
    pub size: u32,
    _padding3: [u32; 0x1],
}

impl FramebufferConfig {
    pub fn allow_color_write(&self) -> u32 {
        bits(self.allow_color_write_raw, 0, 4) // 0 = disable, else enable
    }

    pub fn allow_depth_stencil_write(&self) -> u32 {
        bits(self.allow_depth_stencil_write_raw, 0, 2) // 0 = disable, else enable
    }

    pub fn depth_format(&self) -> DepthFormat {
        DepthFormat(bits(self.depth_format_raw, 0, 2))
    }

    pub fn color_format(&self) -> ColorFormat {
        ColorFormat(bits(self.color_format_raw, 16, 3))
    }

    pub fn depth_buffer_address(&self) -> u32 {
        bits(self.depth_buffer_address_raw, 0, 28)
    }

    pub fn color_buffer_address(&self) -> u32 {
        bits(self.color_buffer_address_raw, 0, 28)
    }

    pub fn get_color_buffer_physical_address(&self) -> u32 {
        self.color_buffer_address().wrapping_mul(8)
    }

    pub fn get_depth_buffer_physical_address(&self) -> u32 {
        self.depth_buffer_address().wrapping_mul(8)
    }

    pub fn get_width(&self) -> u32 {
        bits(self.size, 0, 11)
    }

    pub fn get_height(&self) -> u32 {
        bits(self.size, 12, 10) + 1
    }

    pub fn is_flipped(&self) -> bool {
        bits(self.size, 24, 1) == 0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FramebufferRegs {
    pub output_merger: OutputMerger,
    pub framebuffer: FramebufferConfig,
    _padding0: [u32; 0x10], // Gas related registers
    pub shadow: u32,
    _padding1: [u32; 0xF],
}

const _: () = assert!(
    size_of::<FramebufferRegs>() == 0x40 * size_of::<u32>(),
    "FramebufferRegs struct has incorrect size"
);

impl FramebufferRegs {
    pub fn is_shadow_rendering(&self) -> bool {
        self.output_merger.fragment_operation_mode() == FragmentOperationMode::SHADOW
    }

    pub fn has_stencil(&self) -> bool {
        self.framebuffer.depth_format() == DepthFormat::D24S8
    }

    pub fn shadow_constant(&self) -> u32 {
        bits(self.shadow, 0, 16) // float1.5.10
    }

    pub fn shadow_linear(&self) -> u32 {
        bits(self.shadow, 16, 16) // float1.5.10
    }
}
