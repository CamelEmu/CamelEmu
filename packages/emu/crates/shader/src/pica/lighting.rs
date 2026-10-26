use emu_spirv::{Id, glsl};

use crate::fragment::FragmentModule;

struct ConfigValues;

impl ConfigValues {
    const CONFIG0: u32 = 0;
    const CONFIG1: u32 = 1;
    const CONFIG2: u32 = 2;
    const CONFIG3: u32 = 3;
    const CONFIG4: u32 = 4;
    const CONFIG5: u32 = 5;
    const CONFIG7: u32 = 8;
}

pub trait LightingSampler {
    fn value(&self, light_num: u32) -> u32;

    fn is_supported(&self, config: u32) -> bool;
}

pub struct Distribution0;
pub struct Distribution1;
pub struct Fresnel;
pub struct ReflectBlue;
pub struct ReflectGreen;
pub struct ReflectRed;
pub struct SpotlightAttenuation;
pub struct DistanceAttenuation;

impl LightingSampler for Distribution0 {
    fn value(&self, _light_num: u32) -> u32 {
        0
    }

    fn is_supported(&self, config: u32) -> bool {
        config != ConfigValues::CONFIG1
    }
}

impl LightingSampler for Distribution1 {
    fn value(&self, _light_num: u32) -> u32 {
        1
    }

    fn is_supported(&self, config: u32) -> bool {
        config != ConfigValues::CONFIG0
            && config != ConfigValues::CONFIG1
            && config != ConfigValues::CONFIG5
    }
}

impl LightingSampler for Fresnel {
    fn value(&self, _light_num: u32) -> u32 {
        3
    }

    fn is_supported(&self, config: u32) -> bool {
        config != ConfigValues::CONFIG0
            && config != ConfigValues::CONFIG2
            && config != ConfigValues::CONFIG4
    }
}

impl LightingSampler for ReflectBlue {
    fn value(&self, _light_num: u32) -> u32 {
        4
    }

    fn is_supported(&self, config: u32) -> bool {
        config == ConfigValues::CONFIG4
            || config == ConfigValues::CONFIG5
            || config == ConfigValues::CONFIG7
    }
}

impl LightingSampler for ReflectGreen {
    fn value(&self, _light_num: u32) -> u32 {
        5
    }

    fn is_supported(&self, config: u32) -> bool {
        config == ConfigValues::CONFIG4
            || config == ConfigValues::CONFIG5
            || config == ConfigValues::CONFIG7
    }
}

impl LightingSampler for ReflectRed {
    fn value(&self, _light_num: u32) -> u32 {
        6
    }

    fn is_supported(&self, config: u32) -> bool {
        config != ConfigValues::CONFIG3
    }
}

impl LightingSampler for SpotlightAttenuation {
    fn value(&self, light_num: u32) -> u32 {
        8 + light_num
    }

    fn is_supported(&self, config: u32) -> bool {
        config != ConfigValues::CONFIG2 && config != ConfigValues::CONFIG3
    }
}

impl LightingSampler for DistanceAttenuation {
    fn value(&self, light_num: u32) -> u32 {
        16 + light_num
    }

    fn is_supported(&self, _config: u32) -> bool {
        true
    }
}

pub struct LutInputContext {
    pub normal: Id,
    pub tangent: Id,
    pub view: Id,
    pub half_vector: Id,
    pub light_vector: Id,
    pub spot_dir: Id,
    pub config: u32,
}

pub trait LightingLutInput {
    fn index(&self, fragment: &mut FragmentModule, context: &LutInputContext) -> Id;
}

pub struct NormalHalf;
pub struct ViewHalf;
pub struct NormalView;
pub struct LightNormal;
pub struct SpotDirection;
pub struct TangentProjection;
pub struct UnknownLutInput;

impl LightingLutInput for NormalHalf {
    fn index(&self, fragment: &mut FragmentModule, context: &LutInputContext) -> Id {
        let (f32_id, vec3) = (fragment.f32_id, fragment.vec_ids.get(3));
        let half = fragment.glsl::<glsl::Normalize>(vec3, &[context.half_vector]);
        fragment.op_dot(f32_id, context.normal, half)
    }
}

impl LightingLutInput for ViewHalf {
    fn index(&self, fragment: &mut FragmentModule, context: &LutInputContext) -> Id {
        let (f32_id, vec3) = (fragment.f32_id, fragment.vec_ids.get(3));
        let view = fragment.glsl::<glsl::Normalize>(vec3, &[context.view]);
        let half = fragment.glsl::<glsl::Normalize>(vec3, &[context.half_vector]);
        fragment.op_dot(f32_id, view, half)
    }
}

impl LightingLutInput for NormalView {
    fn index(&self, fragment: &mut FragmentModule, context: &LutInputContext) -> Id {
        let (f32_id, vec3) = (fragment.f32_id, fragment.vec_ids.get(3));
        let view = fragment.glsl::<glsl::Normalize>(vec3, &[context.view]);
        fragment.op_dot(f32_id, context.normal, view)
    }
}

impl LightingLutInput for LightNormal {
    fn index(&self, fragment: &mut FragmentModule, context: &LutInputContext) -> Id {
        let f32_id = fragment.f32_id;
        fragment.op_dot(f32_id, context.light_vector, context.normal)
    }
}

impl LightingLutInput for SpotDirection {
    fn index(&self, fragment: &mut FragmentModule, context: &LutInputContext) -> Id {
        let f32_id = fragment.f32_id;
        fragment.op_dot(f32_id, context.light_vector, context.spot_dir)
    }
}

impl LightingLutInput for TangentProjection {
    fn index(&self, fragment: &mut FragmentModule, context: &LutInputContext) -> Id {
        let (f32_id, vec3) = (fragment.f32_id, fragment.vec_ids.get(3));
        if context.config != ConfigValues::CONFIG7 {
            return fragment.const_f32(0.0);
        }
        let normalized_half_vector = fragment.glsl::<glsl::Normalize>(vec3, &[context.half_vector]);
        let normal_dot_half_vector =
            fragment.op_dot(f32_id, context.normal, normalized_half_vector);
        let normal_mul_dot =
            fragment.op_vector_times_scalar(vec3, context.normal, normal_dot_half_vector);
        let half_angle_proj = fragment.op_f_sub(vec3, normalized_half_vector, normal_mul_dot);
        fragment.op_dot(f32_id, half_angle_proj, context.tangent)
    }
}

impl LightingLutInput for UnknownLutInput {
    fn index(&self, fragment: &mut FragmentModule, _context: &LutInputContext) -> Id {
        fragment.const_f32(0.0)
    }
}

pub fn lighting_lut_input(raw: u32) -> &'static dyn LightingLutInput {
    match raw {
        0 => &NormalHalf,
        1 => &ViewHalf,
        2 => &NormalView,
        3 => &LightNormal,
        4 => &SpotDirection,
        5 => &TangentProjection,
        _ => &UnknownLutInput,
    }
}

pub trait BumpMode {
    fn surface(&self, fragment: &mut FragmentModule) -> (Id, Id);
}

pub struct NoBump;
pub struct NormalMap;
pub struct TangentMap;

impl BumpMode for NoBump {
    fn surface(&self, fragment: &mut FragmentModule) -> (Id, Id) {
        let surface_normal = fragment.const_f32s(&[0.0, 0.0, 1.0]);
        let surface_tangent = fragment.const_f32s(&[1.0, 0.0, 0.0]);
        (surface_normal, surface_tangent)
    }
}

impl BumpMode for NormalMap {
    fn surface(&self, fragment: &mut FragmentModule) -> (Id, Id) {
        let mut surface_normal = fragment.perturbation();

        if fragment.config.lighting.bump_renorm() {
            let (f32_id, vec3) = (fragment.f32_id, fragment.vec_ids.get(3));
            let normal_x = fragment.composite_extract(f32_id, surface_normal, 0);
            let normal_y = fragment.composite_extract(f32_id, surface_normal, 1);
            let y_mul_y = fragment.op_f_mul(f32_id, normal_y, normal_y);
            let one = fragment.const_f32(1.0);
            let fma = fragment.glsl::<glsl::Fma>(f32_id, &[normal_x, normal_x, y_mul_y]);
            let val = fragment.op_f_sub(f32_id, one, fma);
            let zero = fragment.const_f32(0.0);
            let max = fragment.glsl::<glsl::FMax>(f32_id, &[val, zero]);
            let normal_z = fragment.glsl::<glsl::Sqrt>(f32_id, &[max]);
            surface_normal = fragment.op_composite_construct(vec3, &[normal_x, normal_y, normal_z]);
        }

        let surface_tangent = fragment.const_f32s(&[1.0, 0.0, 0.0]);
        (surface_normal, surface_tangent)
    }
}

impl BumpMode for TangentMap {
    fn surface(&self, fragment: &mut FragmentModule) -> (Id, Id) {
        let surface_tangent = fragment.perturbation();
        let surface_normal = fragment.const_f32s(&[0.0, 0.0, 1.0]);
        (surface_normal, surface_tangent)
    }
}

pub fn bump_mode(raw: u32) -> &'static dyn BumpMode {
    match raw {
        1 => &NormalMap,
        2 => &TangentMap,
        _ => &NoBump,
    }
}
