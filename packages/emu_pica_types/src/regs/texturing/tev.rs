use crate::bits::{bits, pica_enum};

pica_enum!(TevSource {
    PRIMARY_COLOR = 0x0,
    PRIMARY_FRAGMENT_COLOR = 0x1,
    SECONDARY_FRAGMENT_COLOR = 0x2,

    TEXTURE0 = 0x3,
    TEXTURE1 = 0x4,
    TEXTURE2 = 0x5,
    TEXTURE3 = 0x6,

    PREVIOUS_BUFFER = 0xd,
    CONSTANT = 0xe,
    PREVIOUS = 0xf,
});

pica_enum!(TevColorModifier {
    SOURCE_COLOR = 0x0,
    ONE_MINUS_SOURCE_COLOR = 0x1,
    SOURCE_ALPHA = 0x2,
    ONE_MINUS_SOURCE_ALPHA = 0x3,
    SOURCE_RED = 0x4,
    ONE_MINUS_SOURCE_RED = 0x5,

    SOURCE_GREEN = 0x8,
    ONE_MINUS_SOURCE_GREEN = 0x9,

    SOURCE_BLUE = 0xc,
    ONE_MINUS_SOURCE_BLUE = 0xd,
});

pica_enum!(TevAlphaModifier {
    SOURCE_ALPHA = 0x0,
    ONE_MINUS_SOURCE_ALPHA = 0x1,
    SOURCE_RED = 0x2,
    ONE_MINUS_SOURCE_RED = 0x3,
    SOURCE_GREEN = 0x4,
    ONE_MINUS_SOURCE_GREEN = 0x5,
    SOURCE_BLUE = 0x6,
    ONE_MINUS_SOURCE_BLUE = 0x7,
});

pica_enum!(TevOperation {
    REPLACE = 0,
    MODULATE = 1,
    ADD = 2,
    ADD_SIGNED = 3,
    LERP = 4,
    SUBTRACT = 5,
    DOT3_RGB = 6,
    DOT3_RGBA = 7,
    MULTIPLY_THEN_ADD = 8,
    ADD_THEN_MULTIPLY = 9,
});

// 0xc0-0xff: Texture Combiner (akin to glTexEnv)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TevStageConfig {
    pub sources_raw: u32,
    pub modifiers_raw: u32,
    pub ops_raw: u32,
    pub const_color: u32,
    pub scales_raw: u32,
}

impl TevStageConfig {
    pub fn color_source1(&self) -> TevSource {
        TevSource(bits(self.sources_raw, 0, 4))
    }

    pub fn color_source2(&self) -> TevSource {
        TevSource(bits(self.sources_raw, 4, 4))
    }

    pub fn color_source3(&self) -> TevSource {
        TevSource(bits(self.sources_raw, 8, 4))
    }

    pub fn alpha_source1(&self) -> TevSource {
        TevSource(bits(self.sources_raw, 16, 4))
    }

    pub fn alpha_source2(&self) -> TevSource {
        TevSource(bits(self.sources_raw, 20, 4))
    }

    pub fn alpha_source3(&self) -> TevSource {
        TevSource(bits(self.sources_raw, 24, 4))
    }

    pub fn color_modifier1(&self) -> TevColorModifier {
        TevColorModifier(bits(self.modifiers_raw, 0, 4))
    }

    pub fn color_modifier2(&self) -> TevColorModifier {
        TevColorModifier(bits(self.modifiers_raw, 4, 4))
    }

    pub fn color_modifier3(&self) -> TevColorModifier {
        TevColorModifier(bits(self.modifiers_raw, 8, 4))
    }

    pub fn alpha_modifier1(&self) -> TevAlphaModifier {
        TevAlphaModifier(bits(self.modifiers_raw, 12, 3))
    }

    pub fn alpha_modifier2(&self) -> TevAlphaModifier {
        TevAlphaModifier(bits(self.modifiers_raw, 16, 3))
    }

    pub fn alpha_modifier3(&self) -> TevAlphaModifier {
        TevAlphaModifier(bits(self.modifiers_raw, 20, 3))
    }

    pub fn color_op(&self) -> TevOperation {
        TevOperation(bits(self.ops_raw, 0, 4))
    }

    pub fn alpha_op(&self) -> TevOperation {
        TevOperation(bits(self.ops_raw, 16, 4))
    }

    pub fn const_r(&self) -> u32 {
        bits(self.const_color, 0, 8)
    }

    pub fn const_g(&self) -> u32 {
        bits(self.const_color, 8, 8)
    }

    pub fn const_b(&self) -> u32 {
        bits(self.const_color, 16, 8)
    }

    pub fn const_a(&self) -> u32 {
        bits(self.const_color, 24, 8)
    }

    pub fn color_scale(&self) -> u32 {
        bits(self.scales_raw, 0, 2)
    }

    pub fn alpha_scale(&self) -> u32 {
        bits(self.scales_raw, 16, 2)
    }

    pub fn get_color_multiplier(&self) -> u32 {
        if self.color_scale() < 3 {
            1 << self.color_scale()
        } else {
            1
        }
    }

    pub fn get_alpha_multiplier(&self) -> u32 {
        if self.alpha_scale() < 3 {
            1 << self.alpha_scale()
        } else {
            1
        }
    }
}

pica_enum!(FogMode {
    NONE = 0,
    FOG = 5,
    GAS = 7,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FogModeConfig {
    pub raw: u32,
}

impl FogModeConfig {
    pub fn fog_mode(&self) -> FogMode {
        FogMode(bits(self.raw, 0, 3))
    }

    pub fn fog_flip(&self) -> u32 {
        bits(self.raw, 16, 1)
    }

    // Tev stages 0-3 write their output to the combiner buffer if the corresponding bit in
    // these masks are set
    pub fn update_mask_rgb(&self) -> u32 {
        bits(self.raw, 8, 4)
    }

    pub fn update_mask_a(&self) -> u32 {
        bits(self.raw, 12, 4)
    }

    pub fn tev_stage_updates_combiner_buffer_color(&self, stage_index: u32) -> bool {
        stage_index < 4 && self.update_mask_rgb() & (1 << stage_index) != 0
    }

    pub fn tev_stage_updates_combiner_buffer_alpha(&self, stage_index: u32) -> bool {
        stage_index < 4 && self.update_mask_a() & (1 << stage_index) != 0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Rgba8 {
    pub raw: u32,
}

impl Rgba8 {
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
}
