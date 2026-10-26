use crate::error::{Result, ShaderError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigLayout {
    pub total_size: usize,
    pub texture_borders: usize,
    pub tev_stages: usize,
    pub lights: usize,
}

impl Default for ConfigLayout {
    fn default() -> Self {
        Self {
            total_size: 248,
            texture_borders: 3,
            tev_stages: 6,
            lights: 8,
        }
    }
}

fn bits(raw: u32, position: u32, width: u32) -> u32 {
    (raw >> position) & ((1u32 << width) - 1)
}

fn flag(raw: u32, position: u32) -> bool {
    bits(raw, position, 1) != 0
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let mut value = [0u8; N];
        value.copy_from_slice(&self.bytes[self.cursor..self.cursor + N]);
        self.cursor += N;
        value
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take())
    }

    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take())
    }

    fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.take())
    }

    fn f32(&mut self) -> f32 {
        f32::from_le_bytes(self.take())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BlendConfig {
    pub eq: u32,
    pub src_factor: u32,
    pub dst_factor: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FramebufferConfig {
    pub raw: u32,
    pub rgb_blend: BlendConfig,
    pub alpha_blend: BlendConfig,
}

impl FramebufferConfig {
    pub fn alpha_test_func(&self) -> u32 {
        bits(self.raw, 0, 3)
    }

    pub fn scissor_test_mode(&self) -> u32 {
        bits(self.raw, 3, 2)
    }

    pub fn depthmap_enable(&self) -> u32 {
        bits(self.raw, 5, 1)
    }

    pub fn logic_op(&self) -> u32 {
        bits(self.raw, 6, 4)
    }

    pub fn shadow_rendering(&self) -> bool {
        flag(self.raw, 10)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TevStageConfig {
    pub sources_raw: u32,
    pub modifiers_raw: u32,
    pub ops_raw: u32,
    pub scales_raw: u32,
}

impl TevStageConfig {
    pub fn color_source1(&self) -> u32 {
        bits(self.sources_raw, 0, 4)
    }

    pub fn color_source2(&self) -> u32 {
        bits(self.sources_raw, 4, 4)
    }

    pub fn color_source3(&self) -> u32 {
        bits(self.sources_raw, 8, 4)
    }

    pub fn alpha_source1(&self) -> u32 {
        bits(self.sources_raw, 16, 4)
    }

    pub fn alpha_source2(&self) -> u32 {
        bits(self.sources_raw, 20, 4)
    }

    pub fn alpha_source3(&self) -> u32 {
        bits(self.sources_raw, 24, 4)
    }

    pub fn color_modifier1(&self) -> u32 {
        bits(self.modifiers_raw, 0, 4)
    }

    pub fn color_modifier2(&self) -> u32 {
        bits(self.modifiers_raw, 4, 4)
    }

    pub fn color_modifier3(&self) -> u32 {
        bits(self.modifiers_raw, 8, 4)
    }

    pub fn alpha_modifier1(&self) -> u32 {
        bits(self.modifiers_raw, 12, 3)
    }

    pub fn alpha_modifier2(&self) -> u32 {
        bits(self.modifiers_raw, 16, 3)
    }

    pub fn alpha_modifier3(&self) -> u32 {
        bits(self.modifiers_raw, 20, 3)
    }

    pub fn color_op(&self) -> u32 {
        bits(self.ops_raw, 0, 4)
    }

    pub fn alpha_op(&self) -> u32 {
        bits(self.ops_raw, 16, 4)
    }

    pub fn color_scale(&self) -> u32 {
        bits(self.scales_raw, 0, 2)
    }

    pub fn alpha_scale(&self) -> u32 {
        bits(self.scales_raw, 16, 2)
    }

    pub fn color_multiplier(&self) -> u32 {
        multiplier(self.color_scale())
    }

    pub fn alpha_multiplier(&self) -> u32 {
        multiplier(self.alpha_scale())
    }
}

fn multiplier(scale: u32) -> u32 {
    if scale < 3 { 1 << scale } else { 1 }
}

#[derive(Debug, Clone, Default)]
pub struct TextureConfig {
    pub raw: u32,
    pub texture_border_color: Vec<u32>,
    pub tev_stages: Vec<TevStageConfig>,
}

impl TextureConfig {
    pub fn texture0_type(&self) -> u32 {
        bits(self.raw, 0, 3)
    }

    pub fn texture2_use_coord1(&self) -> bool {
        flag(self.raw, 3)
    }

    pub fn combiner_buffer_input(&self) -> u32 {
        bits(self.raw, 4, 8)
    }

    pub fn fog_mode(&self) -> u32 {
        bits(self.raw, 12, 3)
    }

    pub fn fog_flip(&self) -> bool {
        flag(self.raw, 15)
    }

    pub fn border_enable_s(&self, unit: usize) -> bool {
        flag(self.texture_border_color[unit], 0)
    }

    pub fn border_enable_t(&self, unit: usize) -> bool {
        flag(self.texture_border_color[unit], 1)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LutConfig {
    pub raw: u32,
    pub scale: f32,
}

impl LutConfig {
    pub fn enable(&self) -> bool {
        flag(self.raw, 0)
    }

    pub fn abs_input(&self) -> bool {
        flag(self.raw, 1)
    }

    pub fn input_type(&self) -> u32 {
        bits(self.raw, 2, 3)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Light {
    pub raw: u16,
}

impl Light {
    fn field(&self, position: u32, width: u32) -> u32 {
        bits(u32::from(self.raw), position, width)
    }

    pub fn num(&self) -> u32 {
        self.field(0, 3)
    }

    pub fn directional(&self) -> bool {
        self.field(3, 1) != 0
    }

    pub fn two_sided_diffuse(&self) -> bool {
        self.field(4, 1) != 0
    }

    pub fn dist_atten_enable(&self) -> bool {
        self.field(5, 1) != 0
    }

    pub fn spot_atten_enable(&self) -> bool {
        self.field(6, 1) != 0
    }

    pub fn geometric_factor_0(&self) -> bool {
        self.field(7, 1) != 0
    }

    pub fn geometric_factor_1(&self) -> bool {
        self.field(8, 1) != 0
    }

    pub fn shadow_enable(&self) -> bool {
        self.field(9, 1) != 0
    }
}

#[derive(Debug, Clone, Default)]
pub struct LightConfig {
    pub raw: u32,
    pub lut_d0: LutConfig,
    pub lut_d1: LutConfig,
    pub lut_sp: LutConfig,
    pub lut_fr: LutConfig,
    pub lut_rr: LutConfig,
    pub lut_rg: LutConfig,
    pub lut_rb: LutConfig,
    pub lights: Vec<Light>,
}

impl LightConfig {
    pub fn enable(&self) -> bool {
        flag(self.raw, 0)
    }

    pub fn src_num(&self) -> u32 {
        bits(self.raw, 1, 4)
    }

    pub fn bump_mode(&self) -> u32 {
        bits(self.raw, 5, 2)
    }

    pub fn bump_selector(&self) -> u32 {
        bits(self.raw, 7, 2)
    }

    pub fn bump_renorm(&self) -> bool {
        flag(self.raw, 9)
    }

    pub fn clamp_highlights(&self) -> bool {
        flag(self.raw, 10)
    }

    pub fn config(&self) -> u32 {
        bits(self.raw, 11, 4)
    }

    pub fn enable_primary_alpha(&self) -> bool {
        flag(self.raw, 15)
    }

    pub fn enable_secondary_alpha(&self) -> bool {
        flag(self.raw, 16)
    }

    pub fn enable_shadow(&self) -> bool {
        flag(self.raw, 17)
    }

    pub fn shadow_primary(&self) -> bool {
        flag(self.raw, 18)
    }

    pub fn shadow_secondary(&self) -> bool {
        flag(self.raw, 19)
    }

    pub fn shadow_invert(&self) -> bool {
        flag(self.raw, 20)
    }

    pub fn shadow_alpha(&self) -> bool {
        flag(self.raw, 21)
    }

    pub fn shadow_selector(&self) -> u32 {
        bits(self.raw, 22, 2)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProcTexConfig {
    pub raw: u32,
    pub lut_width: i32,
    pub lut_offset0: i32,
    pub lut_offset1: i32,
    pub lut_offset2: i32,
    pub lut_offset3: i32,
    pub lod_min: u16,
    pub lod_max: u16,
}

impl ProcTexConfig {
    pub fn enable(&self) -> bool {
        flag(self.raw, 0)
    }

    pub fn coord(&self) -> u32 {
        bits(self.raw, 1, 2)
    }

    pub fn u_clamp(&self) -> u32 {
        bits(self.raw, 3, 3)
    }

    pub fn v_clamp(&self) -> u32 {
        bits(self.raw, 6, 3)
    }

    pub fn color_combiner(&self) -> u32 {
        bits(self.raw, 9, 4)
    }

    pub fn alpha_combiner(&self) -> u32 {
        bits(self.raw, 13, 4)
    }

    pub fn lut_filter(&self) -> u32 {
        bits(self.raw, 17, 3)
    }

    pub fn separate_alpha(&self) -> bool {
        flag(self.raw, 20)
    }

    pub fn noise_enable(&self) -> bool {
        flag(self.raw, 21)
    }

    pub fn u_shift(&self) -> u32 {
        bits(self.raw, 22, 2)
    }

    pub fn v_shift(&self) -> u32 {
        bits(self.raw, 24, 2)
    }
}

#[derive(Debug, Clone, Default)]
pub struct FsConfig {
    pub framebuffer: FramebufferConfig,
    pub texture: TextureConfig,
    pub lighting: LightConfig,
    pub proctex: ProcTexConfig,
    pub user: u32,
}

impl FsConfig {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let layout = ConfigLayout::default();
        if bytes.len() != layout.total_size {
            return Err(ShaderError::ConfigSize {
                expected: layout.total_size,
                actual: bytes.len(),
            });
        }
        let mut reader = Reader { bytes, cursor: 0 };

        let framebuffer = FramebufferConfig {
            raw: reader.u32(),
            rgb_blend: read_blend(&mut reader),
            alpha_blend: read_blend(&mut reader),
        };

        let raw = reader.u32();
        let texture_border_color = (0..layout.texture_borders)
            .map(|_| reader.u32())
            .collect();
        let tev_stages = (0..layout.tev_stages)
            .map(|_| TevStageConfig {
                sources_raw: reader.u32(),
                modifiers_raw: reader.u32(),
                ops_raw: reader.u32(),
                scales_raw: reader.u32(),
            })
            .collect();
        let texture = TextureConfig {
            raw,
            texture_border_color,
            tev_stages,
        };

        let raw = reader.u32();
        let lut_d0 = read_lut(&mut reader);
        let lut_d1 = read_lut(&mut reader);
        let lut_sp = read_lut(&mut reader);
        let lut_fr = read_lut(&mut reader);
        let lut_rr = read_lut(&mut reader);
        let lut_rg = read_lut(&mut reader);
        let lut_rb = read_lut(&mut reader);
        let lights = (0..layout.lights)
            .map(|_| Light { raw: reader.u16() })
            .collect();
        let lighting = LightConfig {
            raw,
            lut_d0,
            lut_d1,
            lut_sp,
            lut_fr,
            lut_rr,
            lut_rg,
            lut_rb,
            lights,
        };

        let proctex = ProcTexConfig {
            raw: reader.u32(),
            lut_width: reader.i32(),
            lut_offset0: reader.i32(),
            lut_offset1: reader.i32(),
            lut_offset2: reader.i32(),
            lut_offset3: reader.i32(),
            lod_min: reader.u16(),
            lod_max: reader.u16(),
        };

        let user = reader.u32();

        Ok(Self {
            framebuffer,
            texture,
            lighting,
            proctex,
            user,
        })
    }

    pub fn tev_stage_updates_combiner_buffer_color(&self, stage_index: u32) -> bool {
        stage_index < 4 && self.texture.combiner_buffer_input() & (1 << stage_index) != 0
    }

    pub fn tev_stage_updates_combiner_buffer_alpha(&self, stage_index: u32) -> bool {
        stage_index < 4 && (self.texture.combiner_buffer_input() >> 4) & (1 << stage_index) != 0
    }
}

fn read_blend(reader: &mut Reader) -> BlendConfig {
    BlendConfig {
        eq: reader.u32(),
        src_factor: reader.u32(),
        dst_factor: reader.u32(),
    }
}

fn read_lut(reader: &mut Reader) -> LutConfig {
    LutConfig {
        raw: reader.u32(),
        scale: reader.f32(),
    }
}
