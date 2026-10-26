use emu_spirv::{Id, glsl};

use crate::fragment::FragmentModule;

pub trait TevSource {
    fn is_previous(&self) -> bool {
        false
    }

    fn get(&self, fragment: &mut FragmentModule, index: i32) -> Id;
}

pub struct PrimaryColor;
pub struct PrimaryFragmentColor;
pub struct SecondaryFragmentColor;
pub struct TextureUnit(pub usize);
pub struct PreviousBuffer;
pub struct Constant;
pub struct Previous;
pub struct UnknownSource;

impl TevSource for PrimaryColor {
    fn get(&self, fragment: &mut FragmentModule, _index: i32) -> Id {
        fragment.rounded_primary_color
    }
}

impl TevSource for PrimaryFragmentColor {
    fn get(&self, fragment: &mut FragmentModule, _index: i32) -> Id {
        fragment.primary_fragment_color
    }
}

impl TevSource for SecondaryFragmentColor {
    fn get(&self, fragment: &mut FragmentModule, _index: i32) -> Id {
        fragment.secondary_fragment_color
    }
}

impl TevSource for TextureUnit {
    fn get(&self, fragment: &mut FragmentModule, _index: i32) -> Id {
        let vec4 = fragment.vec_ids.get(4);
        let function = fragment.sample_tex_unit_func[self.0];
        fragment.op_function_call(vec4, function, &[])
    }
}

impl TevSource for PreviousBuffer {
    fn get(&self, fragment: &mut FragmentModule, _index: i32) -> Id {
        fragment.combiner_buffer
    }
}

impl TevSource for Constant {
    fn get(&self, fragment: &mut FragmentModule, index: i32) -> Id {
        let vec4 = fragment.vec_ids.get(4);
        let const_color = fragment.settings.uniform.const_color;
        let member = fragment.const_s32(const_color);
        let stage = fragment.const_s32(index);
        fragment.get_shader_data_member(vec4, &[member, stage])
    }
}

impl TevSource for Previous {
    fn is_previous(&self) -> bool {
        true
    }

    fn get(&self, fragment: &mut FragmentModule, _index: i32) -> Id {
        fragment.combiner_output
    }
}

impl TevSource for UnknownSource {
    fn get(&self, fragment: &mut FragmentModule, _index: i32) -> Id {
        fragment.const_f32s(&[0.0, 0.0, 0.0, 0.0])
    }
}

pub fn tev_source(raw: u32) -> &'static dyn TevSource {
    match raw {
        0x0 => &PrimaryColor,
        0x1 => &PrimaryFragmentColor,
        0x2 => &SecondaryFragmentColor,
        0x3 => &TextureUnit(0),
        0x4 => &TextureUnit(1),
        0x5 => &TextureUnit(2),
        0x6 => &TextureUnit(3),
        0xd => &PreviousBuffer,
        0xe => &Constant,
        0xf => &Previous,
        _ => &UnknownSource,
    }
}

pub trait ColorModifier {
    fn is_source_color(&self) -> bool {
        false
    }

    fn apply(&self, fragment: &mut FragmentModule, source_color: Id, one_vec: Id) -> Id;
}

pub struct Channels(pub [i32; 3]);
pub struct OneMinusChannels(pub [i32; 3]);
pub struct UnknownColorModifier;

impl ColorModifier for Channels {
    fn is_source_color(&self) -> bool {
        self.0 == [0, 1, 2]
    }

    fn apply(&self, fragment: &mut FragmentModule, source_color: Id, _one_vec: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        fragment.vector_shuffle(vec3, source_color, source_color, &self.0)
    }
}

impl ColorModifier for OneMinusChannels {
    fn apply(&self, fragment: &mut FragmentModule, source_color: Id, one_vec: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        let shuffled = fragment.vector_shuffle(vec3, source_color, source_color, &self.0);
        fragment.op_f_sub(vec3, one_vec, shuffled)
    }
}

impl ColorModifier for UnknownColorModifier {
    fn apply(&self, _fragment: &mut FragmentModule, _source_color: Id, one_vec: Id) -> Id {
        one_vec
    }
}

pub fn color_modifier(raw: u32) -> &'static dyn ColorModifier {
    match raw {
        0x0 => &Channels([0, 1, 2]),
        0x1 => &OneMinusChannels([0, 1, 2]),
        0x2 => &Channels([3, 3, 3]),
        0x3 => &OneMinusChannels([3, 3, 3]),
        0x4 => &Channels([0, 0, 0]),
        0x5 => &OneMinusChannels([0, 0, 0]),
        0x8 => &Channels([1, 1, 1]),
        0x9 => &OneMinusChannels([1, 1, 1]),
        0xc => &Channels([2, 2, 2]),
        0xd => &OneMinusChannels([2, 2, 2]),
        _ => &UnknownColorModifier,
    }
}

pub trait AlphaModifier {
    fn is_source_alpha(&self) -> bool {
        false
    }

    fn apply(&self, fragment: &mut FragmentModule, source_alpha: Id, one_f32: Id) -> Id;
}

pub struct Component(pub i32);
pub struct OneMinusComponent(pub i32);
pub struct UnknownAlphaModifier;

impl AlphaModifier for Component {
    fn is_source_alpha(&self) -> bool {
        self.0 == 3
    }

    fn apply(&self, fragment: &mut FragmentModule, source_alpha: Id, _one_f32: Id) -> Id {
        let f32_id = fragment.f32_id;
        fragment.composite_extract(f32_id, source_alpha, self.0)
    }
}

impl AlphaModifier for OneMinusComponent {
    fn apply(&self, fragment: &mut FragmentModule, source_alpha: Id, one_f32: Id) -> Id {
        let f32_id = fragment.f32_id;
        let component = fragment.composite_extract(f32_id, source_alpha, self.0);
        fragment.op_f_sub(f32_id, one_f32, component)
    }
}

impl AlphaModifier for UnknownAlphaModifier {
    fn apply(&self, _fragment: &mut FragmentModule, _source_alpha: Id, one_f32: Id) -> Id {
        one_f32
    }
}

pub fn alpha_modifier(raw: u32) -> &'static dyn AlphaModifier {
    match raw {
        0x0 => &Component(3),
        0x1 => &OneMinusComponent(3),
        0x2 => &Component(0),
        0x3 => &OneMinusComponent(0),
        0x4 => &Component(1),
        0x5 => &OneMinusComponent(1),
        0x6 => &Component(2),
        0x7 => &OneMinusComponent(2),
        _ => &UnknownAlphaModifier,
    }
}

pub trait TevOperation {
    fn is_replace(&self) -> bool {
        false
    }

    fn is_dot3_rgba(&self) -> bool {
        false
    }

    fn color(&self, fragment: &mut FragmentModule, half_vec: Id, one_vec: Id, zero_vec: Id) -> Id;

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        fragment.const_f32(0.0)
    }
}

pub struct Replace;
pub struct Modulate;
pub struct Add;
pub struct AddSigned;
pub struct Lerp;
pub struct Subtract;
pub struct Dot3Rgb;
pub struct Dot3Rgba;
pub struct MultiplyThenAdd;
pub struct AddThenMultiply;
pub struct UnknownOperation;

impl TevOperation for Replace {
    fn is_replace(&self) -> bool {
        true
    }

    fn color(&self, fragment: &mut FragmentModule, _half: Id, _one: Id, _zero: Id) -> Id {
        fragment.color_results_1
    }

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        fragment.alpha_results_1
    }
}

impl TevOperation for Modulate {
    fn color(&self, fragment: &mut FragmentModule, _half: Id, _one: Id, _zero: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        let (a, b) = (fragment.color_results_1, fragment.color_results_2);
        fragment.op_f_mul(vec3, a, b)
    }

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        let f32_id = fragment.f32_id;
        let (a, b) = (fragment.alpha_results_1, fragment.alpha_results_2);
        fragment.op_f_mul(f32_id, a, b)
    }
}

impl TevOperation for Add {
    fn color(&self, fragment: &mut FragmentModule, _half: Id, _one: Id, _zero: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        let (a, b) = (fragment.color_results_1, fragment.color_results_2);
        fragment.op_f_add(vec3, a, b)
    }

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        let f32_id = fragment.f32_id;
        let (a, b) = (fragment.alpha_results_1, fragment.alpha_results_2);
        fragment.op_f_add(f32_id, a, b)
    }
}

impl TevOperation for AddSigned {
    fn color(&self, fragment: &mut FragmentModule, half_vec: Id, _one: Id, _zero: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        let (a, b) = (fragment.color_results_1, fragment.color_results_2);
        let sum = fragment.op_f_add(vec3, a, b);
        fragment.op_f_sub(vec3, sum, half_vec)
    }

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        let f32_id = fragment.f32_id;
        let (a, b) = (fragment.alpha_results_1, fragment.alpha_results_2);
        let sum = fragment.op_f_add(f32_id, a, b);
        let half = fragment.const_f32(0.5);
        fragment.op_f_sub(f32_id, sum, half)
    }
}

impl TevOperation for Lerp {
    fn color(&self, fragment: &mut FragmentModule, _half: Id, _one: Id, _zero: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        let (a, b, c) = (
            fragment.color_results_1,
            fragment.color_results_2,
            fragment.color_results_3,
        );
        fragment.glsl::<glsl::FMix>(vec3, &[b, a, c])
    }

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        let f32_id = fragment.f32_id;
        let (a, b, c) = (
            fragment.alpha_results_1,
            fragment.alpha_results_2,
            fragment.alpha_results_3,
        );
        fragment.glsl::<glsl::FMix>(f32_id, &[b, a, c])
    }
}

impl TevOperation for Subtract {
    fn color(&self, fragment: &mut FragmentModule, _half: Id, _one: Id, _zero: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        let (a, b) = (fragment.color_results_1, fragment.color_results_2);
        fragment.op_f_sub(vec3, a, b)
    }

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        let f32_id = fragment.f32_id;
        let (a, b) = (fragment.alpha_results_1, fragment.alpha_results_2);
        fragment.op_f_sub(f32_id, a, b)
    }
}

fn dot3(fragment: &mut FragmentModule, half_vec: Id) -> Id {
    let (vec3, f32_id) = (fragment.vec_ids.get(3), fragment.f32_id);
    let (a, b) = (fragment.color_results_1, fragment.color_results_2);
    let left = fragment.op_f_sub(vec3, a, half_vec);
    let right = fragment.op_f_sub(vec3, b, half_vec);
    let dot = fragment.op_dot(f32_id, left, right);
    let four = fragment.const_f32(4.0);
    let scaled = fragment.op_f_mul(f32_id, dot, four);
    fragment.op_composite_construct(vec3, &[scaled, scaled, scaled])
}

impl TevOperation for Dot3Rgb {
    fn color(&self, fragment: &mut FragmentModule, half_vec: Id, _one: Id, _zero: Id) -> Id {
        dot3(fragment, half_vec)
    }
}

impl TevOperation for Dot3Rgba {
    fn is_dot3_rgba(&self) -> bool {
        true
    }

    fn color(&self, fragment: &mut FragmentModule, half_vec: Id, _one: Id, _zero: Id) -> Id {
        dot3(fragment, half_vec)
    }
}

impl TevOperation for MultiplyThenAdd {
    fn color(&self, fragment: &mut FragmentModule, _half: Id, _one: Id, _zero: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        let (a, b, c) = (
            fragment.color_results_1,
            fragment.color_results_2,
            fragment.color_results_3,
        );
        fragment.glsl::<glsl::Fma>(vec3, &[a, b, c])
    }

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        let f32_id = fragment.f32_id;
        let (a, b, c) = (
            fragment.alpha_results_1,
            fragment.alpha_results_2,
            fragment.alpha_results_3,
        );
        fragment.glsl::<glsl::Fma>(f32_id, &[a, b, c])
    }
}

impl TevOperation for AddThenMultiply {
    fn color(&self, fragment: &mut FragmentModule, _half: Id, one_vec: Id, _zero: Id) -> Id {
        let vec3 = fragment.vec_ids.get(3);
        let (a, b, c) = (
            fragment.color_results_1,
            fragment.color_results_2,
            fragment.color_results_3,
        );
        let sum = fragment.op_f_add(vec3, a, b);
        let color = fragment.glsl::<glsl::FMin>(vec3, &[sum, one_vec]);
        fragment.op_f_mul(vec3, color, c)
    }

    fn alpha(&self, fragment: &mut FragmentModule) -> Id {
        let f32_id = fragment.f32_id;
        let (a, b, c) = (
            fragment.alpha_results_1,
            fragment.alpha_results_2,
            fragment.alpha_results_3,
        );
        let sum = fragment.op_f_add(f32_id, a, b);
        let one = fragment.const_f32(1.0);
        let color = fragment.glsl::<glsl::FMin>(f32_id, &[sum, one]);
        fragment.op_f_mul(f32_id, color, c)
    }
}

impl TevOperation for UnknownOperation {
    fn color(&self, _fragment: &mut FragmentModule, _half: Id, _one: Id, zero_vec: Id) -> Id {
        zero_vec
    }
}

pub fn tev_operation(raw: u32) -> &'static dyn TevOperation {
    match raw {
        0 => &Replace,
        1 => &Modulate,
        2 => &Add,
        3 => &AddSigned,
        4 => &Lerp,
        5 => &Subtract,
        6 => &Dot3Rgb,
        7 => &Dot3Rgba,
        8 => &MultiplyThenAdd,
        9 => &AddThenMultiply,
        _ => &UnknownOperation,
    }
}
