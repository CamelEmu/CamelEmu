use emu_spirv::{Id, glsl};

use super::FragmentModule;

impl FragmentModule {
    fn compare_shadow(&mut self, pixel: Id, z: Id) -> Id {
        let (f32_id, u32_id, bool_id) = (self.f32_id, self.u32_id, self.bool_id);
        let shadow = self.settings.shadow.clone();
        let byte_scale = self.settings.color.byte_scale;

        let shift = self.const_s32(shadow.depth_shift);
        let pixel_d24 = self.op_shift_right_logical(u32_id, pixel, shift);
        let mask = self.const_u32(shadow.stencil_mask);
        let masked = self.op_bitwise_and(u32_id, pixel, mask);
        let pixel_s8 = self.op_convert_u_to_f(f32_id, masked);
        let inverse_scale = self.const_f32(1.0 / byte_scale);
        let s8_f32 = self.op_f_mul(f32_id, pixel_s8, inverse_scale);
        let d24_leq_z = self.op_u_less_than_equal(bool_id, pixel_d24, z);
        let zero = self.const_f32(0.0);
        self.op_select(f32_id, d24_leq_z, zero, s8_f32)
    }

    fn sample_shadow_2d(&mut self, shadow_texture_px: Id, uv: Id, px_size: Id, z: Id) -> Id {
        let (f32_id, bool_id) = (self.f32_id, self.bool_id);
        let (bvec2, bvec4, uvec4, u32_id) = (
            self.bvec_ids.get(2),
            self.bvec_ids.get(4),
            self.uvec_ids.get(4),
            self.u32_id,
        );

        let true_label = self.op_label();
        let false_label = self.op_label();
        let end_label = self.op_label();
        let zeros = self.const_s32s(&[0, 0]);
        let uv_le_zero = self.op_s_less_than(bvec2, uv, zeros);
        let uv_geq_size = self.op_s_greater_than_equal(bvec2, uv, px_size);
        let conditions = self.op_composite_construct(bvec4, &[uv_le_zero, uv_geq_size]);
        let cond = self.op_any(bool_id, conditions);
        self.op_selection_merge(end_label, spirv::SelectionControl::NONE);
        self.op_branch_conditional(cond, true_label, false_label, 0, 0);
        self.add_label(true_label);
        self.op_branch(end_label);
        self.add_label(false_label);
        let px_texel = self.op_image_read(uvec4, shadow_texture_px, uv, None, &[]);
        let px_texel_x = self.composite_extract(u32_id, px_texel, 0);
        let result = self.compare_shadow(px_texel_x, z);
        self.op_branch(end_label);
        self.add_label(end_label);
        let one = self.const_f32(1.0);
        self.op_phi(f32_id, &[one, true_label, result, false_label])
    }

    pub(crate) fn sample_shadow(&mut self) -> Id {
        let (f32_id, i32_id, u32_id) = (self.f32_id, self.i32_id, self.u32_id);
        let (vec2, vec4, ivec2) = (
            self.vec_ids.get(2),
            self.vec_ids.get(4),
            self.ivec_ids.get(2),
        );
        let shadow = self.settings.shadow.clone();
        let shadow_texture_bias_member = self.settings.uniform.shadow_texture_bias;

        let texcoord0_id = self.texcoord_id[0];
        let texcoord0 = self.op_load(vec2, texcoord0_id, None);
        let texcoord0_w_id = self.texcoord0_w_id;
        let texcoord0_w = self.op_load(f32_id, texcoord0_w_id, None);
        let abs_w = self.glsl::<glsl::FAbs>(f32_id, &[texcoord0_w]);
        let one = self.const_f32(1.0);
        let min_w = self.glsl::<glsl::FMin>(f32_id, &[abs_w, one]);
        let w_scale = self.const_f32(shadow.w_scale);
        let abs_min_w = self.op_f_mul(f32_id, min_w, w_scale);
        let member = self.const_s32(shadow_texture_bias_member);
        let shadow_texture_bias = self.get_shader_data_member(i32_id, &[member]);
        let zero = self.const_s32(0);
        let abs_min_w_i32 = self.op_convert_f_to_s(i32_id, abs_min_w);
        let biased = self.op_i_sub(i32_id, abs_min_w_i32, shadow_texture_bias);
        let z_i32 = self.glsl::<glsl::SMax>(i32_id, &[zero, biased]);
        let z = self.op_bitcast(u32_id, z_i32);
        let (image_r32_id, shadow_texture_px_id) = (self.image_r32_id, self.shadow_texture_px_id);
        let shadow_texture_px = self.op_load(image_r32_id, shadow_texture_px_id, None);
        let px_size = self.op_image_query_size(ivec2, shadow_texture_px);
        let size_f32 = self.op_convert_s_to_f(vec2, px_size);
        let offsets = self.const_f32s(&[shadow.coord_offset, shadow.coord_offset]);
        let coord = self.glsl::<glsl::Fma>(vec2, &[size_f32, texcoord0, offsets]);
        let coord_floor = self.glsl::<glsl::Floor>(vec2, &[coord]);
        let f = self.op_f_sub(vec2, coord, coord_floor);
        let i = self.op_convert_f_to_s(ivec2, coord_floor);

        let sample_00 = self.sample_shadow_2d(shadow_texture_px, i, px_size, z);
        let offset_10 = self.const_s32s(&[1, 0]);
        let uv_10 = self.op_i_add(ivec2, i, offset_10);
        let sample_10 = self.sample_shadow_2d(shadow_texture_px, uv_10, px_size, z);
        let s_xy = self.op_composite_construct(vec2, &[sample_00, sample_10]);
        let offset_01 = self.const_s32s(&[0, 1]);
        let uv_01 = self.op_i_add(ivec2, i, offset_01);
        let sample_01 = self.sample_shadow_2d(shadow_texture_px, uv_01, px_size, z);
        let offset_11 = self.const_s32s(&[1, 1]);
        let uv_11 = self.op_i_add(ivec2, i, offset_11);
        let sample_11 = self.sample_shadow_2d(shadow_texture_px, uv_11, px_size, z);
        let s_zw = self.op_composite_construct(vec2, &[sample_01, sample_11]);
        let f_yy = self.vector_shuffle(vec2, f, f, &[1, 1]);
        let t = self.glsl::<glsl::FMix>(vec2, &[s_xy, s_zw, f_yy]);
        let t_x = self.composite_extract(f32_id, t, 0);
        let t_y = self.composite_extract(f32_id, t, 1);
        let a_x = self.composite_extract(f32_id, f, 0);
        let val = self.glsl::<glsl::FMix>(f32_id, &[t_x, t_y, a_x]);
        self.op_composite_construct(vec4, &[val, val, val, val])
    }
}
