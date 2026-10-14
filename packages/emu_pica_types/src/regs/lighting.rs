use crate::Vec3;
use crate::bits::{assign, bits, pica_enum, sbits};

pica_enum!(LightingSampler {
    DISTRIBUTION0 = 0,
    DISTRIBUTION1 = 1,
    FRESNEL = 3,
    REFLECT_BLUE = 4,
    REFLECT_GREEN = 5,
    REFLECT_RED = 6,
    SPOTLIGHT_ATTENUATION = 8,
    DISTANCE_ATTENUATION = 16,
});

pub const NUM_LIGHTING_SAMPLER: u32 = 24;

pub fn spotlight_attenuation_sampler(index: u32) -> LightingSampler {
    LightingSampler(LightingSampler::SPOTLIGHT_ATTENUATION.0 + index)
}

pub fn distance_attenuation_sampler(index: u32) -> LightingSampler {
    LightingSampler(LightingSampler::DISTANCE_ATTENUATION.0 + index)
}

pica_enum!(
    /// Pica fragment lighting supports using different LUTs for each lighting component: Reflectance
    /// R, G, and B channels, distribution function for specular components 0 and 1, fresnel factor,
    /// and spotlight attenuation.  Furthermore, which LUTs are used for each channel (or whether a
    /// channel is enabled at all) is specified by various pre-defined lighting configurations.  With
    /// configurations that require more LUTs, more cycles are required on HW to perform lighting
    /// computations.
    LightingConfig {
        CONFIG0 = 0, // Reflect Red, Distribution 0, Spotlight
        CONFIG1 = 1, // Reflect Red, Fresnel, Spotlight
        CONFIG2 = 2, // Reflect Red, Distribution 0/1
        CONFIG3 = 3, // Distribution 0/1, Fresnel
        CONFIG4 = 4, // Reflect Red/Green/Blue, Distribution 0/1, Spotlight
        CONFIG5 = 5, // Reflect Red/Green/Blue, Distribution 0, Fresnel, Spotlight
        CONFIG6 = 6, // Reflect Red, Distribution 0/1, Fresnel, Spotlight

        CONFIG7 = 8, // Reflect Red/Green/Blue, Distribution 0/1, Fresnel, Spotlight
                     // NOTE: '8' is intentional, '7' does not appear to be a valid configuration
    }
);

pica_enum!(
    /// Factor used to scale the output of a lighting LUT
    LightingScale {
        SCALE1 = 0, // Scale is 1x
        SCALE2 = 1, // Scale is 2x
        SCALE4 = 2, // Scale is 4x
        SCALE8 = 3, // Scale is 8x

        SCALE1_4 = 6, // Scale is 0.25x
        SCALE1_2 = 7, // Scale is 0.5x
    }
);

pica_enum!(LightingLutInput {
    NH = 0, // Cosine of the angle between the normal and half-angle vectors
    VH = 1, // Cosine of the angle between the view and half-angle vectors
    NV = 2, // Cosine of the angle between the normal and the view vector
    LN = 3, // Cosine of the angle between the light and the normal vectors
    SP = 4, // Cosine of the angle between the light and the inverse spotlight vectors
    CP = 5, // Cosine of the angle between the tangent and projection of half-angle vectors
});

pica_enum!(LightingBumpMode {
    NONE = 0,
    NORMAL_MAP = 1,
    TANGENT_MAP = 2,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LightColor {
    pub raw: u32,
}

impl LightColor {
    pub fn b(&self) -> u32 {
        bits(self.raw, 0, 10)
    }

    pub fn g(&self) -> u32 {
        bits(self.raw, 10, 10)
    }

    pub fn r(&self) -> u32 {
        bits(self.raw, 20, 10)
    }

    pub fn to_vec3f(&self) -> Vec3<f32> {
        // These fields are 10 bits wide, however 255 corresponds to 1.0f for each color
        // component
        Vec3 {
            x: self.r() as f32 / 255.0,
            y: self.g() as f32 / 255.0,
            z: self.b() as f32 / 255.0,
        }
    }
}

/// Returns true if the specified lighting sampler is supported by the current Pica lighting
/// configuration
pub fn is_lighting_sampler_supported(config: LightingConfig, sampler: LightingSampler) -> bool {
    match sampler {
        LightingSampler::DISTRIBUTION0 => config != LightingConfig::CONFIG1,

        LightingSampler::DISTRIBUTION1 => {
            config != LightingConfig::CONFIG0
                && config != LightingConfig::CONFIG1
                && config != LightingConfig::CONFIG5
        }

        LightingSampler::SPOTLIGHT_ATTENUATION => {
            config != LightingConfig::CONFIG2 && config != LightingConfig::CONFIG3
        }

        LightingSampler::FRESNEL => {
            config != LightingConfig::CONFIG0
                && config != LightingConfig::CONFIG2
                && config != LightingConfig::CONFIG4
        }

        LightingSampler::REFLECT_RED => config != LightingConfig::CONFIG3,

        LightingSampler::REFLECT_GREEN | LightingSampler::REFLECT_BLUE => {
            config == LightingConfig::CONFIG4
                || config == LightingConfig::CONFIG5
                || config == LightingConfig::CONFIG7
        }
        _ => unreachable!(
            "Regs::IsLightingSamplerSupported: Reached unreachable section, sampler should be one \
             of Distribution0, Distribution1, SpotlightAttenuation, Fresnel, ReflectRed, \
             ReflectGreen or ReflectBlue, instead got {}",
            config.0
        ),
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LightSrc {
    pub specular_0: LightColor, // material.specular_0 * light.specular_0
    pub specular_1: LightColor, // material.specular_1 * light.specular_1
    pub diffuse: LightColor,    // material.diffuse * light.diffuse
    pub ambient: LightColor,    // material.ambient * light.ambient

    // Encoded as 16-bit floating point
    pub xy: u32,
    pub z_raw: u32,

    // inverse spotlight direction vector, encoded as fixed1.1.11
    pub spot_xy: u32,
    pub spot_z_raw: u32,

    _padding0: [u32; 0x1],

    pub config: u32,

    pub dist_atten_bias_raw: u32,
    pub dist_atten_scale_raw: u32,

    _padding1: [u32; 0x4],
}

const _: () = assert!(
    size_of::<LightSrc>() == 0x10 * size_of::<u32>(),
    "LightSrc structure must be 0x10 words"
);

impl LightSrc {
    pub fn x(&self) -> u32 {
        bits(self.xy, 0, 16)
    }

    pub fn y(&self) -> u32 {
        bits(self.xy, 16, 16)
    }

    pub fn z(&self) -> u32 {
        bits(self.z_raw, 0, 16)
    }

    pub fn spot_x(&self) -> i32 {
        sbits(self.spot_xy, 0, 13)
    }

    pub fn spot_y(&self) -> i32 {
        sbits(self.spot_xy, 16, 13)
    }

    pub fn spot_z(&self) -> i32 {
        sbits(self.spot_z_raw, 0, 13)
    }

    pub fn directional(&self) -> u32 {
        bits(self.config, 0, 1)
    }

    pub fn two_sided_diffuse(&self) -> u32 {
        bits(self.config, 1, 1) // When disabled, clamp dot-product to 0
    }

    pub fn geometric_factor_0(&self) -> u32 {
        bits(self.config, 2, 1)
    }

    pub fn geometric_factor_1(&self) -> u32 {
        bits(self.config, 3, 1)
    }

    pub fn dist_atten_bias(&self) -> u32 {
        bits(self.dist_atten_bias_raw, 0, 20)
    }

    pub fn dist_atten_scale(&self) -> u32 {
        bits(self.dist_atten_scale_raw, 0, 20)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LightingConfig0 {
    pub raw: u32,
}

impl LightingConfig0 {
    pub fn enable_shadow(&self) -> u32 {
        bits(self.raw, 0, 1)
    }

    pub fn enable_primary_alpha(&self) -> u32 {
        bits(self.raw, 2, 1)
    }

    pub fn enable_secondary_alpha(&self) -> u32 {
        bits(self.raw, 3, 1)
    }

    pub fn config(&self) -> LightingConfig {
        LightingConfig(bits(self.raw, 4, 4))
    }

    pub fn shadow_primary(&self) -> u32 {
        bits(self.raw, 16, 1)
    }

    pub fn shadow_secondary(&self) -> u32 {
        bits(self.raw, 17, 1)
    }

    pub fn shadow_invert(&self) -> u32 {
        bits(self.raw, 18, 1)
    }

    pub fn shadow_alpha(&self) -> u32 {
        bits(self.raw, 19, 1)
    }

    pub fn bump_selector(&self) -> u32 {
        bits(self.raw, 22, 2) // 0: Texture 0, 1: Texture 1, 2: Texture 2
    }

    pub fn shadow_selector(&self) -> u32 {
        bits(self.raw, 24, 2)
    }

    pub fn clamp_highlights(&self) -> u32 {
        bits(self.raw, 27, 1)
    }

    pub fn bump_mode(&self) -> LightingBumpMode {
        LightingBumpMode(bits(self.raw, 28, 2))
    }

    pub fn disable_bump_renorm(&self) -> u32 {
        bits(self.raw, 30, 1)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LightingConfig1 {
    pub raw: u32,
}

impl LightingConfig1 {
    // Each bit specifies whether shadow should be applied for the corresponding light.
    pub fn disable_shadow(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    // Each bit specifies whether spot light attenuation should be applied for the corresponding
    // light.
    pub fn disable_spot_atten(&self) -> u32 {
        bits(self.raw, 8, 8)
    }

    pub fn disable_lut_d0(&self) -> u32 {
        bits(self.raw, 16, 1)
    }

    pub fn disable_lut_d1(&self) -> u32 {
        bits(self.raw, 17, 1)
    }

    // Note: by intuition, BitField<18, 1, u32> should be disable_lut_sp, but it is actually a
    // dummy bit which is always set as 1.
    pub fn disable_lut_fr(&self) -> u32 {
        bits(self.raw, 19, 1)
    }

    pub fn disable_lut_rr(&self) -> u32 {
        bits(self.raw, 20, 1)
    }

    pub fn disable_lut_rg(&self) -> u32 {
        bits(self.raw, 21, 1)
    }

    pub fn disable_lut_rb(&self) -> u32 {
        bits(self.raw, 22, 1)
    }

    // Each bit specifies whether distance attenuation should be applied for the corresponding
    // light.
    pub fn disable_dist_atten(&self) -> u32 {
        bits(self.raw, 24, 8)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LutConfig {
    pub raw: u32,
}

impl LutConfig {
    /// Index at which to set data in the LUT
    pub fn index(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    pub fn set_index(&mut self, index: u32) {
        assign(&mut self.raw, 0, 8, index);
    }

    /// Type of LUT for which to set data
    pub fn lut_type(&self) -> u32 {
        bits(self.raw, 8, 5)
    }
}

// These are used to specify if absolute (abs) value should be used for each LUT index.  When
// abs mode is disabled, LUT indexes are in the range of (-1.0, 1.0).  Otherwise, they are in
// the range of (0.0, 1.0).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AbsLutInput {
    pub raw: u32,
}

impl AbsLutInput {
    pub fn disable_d0(&self) -> u32 {
        bits(self.raw, 1, 1)
    }

    pub fn disable_d1(&self) -> u32 {
        bits(self.raw, 5, 1)
    }

    pub fn disable_sp(&self) -> u32 {
        bits(self.raw, 9, 1)
    }

    pub fn disable_fr(&self) -> u32 {
        bits(self.raw, 13, 1)
    }

    pub fn disable_rb(&self) -> u32 {
        bits(self.raw, 17, 1)
    }

    pub fn disable_rg(&self) -> u32 {
        bits(self.raw, 21, 1)
    }

    pub fn disable_rr(&self) -> u32 {
        bits(self.raw, 25, 1)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LutInput {
    pub raw: u32,
}

impl LutInput {
    pub fn d0(&self) -> LightingLutInput {
        LightingLutInput(bits(self.raw, 0, 3))
    }

    pub fn d1(&self) -> LightingLutInput {
        LightingLutInput(bits(self.raw, 4, 3))
    }

    pub fn sp(&self) -> LightingLutInput {
        LightingLutInput(bits(self.raw, 8, 3))
    }

    pub fn fr(&self) -> LightingLutInput {
        LightingLutInput(bits(self.raw, 12, 3))
    }

    pub fn rb(&self) -> LightingLutInput {
        LightingLutInput(bits(self.raw, 16, 3))
    }

    pub fn rg(&self) -> LightingLutInput {
        LightingLutInput(bits(self.raw, 20, 3))
    }

    pub fn rr(&self) -> LightingLutInput {
        LightingLutInput(bits(self.raw, 24, 3))
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LutScale {
    pub raw: u32,
}

impl LutScale {
    pub fn d0(&self) -> LightingScale {
        LightingScale(bits(self.raw, 0, 3))
    }

    pub fn d1(&self) -> LightingScale {
        LightingScale(bits(self.raw, 4, 3))
    }

    pub fn sp(&self) -> LightingScale {
        LightingScale(bits(self.raw, 8, 3))
    }

    pub fn fr(&self) -> LightingScale {
        LightingScale(bits(self.raw, 12, 3))
    }

    pub fn rb(&self) -> LightingScale {
        LightingScale(bits(self.raw, 16, 3))
    }

    pub fn rg(&self) -> LightingScale {
        LightingScale(bits(self.raw, 20, 3))
    }

    pub fn rr(&self) -> LightingScale {
        LightingScale(bits(self.raw, 24, 3))
    }

    pub fn get_scale(scale: LightingScale) -> f32 {
        match scale {
            LightingScale::SCALE1 => 1.0,
            LightingScale::SCALE2 => 2.0,
            LightingScale::SCALE4 => 4.0,
            LightingScale::SCALE8 => 8.0,
            LightingScale::SCALE1_4 => 0.25,
            LightingScale::SCALE1_2 => 0.5,
            _ => 0.0,
        }
    }
}

// There are 8 light enable "slots", corresponding to the total number of lights supported
// by Pica.  For N enabled lights (specified by register 0x1c2, or 'src_num' above), the
// first N slots below will be set to integers within the range of 0-7, corresponding to the
// actual light that is enabled for each slot.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LightEnable {
    pub raw: u32,
}

impl LightEnable {
    pub fn get_num(&self, index: u32) -> u32 {
        bits(self.raw, index * 4, 3)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct LightingRegs {
    pub light: [LightSrc; 8],
    pub global_ambient: LightColor, // Emission + (material.ambient * lighting.ambient)
    _padding0: [u32; 0x1],
    pub max_light_index_raw: u32, // Number of enabled lights - 1
    pub config0: LightingConfig0,
    pub config1: LightingConfig1,
    pub lut_config: LutConfig,
    pub disable_raw: u32,
    _padding1: [u32; 0x1],

    // When data is written to any of these registers, it gets written to the lookup table of the
    // selected type at the selected index, specified above in the `lut_config` register.  With each
    // write, `lut_config.index` is incremented.  It does not matter which of these registers is
    // written to, the behavior will be the same.
    pub lut_data: [u32; 8],

    pub abs_lut_input: AbsLutInput,
    pub lut_input: LutInput,
    pub lut_scale: LutScale,

    _padding2: [u32; 0x6],

    pub light_enable: LightEnable,

    _padding3: [u32; 0x26],
}

const _: () = assert!(
    size_of::<LightingRegs>() == 0xC0 * size_of::<u32>(),
    "LightingRegs struct has incorrect size"
);

impl Default for LightingRegs {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl LightingRegs {
    pub fn max_light_index(&self) -> u32 {
        bits(self.max_light_index_raw, 0, 3)
    }

    pub fn disable(&self) -> u32 {
        bits(self.disable_raw, 0, 1)
    }

    pub fn is_dist_atten_disabled(&self, index: u32) -> bool {
        self.config1.disable_dist_atten() & (1 << index) != 0
    }

    pub fn is_spot_atten_disabled(&self, index: u32) -> bool {
        self.config1.disable_spot_atten() & (1 << index) != 0
    }

    pub fn is_shadow_disabled(&self, index: u32) -> bool {
        self.config1.disable_shadow() & (1 << index) != 0
    }
}
