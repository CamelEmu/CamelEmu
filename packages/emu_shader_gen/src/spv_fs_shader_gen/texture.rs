use emu_pica_types::regs::texturing::TextureType;
use rspirv::spirv::{FunctionControl, ImageOperands, SelectionControl};

use super::FragmentModule;
use crate::spv_module::Id;

impl FragmentModule<'_> {
    /// Defines the basic texture sampling functions for a unit
    pub(super) fn define_tex_sampler(&mut self, texture_unit: u32) {
        let vec2 = self.vec_ids.get(2);
        let vec4 = self.vec_ids.get(4);
        let f32_id = self.f32_id;

        let func_type = self.m.type_function(vec4);
        self.sample_tex_unit_func[texture_unit as usize] =
            self.m.op_function(vec4, FunctionControl::NONE, func_type);
        let label = self.m.op_label();
        self.m.add_label(label);

        let zero_vec = self.const_f32_vec(&[0.0, 0.0, 0.0, 0.0]);

        if texture_unit == 0 && self.config.texture.texture0_type() == TextureType::DISABLED {
            self.m.op_return_value(zero_vec);
            self.m.op_function_end();
            return;
        }

        if texture_unit == 3 {
            if self.config.proctex.enable() != 0 {
                let proctex = self.proctex_sampler();
                self.m.op_return_value(proctex);
            } else {
                self.m.op_return_value(zero_vec);
            }
            self.m.op_function_end();
            return;
        }

        let border_label = self.m.op_label();
        let not_border_label = self.m.op_label();

        let texcoord_num = if texture_unit == 2 && self.config.texture.texture2_use_coord1() != 0 {
            1
        } else {
            texture_unit
        };
        let texcoord = self
            .m
            .op_load(vec2, self.texcoord_id[texcoord_num as usize]);

        let texture_border_color = self.config.texture.texture_border_color[texture_unit as usize];
        if texture_border_color.enable_s() != 0 || texture_border_color.enable_t() != 0 {
            let bool_id = self.bool_id;
            let texcoord_s = self.m.op_composite_extract(f32_id, texcoord, 0);
            let texcoord_t = self.m.op_composite_extract(f32_id, texcoord, 1);

            let zero = self.const_f32(0.0);
            let one = self.const_f32(1.0);
            let s_lt_zero = self.m.op_f_ord_less_than(bool_id, texcoord_s, zero);
            let s_gt_one = self.m.op_f_ord_greater_than(bool_id, texcoord_s, one);
            let t_lt_zero = self.m.op_f_ord_less_than(bool_id, texcoord_t, zero);
            let t_gt_one = self.m.op_f_ord_greater_than(bool_id, texcoord_t, one);

            let mut cond = 0;
            if texture_border_color.enable_s() != 0 && texture_border_color.enable_t() != 0 {
                let conds = self.m.op_composite_construct(
                    self.bvec_ids.get(4),
                    &[s_lt_zero, s_gt_one, t_lt_zero, t_gt_one],
                );
                cond = self.m.op_any(bool_id, conds);
            } else if texture_border_color.enable_s() != 0 {
                let conds = self
                    .m
                    .op_composite_construct(self.bvec_ids.get(2), &[s_lt_zero, s_gt_one]);
                cond = self.m.op_any(bool_id, conds);
            } else if texture_border_color.enable_t() != 0 {
                let conds = self
                    .m
                    .op_composite_construct(self.bvec_ids.get(2), &[t_lt_zero, t_gt_one]);
                cond = self.m.op_any(bool_id, conds);
            }

            self.m
                .op_selection_merge(not_border_label, SelectionControl::NONE);
            self.m
                .op_branch_conditional(cond, border_label, not_border_label);

            self.m.add_label(border_label);
            let member = self.const_s32(28);
            let unit = self.const_u32(texture_unit);
            let border_color = self.get_shader_data_member(vec4, &[member, unit]);
            self.m.op_return_value(border_color);

            self.m.add_label(not_border_label);
        }

        let ret_val = match texture_unit {
            0 => {
                // Only unit 0 respects the texturing type
                match self.config.texture.texture0_type() {
                    TextureType::TEXTURE_2D => {
                        self.sample_lod(self.tex0_id, texcoord, texture_unit)
                    }
                    TextureType::PROJECTION_2D => self.sample_3d(self.tex0_id, texcoord, true),
                    TextureType::TEXTURE_CUBE => self.sample_3d(self.tex0_id, texcoord, false),
                    TextureType::SHADOW_2D => {
                        // case Pica::TexturingRegs::TextureConfig::ShadowCube:
                        // return "shadowTextureCube(texcoord0, texcoord0_w)";
                        self.sample_shadow()
                    }
                    _ => {
                        log::error!(
                            target: "Render",
                            "Unhandled texture type {:x}",
                            self.config.texture.texture0_type().0
                        );
                        log::error!(target: "Debug", "Unimplemented code!");
                        zero_vec
                    }
                }
            }
            1 => self.sample_lod(self.tex1_id, texcoord, texture_unit),
            2 => self.sample_lod(self.tex2_id, texcoord, texture_unit),
            _ => unreachable!(),
        };

        self.m.op_return_value(ret_val);
        self.m.op_function_end();
    }

    /// PICA's LOD formula for 2D textures.
    /// This LOD formula is the same as the LOD lower limit defined in OpenGL.
    /// f(x, y) >= max{m_u, m_v, m_w}
    /// (See OpenGL 4.6 spec, 8.14.1 - Scale Factor and Level-of-Detail)
    fn sample_lod(&mut self, tex_id: Id, texcoord: Id, texture_unit: u32) -> Id {
        let vec2 = self.vec_ids.get(2);
        let f32_id = self.f32_id;
        let sampled_image_type = self.m.type_sampled_image(self.image2d_id);
        let sampled_image = self.m.op_load(sampled_image_type, tex_id);
        let tex_image = self.m.op_image(self.image2d_id, sampled_image);
        let lod_zero = self.const_s32(0);
        let tex_size = self
            .m
            .op_image_query_size_lod(self.ivec_ids.get(2), tex_image, lod_zero);
        let tex_size_f = self.m.op_convert_s_to_f(vec2, tex_size);
        let coord = self.m.op_f_mul(vec2, texcoord, tex_size_f);
        let dfdx_coord = self.m.op_dpdx(vec2, coord);
        let abs_dfdx_coord = self.m.op_f_abs(vec2, dfdx_coord);
        let dfdy_coord = self.m.op_dpdy(vec2, coord);
        let abs_dfdy_coord = self.m.op_f_abs(vec2, dfdy_coord);
        let d = self.m.op_f_max(vec2, abs_dfdx_coord, abs_dfdy_coord);
        let d_x = self.m.op_composite_extract(f32_id, d, 0);
        let d_y = self.m.op_composite_extract(f32_id, d, 1);
        let dx_dy_max = self.m.op_f_max(f32_id, d_x, d_y);
        let lod = self.m.op_log2(f32_id, dx_dy_max);
        let member = self.const_s32(27);
        let unit = self.const_u32(texture_unit);
        let lod_bias = self.get_shader_data_member(f32_id, &[member, unit]);
        let biased_lod = self.m.op_f_add(f32_id, lod, lod_bias);
        self.m.op_image_sample_explicit_lod(
            self.vec_ids.get(4),
            sampled_image,
            texcoord,
            ImageOperands::LOD,
            biased_lod,
        )
    }

    fn sample_3d(&mut self, tex_id: Id, texcoord: Id, projection: bool) -> Id {
        let f32_id = self.f32_id;
        let image_type = if !projection {
            self.image_cube_id
        } else {
            self.image2d_id
        };
        let sampled_image_type = self.m.type_sampled_image(image_type);
        let sampled_image = self.m.op_load(sampled_image_type, tex_id);
        let texcoord0_w = self.m.op_load(f32_id, self.texcoord0_w_id);
        let texcoord_s = self.m.op_composite_extract(f32_id, texcoord, 0);
        let texcoord_t = self.m.op_composite_extract(f32_id, texcoord, 1);
        let coord = self
            .m
            .op_composite_construct(self.vec_ids.get(3), &[texcoord_s, texcoord_t, texcoord0_w]);
        if projection {
            self.m
                .op_image_sample_proj_implicit_lod(self.vec_ids.get(4), sampled_image, coord)
        } else {
            self.m
                .op_image_sample_implicit_lod(self.vec_ids.get(4), sampled_image, coord)
        }
    }

    pub(super) fn compare_shadow(&mut self, pixel: Id, z: Id) -> Id {
        let u32_id = self.u32_id;
        let f32_id = self.f32_id;
        let shift = self.const_s32(8);
        let pixel_d24 = self.m.op_shift_right_logical(u32_id, pixel, shift);
        let mask = self.const_u32(255);
        let pixel_s8_u32 = self.m.op_bitwise_and(u32_id, pixel, mask);
        let pixel_s8 = self.m.op_convert_u_to_f(f32_id, pixel_s8_u32);
        let scale = self.const_f32(1.0 / 255.0);
        let s8_f32 = self.m.op_f_mul(f32_id, pixel_s8, scale);
        let d24_leq_z = self.m.op_u_less_than_equal(self.bool_id, pixel_d24, z);
        let zero = self.const_f32(0.0);
        self.m.op_select(f32_id, d24_leq_z, zero, s8_f32)
    }

    /// Samples the current fragment texel from shadow plane
    fn sample_shadow(&mut self) -> Id {
        let vec2 = self.vec_ids.get(2);
        let ivec2 = self.ivec_ids.get(2);
        let f32_id = self.f32_id;
        let i32_id = self.i32_id;

        let texcoord0 = self.m.op_load(vec2, self.texcoord_id[0]);
        let texcoord0_w = self.m.op_load(f32_id, self.texcoord0_w_id);
        let abs_w = self.m.op_f_abs(f32_id, texcoord0_w);
        let one = self.const_f32(1.0);
        let min_w = self.m.op_f_min(f32_id, abs_w, one);
        let scale = self.const_f32(16777215.0);
        let abs_min_w = self.m.op_f_mul(f32_id, min_w, scale);
        let member = self.const_s32(17);
        let shadow_texture_bias = self.get_shader_data_member(i32_id, &[member]);
        let abs_min_w_s32 = self.m.op_convert_f_to_s(i32_id, abs_min_w);
        let biased = self.m.op_i_sub(i32_id, abs_min_w_s32, shadow_texture_bias);
        let zero = self.const_s32(0);
        let z_i32 = self.m.op_s_max(i32_id, zero, biased);
        let z = self.m.op_bitcast(self.u32_id, z_i32);
        let shadow_texture_px = self.m.op_load(self.image_r32_id, self.shadow_texture_px_id);
        let px_size = self.m.op_image_query_size(ivec2, shadow_texture_px);
        let px_size_f = self.m.op_convert_s_to_f(vec2, px_size);
        let half = self.const_f32_vec(&[-0.5, -0.5]);
        let coord = self.m.op_fma(vec2, px_size_f, texcoord0, half);
        let coord_floor = self.m.op_floor(vec2, coord);
        let f = self.m.op_f_sub(vec2, coord, coord_floor);
        let i = self.m.op_convert_f_to_s(ivec2, coord_floor);

        let s_x = self.sample_shadow2d(i, px_size, shadow_texture_px, z);
        let offset = self.const_s32_vec(&[1, 0]);
        let i_10 = self.m.op_i_add(ivec2, i, offset);
        let s_y = self.sample_shadow2d(i_10, px_size, shadow_texture_px, z);
        let s_xy = self.m.op_composite_construct(vec2, &[s_x, s_y]);
        let offset = self.const_s32_vec(&[0, 1]);
        let i_01 = self.m.op_i_add(ivec2, i, offset);
        let s_z = self.sample_shadow2d(i_01, px_size, shadow_texture_px, z);
        let offset = self.const_s32_vec(&[1, 1]);
        let i_11 = self.m.op_i_add(ivec2, i, offset);
        let s_w = self.sample_shadow2d(i_11, px_size, shadow_texture_px, z);
        let s_zw = self.m.op_composite_construct(vec2, &[s_z, s_w]);
        let f_yy = self.m.op_vector_shuffle(vec2, f, f, &[1, 1]);
        let t = self.m.op_f_mix(vec2, s_xy, s_zw, f_yy);
        let t_x = self.m.op_composite_extract(f32_id, t, 0);
        let t_y = self.m.op_composite_extract(f32_id, t, 1);
        let a_x = self.m.op_composite_extract(f32_id, f, 0);
        let val = self.m.op_f_mix(f32_id, t_x, t_y, a_x);
        self.m
            .op_composite_construct(self.vec_ids.get(4), &[val, val, val, val])
    }

    fn sample_shadow2d(&mut self, uv: Id, px_size: Id, shadow_texture_px: Id, z: Id) -> Id {
        let bvec2 = self.bvec_ids.get(2);
        let true_label = self.m.op_label();
        let false_label = self.m.op_label();
        let end_label = self.m.op_label();
        let zero = self.const_s32_vec(&[0, 0]);
        let uv_le_zero = self.m.op_s_less_than(bvec2, uv, zero);
        let uv_geq_size = self.m.op_s_greater_than_equal(bvec2, uv, px_size);
        let conds = self
            .m
            .op_composite_construct(self.bvec_ids.get(4), &[uv_le_zero, uv_geq_size]);
        let cond = self.m.op_any(self.bool_id, conds);
        self.m.op_selection_merge(end_label, SelectionControl::NONE);
        self.m.op_branch_conditional(cond, true_label, false_label);
        self.m.add_label(true_label);
        self.m.op_branch(end_label);
        self.m.add_label(false_label);
        let px_texel = self
            .m
            .op_image_read(self.uvec_ids.get(4), shadow_texture_px, uv);
        let px_texel_x = self.m.op_composite_extract(self.u32_id, px_texel, 0);
        let result = self.compare_shadow(px_texel_x, z);
        self.m.op_branch(end_label);
        self.m.add_label(end_label);
        let one = self.const_f32(1.0);
        self.m
            .op_phi(self.f32_id, &[(one, true_label), (result, false_label)])
    }
}
