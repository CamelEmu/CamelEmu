use std::hash::{Hash, Hasher};

use emu_pica_types::bits::{assign, bits};
use emu_pica_types::regs::RegsInternal;
use emu_pica_types::regs::framebuffer::{BlendEquation, BlendFactor, CompareFunc, LogicOp};
use emu_pica_types::regs::lighting::{
    LightingBumpMode, LightingConfig, LightingLutInput, LightingRegs, LutScale,
};
use emu_pica_types::regs::rasterizer::{DepthBuffering, ScissorMode};
use emu_pica_types::regs::texturing::{
    FogMode, ProcTexClamp, ProcTexCombiner, ProcTexFilter, ProcTexShift, TevOperation,
    TevStageConfig, TextureType, TexturingRegs, WrapMode,
};

use crate::profile::Profile;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct BlendConfig {
    pub eq: BlendEquation,
    pub src_factor: BlendFactor,
    pub dst_factor: BlendFactor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FramebufferConfig {
    pub raw: u32,
    pub rgb_blend: BlendConfig,
    pub alpha_blend: BlendConfig,
}

impl FramebufferConfig {
    pub fn new(regs: &RegsInternal, profile: &Profile) -> Self {
        let mut config = Self::default();
        let output_merger = &regs.framebuffer.output_merger;
        assign(&mut config.raw, 3, 2, regs.rasterizer.scissor_test.mode().0);
        assign(&mut config.raw, 5, 1, regs.rasterizer.depthmap_enable().0);
        assign(
            &mut config.raw,
            10,
            1,
            u32::from(regs.framebuffer.is_shadow_rendering()),
        );
        assign(
            &mut config.raw,
            0,
            3,
            if output_merger.alpha_test_enable() != 0 {
                output_merger.alpha_test_func().0
            } else {
                CompareFunc::ALWAYS.0
            },
        );

        // Emulate logic op in the shader if needed and not supported.
        assign(&mut config.raw, 6, 4, LogicOp::COPY.0);
        if !profile.has_logic_op && output_merger.alphablend_enable() == 0 {
            assign(&mut config.raw, 6, 4, output_merger.logic_op().0);
        }

        let alpha_eq = output_merger.blend_equation_a();
        let rgb_eq = output_merger.blend_equation_rgb();
        if !profile.has_blend_minmax_factor && output_merger.alphablend_enable() != 0 {
            if rgb_eq == BlendEquation::MAX || rgb_eq == BlendEquation::MIN {
                config.rgb_blend.eq = rgb_eq;
                config.rgb_blend.src_factor = output_merger.factor_source_rgb();
                config.rgb_blend.dst_factor = output_merger.factor_dest_rgb();
            }
            if alpha_eq == BlendEquation::MAX || alpha_eq == BlendEquation::MIN {
                config.alpha_blend.eq = alpha_eq;
                config.alpha_blend.src_factor = output_merger.factor_source_a();
                config.alpha_blend.dst_factor = output_merger.factor_dest_a();
            }
        }
        config
    }

    pub fn alpha_test_func(&self) -> CompareFunc {
        CompareFunc(bits(self.raw, 0, 3))
    }

    pub fn scissor_test_mode(&self) -> ScissorMode {
        ScissorMode(bits(self.raw, 3, 2))
    }

    pub fn depthmap_enable(&self) -> DepthBuffering {
        DepthBuffering(bits(self.raw, 5, 1))
    }

    pub fn logic_op(&self) -> LogicOp {
        LogicOp(bits(self.raw, 6, 4))
    }

    pub fn shadow_rendering(&self) -> u32 {
        bits(self.raw, 10, 1)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TevStageConfigRaw {
    pub sources_raw: u32,
    pub modifiers_raw: u32,
    pub ops_raw: u32,
    pub scales_raw: u32,
}

impl From<TevStageConfigRaw> for TevStageConfig {
    fn from(raw: TevStageConfigRaw) -> Self {
        TevStageConfig {
            sources_raw: raw.sources_raw,
            modifiers_raw: raw.modifiers_raw,
            ops_raw: raw.ops_raw,
            const_color: 0,
            scales_raw: raw.scales_raw,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureBorder {
    pub raw: u32,
}

impl TextureBorder {
    pub fn enable_s(&self) -> u32 {
        bits(self.raw, 0, 1)
    }

    pub fn enable_t(&self) -> u32 {
        bits(self.raw, 1, 1)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureConfig {
    pub raw: u32,
    pub texture_border_color: [TextureBorder; 3],
    pub tev_stages: [TevStageConfigRaw; 6],
}

impl TextureConfig {
    pub fn new(regs: &TexturingRegs, profile: &Profile) -> Self {
        let mut config = Self::default();
        assign(&mut config.raw, 0, 3, regs.texture0.texture_type().0);
        assign(
            &mut config.raw,
            3,
            1,
            u32::from(regs.main_config.texture2_use_coord1() != 0),
        );
        assign(
            &mut config.raw,
            4,
            8,
            regs.fog_mode.update_mask_rgb() | (regs.fog_mode.update_mask_a() << 4),
        );
        assign(&mut config.raw, 12, 3, regs.fog_mode.fog_mode().0);
        assign(
            &mut config.raw,
            15,
            1,
            u32::from(regs.fog_mode.fog_flip() != 0),
        );
        assign(
            &mut config.raw,
            16,
            1,
            u32::from(regs.shadow_orthographic() != 0),
        );

        // Emulate custom border color if needed and not supported.
        let pica_textures = regs.get_textures();
        for (tex_index, pica_texture) in pica_textures.iter().enumerate() {
            let config_regs = &pica_texture.config;
            let border = &mut config.texture_border_color[tex_index];
            assign(
                &mut border.raw,
                0,
                1,
                u32::from(
                    !profile.has_custom_border_color
                        && config_regs.wrap_s() == WrapMode::CLAMP_TO_BORDER,
                ),
            );
            assign(
                &mut border.raw,
                1,
                1,
                u32::from(
                    !profile.has_custom_border_color
                        && config_regs.wrap_t() == WrapMode::CLAMP_TO_BORDER,
                ),
            );
        }

        let stages = regs.get_tev_stages();
        for (tev_stage, stage) in stages.iter().zip(config.tev_stages.iter_mut()) {
            stage.sources_raw = tev_stage.sources_raw;
            stage.modifiers_raw = tev_stage.modifiers_raw;
            stage.ops_raw = tev_stage.ops_raw;
            stage.scales_raw = tev_stage.scales_raw;
            if tev_stage.color_op() == TevOperation::DOT3_RGBA {
                stage.sources_raw &= 0xFFF;
                stage.modifiers_raw &= 0xFFF;
                stage.ops_raw &= 0xF;
            }
        }
        config
    }

    pub fn texture0_type(&self) -> TextureType {
        TextureType(bits(self.raw, 0, 3))
    }

    pub fn texture2_use_coord1(&self) -> u32 {
        bits(self.raw, 3, 1)
    }

    pub fn combiner_buffer_input(&self) -> u32 {
        bits(self.raw, 4, 8)
    }

    pub fn fog_mode(&self) -> FogMode {
        FogMode(bits(self.raw, 12, 3))
    }

    pub fn fog_flip(&self) -> u32 {
        bits(self.raw, 15, 1)
    }

    pub fn shadow_texture_orthographic(&self) -> u32 {
        bits(self.raw, 16, 1)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Light {
    pub raw: u16,
}

impl Light {
    fn get(&self, position: u32, width: u32) -> u16 {
        bits(u32::from(self.raw), position, width) as u16
    }

    fn set(&mut self, position: u32, width: u32, value: u32) {
        let mut raw = u32::from(self.raw);
        assign(&mut raw, position, width, value);
        self.raw = raw as u16;
    }

    pub fn num(&self) -> u16 {
        self.get(0, 3)
    }

    pub fn directional(&self) -> u16 {
        self.get(3, 1)
    }

    pub fn two_sided_diffuse(&self) -> u16 {
        self.get(4, 1)
    }

    pub fn dist_atten_enable(&self) -> u16 {
        self.get(5, 1)
    }

    pub fn spot_atten_enable(&self) -> u16 {
        self.get(6, 1)
    }

    pub fn geometric_factor_0(&self) -> u16 {
        self.get(7, 1)
    }

    pub fn geometric_factor_1(&self) -> u16 {
        self.get(8, 1)
    }

    pub fn shadow_enable(&self) -> u16 {
        self.get(9, 1)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LutConfig {
    pub raw: u32,
    pub scale: f32,
}

impl PartialEq for LutConfig {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw && self.scale.to_bits() == other.scale.to_bits()
    }
}

impl Eq for LutConfig {}

impl Hash for LutConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
        self.scale.to_bits().hash(state);
    }
}

impl LutConfig {
    pub fn enable(&self) -> u32 {
        bits(self.raw, 0, 1)
    }

    pub fn abs_input(&self) -> u32 {
        bits(self.raw, 1, 1)
    }

    pub fn ty(&self) -> LightingLutInput {
        LightingLutInput(bits(self.raw, 2, 3))
    }

    fn setup(&mut self, enable: bool, disable_abs: u32, ty: LightingLutInput, scale: f32) {
        assign(&mut self.raw, 0, 1, u32::from(enable));
        if self.enable() != 0 {
            assign(&mut self.raw, 1, 1, u32::from(disable_abs == 0));
            assign(&mut self.raw, 2, 3, ty.0);
            self.scale = scale;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LightConfig {
    pub raw: u32,
    pub lut_d0: LutConfig,
    pub lut_d1: LutConfig,
    pub lut_sp: LutConfig,
    pub lut_fr: LutConfig,
    pub lut_rr: LutConfig,
    pub lut_rg: LutConfig,
    pub lut_rb: LutConfig,
    pub lights: [Light; 8],
}

impl LightConfig {
    pub fn new(regs: &LightingRegs) -> Self {
        let mut config = Self::default();
        if regs.disable() != 0 {
            return config;
        }

        let config0 = &regs.config0;
        assign(&mut config.raw, 0, 1, 1);
        assign(&mut config.raw, 1, 4, regs.max_light_index() + 1);
        assign(&mut config.raw, 11, 4, config0.config().0);
        assign(&mut config.raw, 15, 1, config0.enable_primary_alpha());
        assign(&mut config.raw, 16, 1, config0.enable_secondary_alpha());
        assign(&mut config.raw, 5, 2, config0.bump_mode().0);
        assign(&mut config.raw, 7, 2, config0.bump_selector());
        assign(
            &mut config.raw,
            9,
            1,
            u32::from(config0.disable_bump_renorm() == 0),
        );
        assign(
            &mut config.raw,
            10,
            1,
            u32::from(config0.clamp_highlights() != 0),
        );

        assign(
            &mut config.raw,
            17,
            1,
            u32::from(config0.enable_shadow() != 0),
        );
        if config.enable_shadow() != 0 {
            assign(
                &mut config.raw,
                18,
                1,
                u32::from(config0.shadow_primary() != 0),
            );
            assign(
                &mut config.raw,
                19,
                1,
                u32::from(config0.shadow_secondary() != 0),
            );
            assign(
                &mut config.raw,
                20,
                1,
                u32::from(config0.shadow_invert() != 0),
            );
            assign(
                &mut config.raw,
                21,
                1,
                u32::from(config0.shadow_alpha() != 0),
            );
            assign(&mut config.raw, 22, 2, config0.shadow_selector());
        }

        for light_index in 0..=regs.max_light_index() {
            let num = regs.light_enable.get_num(light_index);
            let light_regs = &regs.light[num as usize];
            let light = &mut config.lights[light_index as usize];
            light.set(0, 3, num);
            light.set(3, 1, u32::from(light_regs.directional() != 0));
            light.set(4, 1, u32::from(light_regs.two_sided_diffuse() != 0));
            light.set(7, 1, u32::from(light_regs.geometric_factor_0() != 0));
            light.set(8, 1, u32::from(light_regs.geometric_factor_1() != 0));
            light.set(5, 1, u32::from(!regs.is_dist_atten_disabled(num)));
            light.set(6, 1, u32::from(!regs.is_spot_atten_disabled(num)));
            light.set(9, 1, u32::from(!regs.is_shadow_disabled(num)));
        }

        let config1 = &regs.config1;
        let abs = &regs.abs_lut_input;
        let input = &regs.lut_input;
        let scale = &regs.lut_scale;

        config.lut_d0.setup(
            config1.disable_lut_d0() == 0,
            abs.disable_d0(),
            input.d0(),
            LutScale::get_scale(scale.d0()),
        );

        config.lut_d1.setup(
            config1.disable_lut_d1() == 0,
            abs.disable_d1(),
            input.d1(),
            LutScale::get_scale(scale.d1()),
        );

        // This is a dummy field due to lack of the corresponding register
        config.lut_sp.setup(
            true,
            abs.disable_sp(),
            input.sp(),
            LutScale::get_scale(scale.sp()),
        );

        config.lut_fr.setup(
            config1.disable_lut_fr() == 0,
            abs.disable_fr(),
            input.fr(),
            LutScale::get_scale(scale.fr()),
        );

        config.lut_rr.setup(
            config1.disable_lut_rr() == 0,
            abs.disable_rr(),
            input.rr(),
            LutScale::get_scale(scale.rr()),
        );

        config.lut_rg.setup(
            config1.disable_lut_rg() == 0,
            abs.disable_rg(),
            input.rg(),
            LutScale::get_scale(scale.rg()),
        );

        config.lut_rb.setup(
            config1.disable_lut_rb() == 0,
            abs.disable_rb(),
            input.rb(),
            LutScale::get_scale(scale.rb()),
        );
        config
    }

    pub fn enable(&self) -> u32 {
        bits(self.raw, 0, 1)
    }

    pub fn src_num(&self) -> u32 {
        bits(self.raw, 1, 4)
    }

    pub fn bump_mode(&self) -> LightingBumpMode {
        LightingBumpMode(bits(self.raw, 5, 2))
    }

    pub fn bump_selector(&self) -> u32 {
        bits(self.raw, 7, 2)
    }

    pub fn bump_renorm(&self) -> u32 {
        bits(self.raw, 9, 1)
    }

    pub fn clamp_highlights(&self) -> u32 {
        bits(self.raw, 10, 1)
    }

    pub fn config(&self) -> LightingConfig {
        LightingConfig(bits(self.raw, 11, 4))
    }

    pub fn enable_primary_alpha(&self) -> u32 {
        bits(self.raw, 15, 1)
    }

    pub fn enable_secondary_alpha(&self) -> u32 {
        bits(self.raw, 16, 1)
    }

    pub fn enable_shadow(&self) -> u32 {
        bits(self.raw, 17, 1)
    }

    pub fn shadow_primary(&self) -> u32 {
        bits(self.raw, 18, 1)
    }

    pub fn shadow_secondary(&self) -> u32 {
        bits(self.raw, 19, 1)
    }

    pub fn shadow_invert(&self) -> u32 {
        bits(self.raw, 20, 1)
    }

    pub fn shadow_alpha(&self) -> u32 {
        bits(self.raw, 21, 1)
    }

    pub fn shadow_selector(&self) -> u32 {
        bits(self.raw, 22, 2)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
    pub fn new(regs: &TexturingRegs) -> Self {
        let mut config = Self::default();
        if regs.main_config.texture3_enable() == 0 {
            return config;
        }

        assign(&mut config.raw, 0, 1, 1);
        assign(
            &mut config.raw,
            1,
            2,
            regs.main_config.texture3_coordinates(),
        );
        assign(&mut config.raw, 3, 3, regs.proctex.u_clamp().0);
        assign(&mut config.raw, 6, 3, regs.proctex.v_clamp().0);
        assign(&mut config.raw, 9, 4, regs.proctex.color_combiner().0);
        assign(&mut config.raw, 13, 4, regs.proctex.alpha_combiner().0);
        assign(&mut config.raw, 20, 1, regs.proctex.separate_alpha());
        assign(&mut config.raw, 21, 1, regs.proctex.noise_enable());
        assign(&mut config.raw, 22, 2, regs.proctex.u_shift().0);
        assign(&mut config.raw, 24, 2, regs.proctex.v_shift().0);
        config.lut_width = regs.proctex_lut.width() as i32;
        config.lut_offset0 = regs.proctex_lut_offset.level0() as i32;
        config.lut_offset1 = regs.proctex_lut_offset.level1() as i32;
        config.lut_offset2 = regs.proctex_lut_offset.level2() as i32;
        config.lut_offset3 = regs.proctex_lut_offset.level3() as i32;
        config.lod_min = regs.proctex_lut.lod_min() as u16;
        config.lod_max = regs.proctex_lut.lod_max() as u16;
        assign(&mut config.raw, 17, 3, regs.proctex_lut.filter().0);
        config
    }

    pub fn enable(&self) -> u32 {
        bits(self.raw, 0, 1)
    }

    pub fn coord(&self) -> u32 {
        bits(self.raw, 1, 2)
    }

    pub fn u_clamp(&self) -> ProcTexClamp {
        ProcTexClamp(bits(self.raw, 3, 3))
    }

    pub fn v_clamp(&self) -> ProcTexClamp {
        ProcTexClamp(bits(self.raw, 6, 3))
    }

    pub fn color_combiner(&self) -> ProcTexCombiner {
        ProcTexCombiner(bits(self.raw, 9, 4))
    }

    pub fn alpha_combiner(&self) -> ProcTexCombiner {
        ProcTexCombiner(bits(self.raw, 13, 4))
    }

    pub fn lut_filter(&self) -> ProcTexFilter {
        ProcTexFilter(bits(self.raw, 17, 3))
    }

    pub fn separate_alpha(&self) -> u32 {
        bits(self.raw, 20, 1)
    }

    pub fn noise_enable(&self) -> u32 {
        bits(self.raw, 21, 1)
    }

    pub fn u_shift(&self) -> ProcTexShift {
        ProcTexShift(bits(self.raw, 22, 2))
    }

    pub fn v_shift(&self) -> ProcTexShift {
        ProcTexShift(bits(self.raw, 24, 2))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct UserConfig {
    pub raw: u32,
}

impl UserConfig {
    pub fn use_custom_normal(&self) -> u32 {
        bits(self.raw, 0, 1)
    }

    pub fn set_use_custom_normal(&mut self, value: bool) {
        assign(&mut self.raw, 0, 1, u32::from(value));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FSConfig {
    pub framebuffer: FramebufferConfig,
    pub texture: TextureConfig,
    pub lighting: LightConfig,
    pub proctex: ProcTexConfig,
    pub user: UserConfig,
}

impl FSConfig {
    pub fn new(regs: &RegsInternal, user: &UserConfig, profile: &Profile) -> Self {
        Self {
            framebuffer: FramebufferConfig::new(regs, profile),
            texture: TextureConfig::new(&regs.texturing, profile),
            lighting: LightConfig::new(&regs.lighting),
            proctex: ProcTexConfig::new(&regs.texturing),
            user: *user,
        }
    }

    pub fn tev_stage_updates_combiner_buffer_color(&self, stage_index: u32) -> bool {
        (stage_index < 4) && (self.texture.combiner_buffer_input() & (1 << stage_index)) != 0
    }

    pub fn tev_stage_updates_combiner_buffer_alpha(&self, stage_index: u32) -> bool {
        (stage_index < 4) && ((self.texture.combiner_buffer_input() >> 4) & (1 << stage_index)) != 0
    }

    pub fn emulate_blend(&self) -> bool {
        self.framebuffer.rgb_blend.eq != BlendEquation::ADD
            || self.framebuffer.alpha_blend.eq != BlendEquation::ADD
    }

    pub fn uses_shadow_pipeline(&self) -> bool {
        let texture0_type = self.texture.texture0_type();
        texture0_type == TextureType::SHADOW_2D
            || texture0_type == TextureType::SHADOW_CUBE
            || self.framebuffer.shadow_rendering() != 0
    }
}
