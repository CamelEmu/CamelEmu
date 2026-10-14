use std::mem::offset_of;

use emu_pica_types::{AttributeBuffer, F24, OutputVertex};

use crate::regs::RasterizerRegs;
use crate::regs::rasterizer::Semantic;
use crate::vector::component;

pub fn output_vertex(regs: &RasterizerRegs, output: &AttributeBuffer) -> OutputVertex {
    // Attributes can be used without being set in GPUREG_SH_OUTMAP_Oi
    // Hardware tests have shown that they are initialized to 1 in this case.
    let mut vertex_slots_overflow = [F24::one(); 32];

    let num_attributes = regs.vs_output_total_raw & 7;
    for (output_register_map, attribute) in regs
        .vs_output_attributes
        .iter()
        .zip(output.iter())
        .take(num_attributes as usize)
    {
        vertex_slots_overflow[output_register_map.map_x().0 as usize] = *component(attribute, 0);
        vertex_slots_overflow[output_register_map.map_y().0 as usize] = *component(attribute, 1);
        vertex_slots_overflow[output_register_map.map_z().0 as usize] = *component(attribute, 2);
        vertex_slots_overflow[output_register_map.map_w().0 as usize] = *component(attribute, 3);
    }

    // Copy to result
    let mut vertex: OutputVertex =
        unsafe { std::ptr::read(vertex_slots_overflow.as_ptr().cast::<OutputVertex>()) };

    // The hardware takes the absolute and saturates vertex colors, *before* doing interpolation
    for color in [
        &mut vertex.color.x,
        &mut vertex.color.y,
        &mut vertex.color.z,
        &mut vertex.color.w,
    ] {
        let c = color.to_float32().abs();
        *color = F24::from_float32(if c < 1.0 { c } else { 1.0 });
    }
    vertex
}

macro_rules! assert_pos {
    ($var:ident, $pos:expr) => {
        const _: () = assert!(
            offset_of!(OutputVertex, $var) == $pos.0 as usize * size_of::<F24>(),
            "Semantic at wrong offset."
        );
    };
}

assert_pos!(pos, Semantic::POSITION_X);
assert_pos!(quat, Semantic::QUATERNION_X);
assert_pos!(color, Semantic::COLOR_R);
assert_pos!(tc0, Semantic::TEXCOORD0_U);
assert_pos!(tc1, Semantic::TEXCOORD1_U);
assert_pos!(tc0_w, Semantic::TEXCOORD0_W);
assert_pos!(view, Semantic::VIEW_X);
assert_pos!(tc2, Semantic::TEXCOORD2_U);
