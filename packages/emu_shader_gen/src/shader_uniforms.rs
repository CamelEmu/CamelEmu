use emu_pica_types::regs::ShaderRegs;
use emu_pica_types::regs::lighting::NUM_LIGHTING_SAMPLER;
use emu_pica_types::{F24, Vec4};

/// A value placed on a 16-byte boundary, as `alignas(16)` does.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Align16<T>(pub T);

/// A value placed on an 8-byte boundary, as `alignas(8)` does.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Align8<T>(pub T);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightSrc {
    pub specular_0: Align16<[f32; 3]>,
    pub specular_1: Align16<[f32; 3]>,
    pub diffuse: Align16<[f32; 3]>,
    pub ambient: Align16<[f32; 3]>,
    pub position: Align16<[f32; 3]>,
    pub spot_direction: Align16<[f32; 3]>, // negated
    pub dist_atten_bias: f32,
    pub dist_atten_scale: f32,
}

/**
 * Uniform structure for the Uniform Buffer Object, all vectors must be 16-byte aligned
 * NOTE: Always keep a vec4 at the end. The GL spec is not clear wether the alignment at
 *       the end of a uniform block is included in UNIFORM_BLOCK_DATA_SIZE or not.
 *       Not following that rule will cause problems on some AMD drivers.
 */
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FSUniformData {
    pub framebuffer_scale: i32,
    pub alphatest_ref: i32,
    pub depth_scale: f32,
    pub depth_offset: f32,
    pub shadow_bias_constant: f32,
    pub shadow_bias_linear: f32,
    pub scissor_x1: i32,
    pub scissor_y1: i32,
    pub scissor_x2: i32,
    pub scissor_y2: i32,
    pub fog_lut_offset: i32,
    pub proctex_noise_lut_offset: i32,
    pub proctex_color_map_offset: i32,
    pub proctex_alpha_map_offset: i32,
    pub proctex_lut_offset: i32,
    pub proctex_diff_lut_offset: i32,
    pub proctex_bias: f32,
    pub shadow_texture_bias: i32,
    pub lighting_lut_offset: Align16<[[i32; 4]; NUM_LIGHTING_SAMPLER as usize / 4]>,
    pub fog_color: Align16<[f32; 3]>,
    pub proctex_noise_f: Align8<[f32; 2]>,
    pub proctex_noise_a: Align8<[f32; 2]>,
    pub proctex_noise_p: Align8<[f32; 2]>,
    pub lighting_global_ambient: Align16<[f32; 3]>,
    pub light_src: [LightSrc; 8],
    pub const_color: Align16<[[f32; 4]; 6]>, // A vec4 color for each of the six tev stages
    pub tev_combiner_buffer_color: Align16<[f32; 4]>,
    pub tex_lod_bias: Align16<[f32; 3]>,
    pub tex_border_color: Align16<[[f32; 4]; 3]>,
    pub blend_color: Align16<[f32; 4]>,
}

const _: () = assert!(
    size_of::<FSUniformData>() == 0x530,
    "The size of the UniformData does not match the structure in the shader"
);
const _: () = assert!(
    size_of::<FSUniformData>() < 16384,
    "UniformData structure must be less than 16kb as per the OpenGL spec"
);

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoolAligned {
    pub b: i32,
}

/**
 * Uniform struct for the Uniform Buffer Object that contains PICA vertex/geometry shader uniforms.
 * NOTE: the same rule from UniformData also applies here.
 */
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PicaUniformsData {
    pub bools: [BoolAligned; 16],
    pub i: Align16<[[u32; 4]; 4]>,
    pub f: Align16<[[f32; 4]; 96]>,
}

impl Default for PicaUniformsData {
    fn default() -> Self {
        Self {
            bools: [BoolAligned::default(); 16],
            i: Align16([[0; 4]; 4]),
            f: Align16([[0.0; 4]; 96]),
        }
    }
}

impl PicaUniformsData {
    /// The boolean and float uniforms come from the `ShaderSetup` of the shader.
    pub fn set_from_regs(
        &mut self,
        regs: &ShaderRegs,
        bool_uniforms: &[bool; 16],
        float_uniforms: &[Vec4<F24>; 96],
    ) {
        for (dest, &value) in self.bools.iter_mut().zip(bool_uniforms) {
            *dest = BoolAligned {
                b: if value { 1 } else { 0 },
            };
        }
        for (index, dest) in self.i.0.iter_mut().enumerate() {
            let value = regs.get_int_uniform(index as u32);
            *dest = [
                u32::from(value.x),
                u32::from(value.y),
                u32::from(value.z),
                u32::from(value.w),
            ];
        }
        for (dest, value) in self.f.0.iter_mut().zip(float_uniforms) {
            *dest = [
                value.x.to_float32(),
                value.y.to_float32(),
                value.z.to_float32(),
                value.w.to_float32(),
            ];
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VSUniformData {
    pub enable_clip1: u32,
    pub flip_viewport: u32,
    pub clip_coef: Align16<[f32; 4]>,
}

const _: () = assert!(
    size_of::<VSUniformData>() == 32,
    "The size of the VSUniformData does not match the structure in the shader"
);
const _: () = assert!(
    size_of::<VSUniformData>() < 16384,
    "VSUniformData structure must be less than 16kb as per the OpenGL spec"
);

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VSPicaUniformData {
    pub uniforms: PicaUniformsData,
}

const _: () = assert!(
    size_of::<VSPicaUniformData>() == 1856,
    "The size of the VSPicaUniformData does not match the structure in the shader"
);
const _: () = assert!(
    size_of::<VSPicaUniformData>() < 16384,
    "VSPicaUniformData structure must be less than 16kb as per the OpenGL spec"
);
