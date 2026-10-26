use emu_spirv::{Id, glsl};

use crate::fragment::FragmentModule;

pub trait ProcTexClamp {
    fn is_mirrored_repeat(&self) -> bool {
        false
    }

    fn apply(&self, fragment: &mut FragmentModule, var: Id, zero: Id, one: Id) -> Id;
}

pub struct ToZero;
pub struct ToEdge;
pub struct SymmetricalRepeat;
pub struct MirroredRepeat;
pub struct Pulse;
pub struct UnknownClamp;

impl ProcTexClamp for ToZero {
    fn apply(&self, fragment: &mut FragmentModule, var: Id, zero: Id, one: Id) -> Id {
        let (f32_id, bool_id) = (fragment.f32_id, fragment.bool_id);
        let greater = fragment.op_f_ord_greater_than(bool_id, var, one);
        fragment.op_select(f32_id, greater, zero, var)
    }
}

impl ProcTexClamp for ToEdge {
    fn apply(&self, fragment: &mut FragmentModule, var: Id, _zero: Id, one: Id) -> Id {
        let f32_id = fragment.f32_id;
        fragment.glsl::<glsl::FMin>(f32_id, &[var, one])
    }
}

impl ProcTexClamp for SymmetricalRepeat {
    fn apply(&self, fragment: &mut FragmentModule, var: Id, _zero: Id, _one: Id) -> Id {
        let f32_id = fragment.f32_id;
        fragment.glsl::<glsl::Fract>(f32_id, &[var])
    }
}

impl ProcTexClamp for MirroredRepeat {
    fn is_mirrored_repeat(&self) -> bool {
        true
    }

    fn apply(&self, fragment: &mut FragmentModule, var: Id, _zero: Id, one: Id) -> Id {
        let (f32_id, i32_id, bool_id) = (fragment.f32_id, fragment.i32_id, fragment.bool_id);
        let fract = fragment.glsl::<glsl::Fract>(f32_id, &[var]);
        let var_i32 = fragment.op_convert_f_to_s(i32_id, var);
        let two = fragment.const_s32(2);
        let remainder = fragment.op_s_mod(i32_id, var_i32, two);
        let zero_i32 = fragment.const_s32(0);
        let cond = fragment.op_i_equal(bool_id, remainder, zero_i32);
        let inverse = fragment.op_f_sub(f32_id, one, fract);
        fragment.op_select(f32_id, cond, fract, inverse)
    }
}

impl ProcTexClamp for Pulse {
    fn apply(&self, fragment: &mut FragmentModule, var: Id, zero: Id, one: Id) -> Id {
        let (f32_id, bool_id) = (fragment.f32_id, fragment.bool_id);
        let threshold = fragment.settings.proctex.pulse_threshold;
        let half = fragment.const_f32(threshold);
        let greater = fragment.op_f_ord_greater_than(bool_id, var, half);
        fragment.op_select(f32_id, greater, one, zero)
    }
}

impl ProcTexClamp for UnknownClamp {
    fn apply(&self, fragment: &mut FragmentModule, var: Id, _zero: Id, one: Id) -> Id {
        let f32_id = fragment.f32_id;
        fragment.glsl::<glsl::FMin>(f32_id, &[var, one])
    }
}

pub fn proctex_clamp(raw: u32) -> &'static dyn ProcTexClamp {
    match raw {
        0 => &ToZero,
        1 => &ToEdge,
        2 => &SymmetricalRepeat,
        3 => &MirroredRepeat,
        4 => &Pulse,
        _ => &UnknownClamp,
    }
}

pub trait ProcTexShift {
    fn offset(&self, fragment: &mut FragmentModule, offset: Id, v_i32: Id) -> Id;
}

pub struct NoShift;
pub struct OddShift;
pub struct EvenShift;

fn shift(fragment: &mut FragmentModule, offset: Id, v_i32: Id, even: bool) -> Id {
    let (f32_id, i32_id) = (fragment.f32_id, fragment.i32_id);
    let dividend = if even {
        let one = fragment.const_s32(1);
        fragment.op_i_add(i32_id, v_i32, one)
    } else {
        v_i32
    };
    let two = fragment.const_s32(2);
    let temp1 = fragment.op_s_div(i32_id, dividend, two);
    let two = fragment.const_s32(2);
    let remainder = fragment.op_s_mod(i32_id, temp1, two);
    let temp2 = fragment.op_convert_s_to_f(f32_id, remainder);
    fragment.op_f_mul(f32_id, offset, temp2)
}

impl ProcTexShift for NoShift {
    fn offset(&self, fragment: &mut FragmentModule, _offset: Id, _v_i32: Id) -> Id {
        fragment.const_f32(0.0)
    }
}

impl ProcTexShift for OddShift {
    fn offset(&self, fragment: &mut FragmentModule, offset: Id, v_i32: Id) -> Id {
        shift(fragment, offset, v_i32, false)
    }
}

impl ProcTexShift for EvenShift {
    fn offset(&self, fragment: &mut FragmentModule, offset: Id, v_i32: Id) -> Id {
        shift(fragment, offset, v_i32, true)
    }
}

pub fn proctex_shift(raw: u32) -> &'static dyn ProcTexShift {
    match raw {
        1 => &OddShift,
        2 => &EvenShift,
        _ => &NoShift,
    }
}

pub trait ProcTexCombiner {
    fn combine(&self, fragment: &mut FragmentModule, u: Id, v: Id, u2v2: Id) -> Id;
}

pub struct CombineU;
pub struct CombineU2;
pub struct CombineV;
pub struct CombineV2;
pub struct CombineAdd;
pub struct CombineAdd2;
pub struct CombineSqrtAdd2;
pub struct CombineMin;
pub struct CombineMax;
pub struct CombineRMax;
pub struct UnknownCombiner;

impl ProcTexCombiner for CombineU {
    fn combine(&self, _fragment: &mut FragmentModule, u: Id, _v: Id, _u2v2: Id) -> Id {
        u
    }
}

impl ProcTexCombiner for CombineU2 {
    fn combine(&self, fragment: &mut FragmentModule, u: Id, _v: Id, _u2v2: Id) -> Id {
        let f32_id = fragment.f32_id;
        fragment.op_f_mul(f32_id, u, u)
    }
}

impl ProcTexCombiner for CombineV {
    fn combine(&self, _fragment: &mut FragmentModule, _u: Id, v: Id, _u2v2: Id) -> Id {
        v
    }
}

impl ProcTexCombiner for CombineV2 {
    fn combine(&self, fragment: &mut FragmentModule, _u: Id, v: Id, _u2v2: Id) -> Id {
        let f32_id = fragment.f32_id;
        fragment.op_f_mul(f32_id, v, v)
    }
}

impl ProcTexCombiner for CombineAdd {
    fn combine(&self, fragment: &mut FragmentModule, u: Id, v: Id, _u2v2: Id) -> Id {
        let f32_id = fragment.f32_id;
        let sum = fragment.op_f_add(f32_id, u, v);
        let half = fragment.const_f32(0.5);
        fragment.op_f_mul(f32_id, sum, half)
    }
}

impl ProcTexCombiner for CombineAdd2 {
    fn combine(&self, fragment: &mut FragmentModule, _u: Id, _v: Id, u2v2: Id) -> Id {
        let f32_id = fragment.f32_id;
        let half = fragment.const_f32(0.5);
        fragment.op_f_mul(f32_id, u2v2, half)
    }
}

impl ProcTexCombiner for CombineSqrtAdd2 {
    fn combine(&self, fragment: &mut FragmentModule, _u: Id, _v: Id, u2v2: Id) -> Id {
        let f32_id = fragment.f32_id;
        let sqrt = fragment.glsl::<glsl::Sqrt>(f32_id, &[u2v2]);
        let one = fragment.const_f32(1.0);
        fragment.glsl::<glsl::FMin>(f32_id, &[sqrt, one])
    }
}

impl ProcTexCombiner for CombineMin {
    fn combine(&self, fragment: &mut FragmentModule, u: Id, v: Id, _u2v2: Id) -> Id {
        let f32_id = fragment.f32_id;
        fragment.glsl::<glsl::FMin>(f32_id, &[u, v])
    }
}

impl ProcTexCombiner for CombineMax {
    fn combine(&self, fragment: &mut FragmentModule, u: Id, v: Id, _u2v2: Id) -> Id {
        let f32_id = fragment.f32_id;
        fragment.glsl::<glsl::FMax>(f32_id, &[u, v])
    }
}

impl ProcTexCombiner for CombineRMax {
    fn combine(&self, fragment: &mut FragmentModule, u: Id, v: Id, u2v2: Id) -> Id {
        let f32_id = fragment.f32_id;
        let sum = fragment.op_f_add(f32_id, u, v);
        let half = fragment.const_f32(0.5);
        let sqrt = fragment.glsl::<glsl::Sqrt>(f32_id, &[u2v2]);
        let r = fragment.glsl::<glsl::Fma>(f32_id, &[sum, half, sqrt]);
        let half = fragment.const_f32(0.5);
        let scaled = fragment.op_f_mul(f32_id, r, half);
        let one = fragment.const_f32(1.0);
        fragment.glsl::<glsl::FMin>(f32_id, &[scaled, one])
    }
}

impl ProcTexCombiner for UnknownCombiner {
    fn combine(&self, fragment: &mut FragmentModule, _u: Id, _v: Id, _u2v2: Id) -> Id {
        fragment.const_f32(0.0)
    }
}

pub fn proctex_combiner(raw: u32) -> &'static dyn ProcTexCombiner {
    match raw {
        0 => &CombineU,
        1 => &CombineU2,
        2 => &CombineV,
        3 => &CombineV2,
        4 => &CombineAdd,
        5 => &CombineAdd2,
        6 => &CombineSqrtAdd2,
        7 => &CombineMin,
        8 => &CombineMax,
        9 => &CombineRMax,
        _ => &UnknownCombiner,
    }
}

pub struct LutFetch {
    pub lut_coord: Id,
    pub lut_offset: Id,
    pub proctex_lut_offset: Id,
}

pub trait ProcTexFilter {
    fn final_color(&self, fragment: &mut FragmentModule, lut_coord: Id, lod: Id) -> Id;

    fn fetch(&self, fragment: &mut FragmentModule, lut: &LutFetch) -> Id;
}

pub struct Nearest;
pub struct Linear;
pub struct NearestMipmapNearest;
pub struct LinearMipmapNearest;
pub struct NearestMipmapLinear;
pub struct LinearMipmapLinear;
pub struct UnknownFilter;

fn sample_base_level(fragment: &mut FragmentModule, lut_coord: Id) -> Id {
    let level = fragment.const_s32(0);
    fragment.sample_proctex_color(lut_coord, level)
}

fn sample_nearest_level(fragment: &mut FragmentModule, lut_coord: Id, lod: Id) -> Id {
    let (f32_id, i32_id) = (fragment.f32_id, fragment.i32_id);
    let rounded = fragment.glsl::<glsl::Round>(f32_id, &[lod]);
    let level = fragment.op_convert_f_to_s(i32_id, rounded);
    fragment.sample_proctex_color(lut_coord, level)
}

fn sample_linear_levels(fragment: &mut FragmentModule, lut_coord: Id, lod: Id) -> Id {
    let (f32_id, i32_id) = (fragment.f32_id, fragment.i32_id);
    let lod_i = fragment.op_convert_f_to_s(i32_id, lod);
    let lod_f = fragment.glsl::<glsl::Fract>(f32_id, &[lod]);
    let color1 = fragment.sample_proctex_color(lut_coord, lod_i);
    let one = fragment.const_s32(1);
    let next_level = fragment.op_i_add(i32_id, lod_i, one);
    let color2 = fragment.sample_proctex_color(lut_coord, next_level);
    fragment.glsl::<glsl::FMix>(f32_id, &[color1, color2, lod_f])
}

fn fetch_linear(fragment: &mut FragmentModule, lut: &LutFetch) -> Id {
    let (f32_id, i32_id, vec4) = (fragment.f32_id, fragment.i32_id, fragment.vec_ids.get(4));
    let converted = fragment.op_convert_f_to_s(i32_id, lut.lut_coord);
    let lut_index_i = fragment.op_i_add(i32_id, converted, lut.lut_offset);
    let lut_index_f = fragment.glsl::<glsl::Fract>(f32_id, &[lut.lut_coord]);
    let diff_member = fragment.settings.uniform.proctex_diff_lut_offset;
    let member = fragment.const_s32(diff_member);
    let proctex_diff_lut_offset = fragment.get_shader_data_member(i32_id, &[member]);
    let p1 = fragment.op_i_add(i32_id, lut_index_i, lut.proctex_lut_offset);
    let p2 = fragment.op_i_add(i32_id, lut_index_i, proctex_diff_lut_offset);
    let image = fragment.texture_buffer_lut_rgba;
    let texel1 = fragment.op_image_fetch(vec4, image, p1, None, &[]);
    let texel2 = fragment.op_image_fetch(vec4, image, p2, None, &[]);
    let scaled = fragment.op_vector_times_scalar(vec4, texel2, lut_index_f);
    fragment.op_f_add(vec4, texel1, scaled)
}

fn fetch_nearest(fragment: &mut FragmentModule, lut: &LutFetch) -> Id {
    let (f32_id, i32_id, vec4) = (fragment.f32_id, fragment.i32_id, fragment.vec_ids.get(4));
    let offset = fragment.op_convert_s_to_f(f32_id, lut.lut_offset);
    let lut_coord = fragment.op_f_add(f32_id, lut.lut_coord, offset);
    let rounded = fragment.glsl::<glsl::Round>(f32_id, &[lut_coord]);
    let lut_coord_rounded = fragment.op_convert_f_to_s(i32_id, rounded);
    let p = fragment.op_i_add(i32_id, lut_coord_rounded, lut.proctex_lut_offset);
    let image = fragment.texture_buffer_lut_rgba;
    fragment.op_image_fetch(vec4, image, p, None, &[])
}

impl ProcTexFilter for Nearest {
    fn final_color(&self, fragment: &mut FragmentModule, lut_coord: Id, _lod: Id) -> Id {
        sample_base_level(fragment, lut_coord)
    }

    fn fetch(&self, fragment: &mut FragmentModule, lut: &LutFetch) -> Id {
        fetch_nearest(fragment, lut)
    }
}

impl ProcTexFilter for Linear {
    fn final_color(&self, fragment: &mut FragmentModule, lut_coord: Id, _lod: Id) -> Id {
        sample_base_level(fragment, lut_coord)
    }

    fn fetch(&self, fragment: &mut FragmentModule, lut: &LutFetch) -> Id {
        fetch_linear(fragment, lut)
    }
}

impl ProcTexFilter for NearestMipmapNearest {
    fn final_color(&self, fragment: &mut FragmentModule, lut_coord: Id, lod: Id) -> Id {
        sample_nearest_level(fragment, lut_coord, lod)
    }

    fn fetch(&self, fragment: &mut FragmentModule, lut: &LutFetch) -> Id {
        fetch_nearest(fragment, lut)
    }
}

impl ProcTexFilter for LinearMipmapNearest {
    fn final_color(&self, fragment: &mut FragmentModule, lut_coord: Id, lod: Id) -> Id {
        sample_nearest_level(fragment, lut_coord, lod)
    }

    fn fetch(&self, fragment: &mut FragmentModule, lut: &LutFetch) -> Id {
        fetch_linear(fragment, lut)
    }
}

impl ProcTexFilter for NearestMipmapLinear {
    fn final_color(&self, fragment: &mut FragmentModule, lut_coord: Id, lod: Id) -> Id {
        sample_linear_levels(fragment, lut_coord, lod)
    }

    fn fetch(&self, fragment: &mut FragmentModule, lut: &LutFetch) -> Id {
        fetch_nearest(fragment, lut)
    }
}

impl ProcTexFilter for LinearMipmapLinear {
    fn final_color(&self, fragment: &mut FragmentModule, lut_coord: Id, lod: Id) -> Id {
        sample_linear_levels(fragment, lut_coord, lod)
    }

    fn fetch(&self, fragment: &mut FragmentModule, lut: &LutFetch) -> Id {
        fetch_linear(fragment, lut)
    }
}

impl ProcTexFilter for UnknownFilter {
    fn final_color(&self, _fragment: &mut FragmentModule, _lut_coord: Id, _lod: Id) -> Id {
        Id::default()
    }

    fn fetch(&self, _fragment: &mut FragmentModule, _lut: &LutFetch) -> Id {
        Id::default()
    }
}

pub fn proctex_filter(raw: u32) -> &'static dyn ProcTexFilter {
    match raw {
        0 => &Nearest,
        1 => &Linear,
        2 => &NearestMipmapNearest,
        3 => &LinearMipmapNearest,
        4 => &NearestMipmapLinear,
        5 => &LinearMipmapLinear,
        _ => &UnknownFilter,
    }
}
