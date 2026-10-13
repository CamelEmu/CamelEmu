use crate::bits::{assign, bits, mask, pica_enum, sbits};

pica_enum!(TextureType {
    TEXTURE_2D = 0,
    TEXTURE_CUBE = 1,
    SHADOW_2D = 2,
    PROJECTION_2D = 3,
    SHADOW_CUBE = 4,
    DISABLED = 5,
});

pica_enum!(WrapMode {
    CLAMP_TO_EDGE = 0,
    CLAMP_TO_BORDER = 1,
    REPEAT = 2,
    MIRRORED_REPEAT = 3,
    // Mode 4-7 produces some weird result and may be just invalid:
    CLAMP_TO_EDGE2 = 4,   // Positive coord: clamp to edge; negative coord: repeat
    CLAMP_TO_BORDER2 = 5, // Positive coord: clamp to border; negative coord: repeat
    REPEAT2 = 6,          // Same as Repeat
    REPEAT3 = 7,          // Same as Repeat
});

pica_enum!(TextureFilter {
    NEAREST = 0,
    LINEAR = 1,
});

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TextureConfig {
    pub border_color: u32,
    pub size: u32,
    pub params: u32,
    pub lod: u32,
    pub address_raw: u32,
    // texture1 and texture2 store the texture format directly after the address
    // whereas texture0 inserts some additional flags inbetween.
    // Hence, we store the format separately so that all other parameters can be described
    // in a single structure.
}

impl TextureConfig {
    pub fn border_color_r(&self) -> u32 {
        bits(self.border_color, 0, 8)
    }

    pub fn border_color_g(&self) -> u32 {
        bits(self.border_color, 8, 8)
    }

    pub fn border_color_b(&self) -> u32 {
        bits(self.border_color, 16, 8)
    }

    pub fn border_color_a(&self) -> u32 {
        bits(self.border_color, 24, 8)
    }

    pub fn height(&self) -> u32 {
        bits(self.size, 0, 11)
    }

    pub fn width(&self) -> u32 {
        bits(self.size, 16, 11)
    }

    pub fn mag_filter(&self) -> TextureFilter {
        TextureFilter(bits(self.params, 1, 1))
    }

    pub fn min_filter(&self) -> TextureFilter {
        TextureFilter(bits(self.params, 2, 1))
    }

    pub fn wrap_t(&self) -> WrapMode {
        WrapMode(bits(self.params, 8, 3))
    }

    pub fn wrap_s(&self) -> WrapMode {
        WrapMode(bits(self.params, 12, 3))
    }

    pub fn mip_filter(&self) -> TextureFilter {
        TextureFilter(bits(self.params, 24, 1))
    }

    /// @note Only valid for texture 0 according to 3DBrew.
    pub fn texture_type(&self) -> TextureType {
        TextureType(bits(self.params, 28, 3))
    }

    pub fn lod_bias(&self) -> i32 {
        sbits(self.lod, 0, 13) // fixed1.4.8
    }

    pub fn lod_max_level(&self) -> u32 {
        bits(self.lod, 16, 4)
    }

    pub fn lod_min_level(&self) -> u32 {
        bits(self.lod, 24, 4)
    }

    pub fn address(&self) -> u32 {
        bits(self.address_raw, 0, 28)
    }

    pub fn get_physical_address(&self) -> u32 {
        self.address().wrapping_mul(8)
    }
}

pica_enum!(TextureFormat {
    RGBA8 = 0,
    RGB8 = 1,
    RGB5A1 = 2,
    RGB565 = 3,
    RGBA4 = 4,
    IA8 = 5,
    RG8 = 6, // @note Also called HILO8 in 3DBrew.
    I8 = 7,
    A8 = 8,
    IA4 = 9,
    I4 = 10,
    A4 = 11,
    ETC1 = 12,   // compressed
    ETC1A4 = 13, // compressed
});

impl TextureFormat {
    pub fn nibbles_per_pixel(self) -> u32 {
        match self {
            Self::RGBA8 => 8,
            Self::RGB8 => 6,
            Self::RGB5A1 | Self::RGB565 | Self::RGBA4 | Self::IA8 | Self::RG8 => 4,
            Self::I4 | Self::A4 => 1,
            Self::I8 | Self::A8 | Self::IA4 => 2,
            // placeholder for yet unknown formats
            _ => {
                log::error!(target: "Debug", "Unimplemented code!");
                0
            }
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MainConfig {
    pub raw: u32,
}

impl MainConfig {
    pub fn texture0_enable(&self) -> u32 {
        bits(self.raw, 0, 1)
    }

    pub fn texture1_enable(&self) -> u32 {
        bits(self.raw, 1, 1)
    }

    pub fn texture2_enable(&self) -> u32 {
        bits(self.raw, 2, 1)
    }

    pub fn texture3_coordinates(&self) -> u32 {
        bits(self.raw, 8, 2)
    }

    pub fn texture3_enable(&self) -> u32 {
        bits(self.raw, 10, 1)
    }

    pub fn texture2_use_coord1(&self) -> u32 {
        bits(self.raw, 13, 1)
    }

    pub fn clear_texture_cache(&self) -> u32 {
        bits(self.raw, 16, 1) // TODO: unimplemented
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CubeFace {
    PositiveX = 0,
    NegativeX = 1,
    PositiveY = 2,
    NegativeY = 3,
    PositiveZ = 4,
    NegativeZ = 5,
}

pica_enum!(ProcTexClamp {
    TO_ZERO = 0,
    TO_EDGE = 1,
    SYMMETRICAL_REPEAT = 2,
    MIRRORED_REPEAT = 3,
    PULSE = 4,
});

pica_enum!(ProcTexCombiner {
    U = 0,          // u
    U2 = 1,         // u * u
    V = 2,          // v
    V2 = 3,         // v * v
    ADD = 4,        // (u + v) / 2
    ADD2 = 5,       // (u * u + v * v) / 2
    SQRT_ADD2 = 6,  // sqrt(u * u + v * v)
    MIN = 7,        // min(u, v)
    MAX = 8,        // max(u, v)
    R_MAX = 9,      // Average of Max and SqrtAdd2
});

pica_enum!(ProcTexShift {
    NONE = 0,
    ODD = 1,
    EVEN = 2,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTex {
    pub raw: u32,
}

impl ProcTex {
    pub fn u_clamp(&self) -> ProcTexClamp {
        ProcTexClamp(bits(self.raw, 0, 3))
    }

    pub fn v_clamp(&self) -> ProcTexClamp {
        ProcTexClamp(bits(self.raw, 3, 3))
    }

    pub fn color_combiner(&self) -> ProcTexCombiner {
        ProcTexCombiner(bits(self.raw, 6, 4))
    }

    pub fn alpha_combiner(&self) -> ProcTexCombiner {
        ProcTexCombiner(bits(self.raw, 10, 4))
    }

    pub fn separate_alpha(&self) -> u32 {
        bits(self.raw, 14, 1)
    }

    pub fn noise_enable(&self) -> u32 {
        bits(self.raw, 15, 1)
    }

    pub fn u_shift(&self) -> ProcTexShift {
        ProcTexShift(bits(self.raw, 16, 2))
    }

    pub fn v_shift(&self) -> ProcTexShift {
        ProcTexShift(bits(self.raw, 18, 2))
    }

    pub fn bias_low(&self) -> u32 {
        bits(self.raw, 20, 8) // float16 TODO: unimplemented
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexNoiseConfig {
    pub raw: u32,
}

impl ProcTexNoiseConfig {
    pub fn amplitude(&self) -> i32 {
        sbits(self.raw, 0, 16) // fixed1.3.12
    }

    pub fn phase(&self) -> u32 {
        bits(self.raw, 16, 16) // float16
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexNoiseFrequency {
    pub raw: u32,
}

impl ProcTexNoiseFrequency {
    pub fn u(&self) -> u32 {
        bits(self.raw, 0, 16) // float16
    }

    pub fn v(&self) -> u32 {
        bits(self.raw, 16, 16) // float16
    }
}

pica_enum!(ProcTexFilter {
    NEAREST = 0,
    LINEAR = 1,
    NEAREST_MIPMAP_NEAREST = 2,
    LINEAR_MIPMAP_NEAREST = 3,
    NEAREST_MIPMAP_LINEAR = 4,
    LINEAR_MIPMAP_LINEAR = 5,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexLut {
    pub raw: u32,
}

impl ProcTexLut {
    pub fn filter(&self) -> ProcTexFilter {
        ProcTexFilter(bits(self.raw, 0, 3))
    }

    pub fn lod_min(&self) -> u32 {
        bits(self.raw, 3, 4)
    }

    pub fn lod_max(&self) -> u32 {
        bits(self.raw, 7, 4)
    }

    pub fn width(&self) -> u32 {
        bits(self.raw, 11, 8)
    }

    pub fn bias_high(&self) -> u32 {
        bits(self.raw, 19, 8) // TODO: unimplemented
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexLutOffset {
    pub raw: u32,
}

impl ProcTexLutOffset {
    pub fn level0(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    pub fn level1(&self) -> u32 {
        bits(self.raw, 8, 8)
    }

    pub fn level2(&self) -> u32 {
        bits(self.raw, 16, 8)
    }

    pub fn level3(&self) -> u32 {
        bits(self.raw, 24, 8)
    }
}

pica_enum!(ProcTexLutTable {
    NOISE = 0,
    COLOR_MAP = 2,
    ALPHA_MAP = 3,
    COLOR = 4,
    COLOR_DIFF = 5,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexLutConfig {
    pub raw: u32,
}

impl ProcTexLutConfig {
    pub fn index(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    pub fn set_index(&mut self, index: u32) {
        assign(&mut self.raw, 0, 8, index);
    }

    pub fn ref_table(&self) -> ProcTexLutTable {
        ProcTexLutTable(bits(self.raw, 8, 4))
    }
}

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

#[derive(Clone, Copy, Debug)]
pub struct FullTextureConfig {
    pub enabled: u32,
    pub config: TextureConfig,
    pub format: TextureFormat,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TexturingRegs {
    pub main_config: MainConfig,
    pub texture0: TextureConfig,
    pub cube_address_raw: [u32; 5],
    pub shadow: u32,
    _padding0: [u32; 0x2],
    pub texture0_format_raw: u32,
    pub fragment_lighting_enable_raw: u32,
    _padding1: [u32; 0x1],
    pub texture1: TextureConfig,
    pub texture1_format_raw: u32,
    _padding2: [u32; 0x2],
    pub texture2: TextureConfig,
    pub texture2_format_raw: u32,
    _padding3: [u32; 0x9],
    // 0xa8-0xad: ProcTex Config
    pub proctex: ProcTex,
    pub proctex_noise_u: ProcTexNoiseConfig,
    pub proctex_noise_v: ProcTexNoiseConfig,
    pub proctex_noise_frequency: ProcTexNoiseFrequency,
    pub proctex_lut: ProcTexLut,
    pub proctex_lut_offset: ProcTexLutOffset,
    _padding4: [u32; 0x1],
    // 0xaf-0xb7: ProcTex LUT
    pub proctex_lut_config: ProcTexLutConfig,
    pub proctex_lut_data: [u32; 8],
    _padding5: [u32; 0x8],
    pub tev_stage0: TevStageConfig,
    _padding6: [u32; 0x3],
    pub tev_stage1: TevStageConfig,
    _padding7: [u32; 0x3],
    pub tev_stage2: TevStageConfig,
    _padding8: [u32; 0x3],
    pub tev_stage3: TevStageConfig,
    _padding9: [u32; 0x3],
    pub fog_mode: FogModeConfig,
    pub fog_color: Rgba8,
    _padding10: [u32; 0x4],
    pub fog_lut_offset_raw: u32,
    _padding11: [u32; 0x1],
    pub fog_lut_data: [u32; 8],
    pub tev_stage4: TevStageConfig,
    _padding12: [u32; 0x3],
    pub tev_stage5: TevStageConfig,
    pub tev_combiner_buffer_color: Rgba8,
    _padding13: [u32; 0x2],
}

const _: () = assert!(size_of::<TexturingRegs>() == 0x80 * size_of::<u32>());

impl TexturingRegs {
    pub fn cube_address(&self, index: usize) -> u32 {
        bits(self.cube_address_raw[index], 0, 22)
    }

    pub fn get_cube_physical_address(&self, face: CubeFace) -> u32 {
        let mut address = self.texture0.address();
        if face != CubeFace::PositiveX {
            // Bits [22:27] from the main texture address is shared with all cubemap additional
            // addresses.
            let face_addr = self.cube_address(face as usize - 1);
            address &= !mask(22);
            address |= face_addr;
        }
        // A multiplier of 8 is also needed in the same way as the main address.
        address.wrapping_mul(8)
    }

    pub fn shadow_orthographic(&self) -> u32 {
        bits(self.shadow, 0, 1) // 0: enable perspective divide
    }

    pub fn shadow_bias(&self) -> u32 {
        bits(self.shadow, 1, 23) // 23-bit fraction
    }

    pub fn texture0_format(&self) -> TextureFormat {
        TextureFormat(bits(self.texture0_format_raw, 0, 4))
    }

    pub fn fragment_lighting_enable(&self) -> u32 {
        bits(self.fragment_lighting_enable_raw, 0, 1)
    }

    pub fn texture1_format(&self) -> TextureFormat {
        TextureFormat(bits(self.texture1_format_raw, 0, 4))
    }

    pub fn texture2_format(&self) -> TextureFormat {
        TextureFormat(bits(self.texture2_format_raw, 0, 4))
    }

    pub fn get_textures(&self) -> [FullTextureConfig; 3] {
        [
            FullTextureConfig {
                enabled: self.main_config.texture0_enable(),
                config: self.texture0,
                format: self.texture0_format(),
            },
            FullTextureConfig {
                enabled: self.main_config.texture1_enable(),
                config: self.texture1,
                format: self.texture1_format(),
            },
            FullTextureConfig {
                enabled: self.main_config.texture2_enable(),
                config: self.texture2,
                format: self.texture2_format(),
            },
        ]
    }

    pub fn fog_lut_offset(&self) -> u32 {
        bits(self.fog_lut_offset_raw, 0, 16)
    }

    pub fn set_fog_lut_offset(&mut self, offset: u32) {
        assign(&mut self.fog_lut_offset_raw, 0, 16, offset);
    }

    pub fn get_tev_stages(&self) -> [TevStageConfig; 6] {
        [
            self.tev_stage0,
            self.tev_stage1,
            self.tev_stage2,
            self.tev_stage3,
            self.tev_stage4,
            self.tev_stage5,
        ]
    }
}
