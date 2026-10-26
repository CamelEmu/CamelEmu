use emu_spirv::{Id, glsl};

use super::FragmentModule;
use crate::error::Result;
use crate::pica;

impl FragmentModule {
    pub(crate) fn define_tex_sampler(&mut self, texture_unit: u32) -> Result<()> {
        let vec4 = self.vec_ids.get(4);
        let func_type = self.type_function(vec4, &[]);
        self.sample_tex_unit_func[texture_unit as usize] =
            self.op_function(vec4, spirv::FunctionControl::NONE, func_type);
        let label = self.op_label();
        self.add_label(label);

        let zero_vec = self.const_f32s(&[0.0, 0.0, 0.0, 0.0]);

        let texture0_raw = self.config.texture.texture0_type();
        let texture0 = pica::texture0_type(texture0_raw);
        if texture_unit == 0 && texture0.is_disabled() {
            self.op_return_value(zero_vec);
            self.op_function_end();
            return Ok(());
        }

        if texture_unit == 3 {
            if self.config.proctex.enable() {
                let proctex = self.proctex_sampler();
                self.op_return_value(proctex);
            } else {
                self.op_return_value(zero_vec);
            }
            self.op_function_end();
            return Ok(());
        }

        let border_label = self.op_label();
        let not_border_label = self.op_label();

        let texcoord_num = if texture_unit == 2 && self.config.texture.texture2_use_coord1() {
            1
        } else {
            texture_unit
        };
        let (vec2, f32_id, bool_id) = (self.vec_ids.get(2), self.f32_id, self.bool_id);
        let texcoord_input = self.texcoord_id[texcoord_num as usize];
        let texcoord = self.op_load(vec2, texcoord_input, None);

        let enable_s = self.config.texture.border_enable_s(texture_unit as usize);
        let enable_t = self.config.texture.border_enable_t(texture_unit as usize);
        if enable_s || enable_t {
            let bvec2 = self.bvec_ids.get(2);
            let bvec4 = self.bvec_ids.get(4);
            let texcoord_s = self.composite_extract(f32_id, texcoord, 0);
            let texcoord_t = self.composite_extract(f32_id, texcoord, 1);

            let zero = self.const_f32(0.0);
            let s_lt_zero = self.op_f_ord_less_than(bool_id, texcoord_s, zero);
            let one = self.const_f32(1.0);
            let s_gt_one = self.op_f_ord_greater_than(bool_id, texcoord_s, one);
            let zero = self.const_f32(0.0);
            let t_lt_zero = self.op_f_ord_less_than(bool_id, texcoord_t, zero);
            let one = self.const_f32(1.0);
            let t_gt_one = self.op_f_ord_greater_than(bool_id, texcoord_t, one);

            let cond = if enable_s && enable_t {
                let conditions = self
                    .op_composite_construct(bvec4, &[s_lt_zero, s_gt_one, t_lt_zero, t_gt_one]);
                self.op_any(bool_id, conditions)
            } else if enable_s {
                let conditions = self.op_composite_construct(bvec2, &[s_lt_zero, s_gt_one]);
                self.op_any(bool_id, conditions)
            } else {
                let conditions = self.op_composite_construct(bvec2, &[t_lt_zero, t_gt_one]);
                self.op_any(bool_id, conditions)
            };

            self.op_selection_merge(not_border_label, spirv::SelectionControl::NONE);
            self.op_branch_conditional(cond, border_label, not_border_label, 0, 0);

            self.add_label(border_label);
            let border_member = self.settings.uniform.tex_border_color;
            let member = self.const_s32(border_member);
            let unit = self.const_u32(texture_unit);
            let border_color = self.get_shader_data_member(vec4, &[member, unit]);
            self.op_return_value(border_color);

            self.add_label(not_border_label);
        }

        let ret_val = match texture_unit {
            0 => texture0.sample(self, texcoord, texture0_raw)?,
            1 => {
                let tex1_id = self.tex1_id;
                self.sample_lod(tex1_id, texcoord, texture_unit)
            }
            _ => {
                let tex2_id = self.tex2_id;
                self.sample_lod(tex2_id, texcoord, texture_unit)
            }
        };

        self.op_return_value(ret_val);
        self.op_function_end();
        Ok(())
    }

    pub(crate) fn sample_lod(&mut self, tex_id: Id, texcoord: Id, texture_unit: u32) -> Id {
        let (f32_id, vec2, vec4, ivec2) = (
            self.f32_id,
            self.vec_ids.get(2),
            self.vec_ids.get(4),
            self.ivec_ids.get(2),
        );
        let uniform = self.settings.uniform.clone();
        let image2d_id = self.image2d_id;

        let sampled_image_type = self.type_sampled_image(image2d_id);
        let sampled_image = self.op_load(sampled_image_type, tex_id, None);
        let tex_image = self.op_image(image2d_id, sampled_image);
        let level = self.const_s32(0);
        let tex_size = self.op_image_query_size_lod(ivec2, tex_image, level);
        let size_f32 = self.op_convert_s_to_f(vec2, tex_size);
        let coord = self.op_f_mul(vec2, texcoord, size_f32);
        let dfdx = self.op_dpdx(vec2, coord);
        let abs_dfdx_coord = self.glsl::<glsl::FAbs>(vec2, &[dfdx]);
        let dfdy = self.op_dpdy(vec2, coord);
        let abs_dfdy_coord = self.glsl::<glsl::FAbs>(vec2, &[dfdy]);
        let d = self.glsl::<glsl::FMax>(vec2, &[abs_dfdx_coord, abs_dfdy_coord]);
        let d_x = self.composite_extract(f32_id, d, 0);
        let d_y = self.composite_extract(f32_id, d, 1);
        let dx_dy_max = self.glsl::<glsl::FMax>(f32_id, &[d_x, d_y]);
        let lod = self.glsl::<glsl::Log2>(f32_id, &[dx_dy_max]);
        let member = self.const_s32(uniform.tex_lod_bias);
        let unit = self.const_u32(texture_unit);
        let lod_bias = self.get_shader_data_member(f32_id, &[member, unit]);
        let biased_lod = self.op_f_add(f32_id, lod, lod_bias);
        self.op_image_sample_explicit_lod(
            vec4,
            sampled_image,
            texcoord,
            spirv::ImageOperands::LOD,
            &[biased_lod],
        )
    }

    pub(crate) fn sample_3d(&mut self, tex_id: Id, projection: bool, texcoord: Id) -> Id {
        let (f32_id, vec3, vec4) = (self.f32_id, self.vec_ids.get(3), self.vec_ids.get(4));
        let image_type = if !projection {
            self.image_cube_id
        } else {
            self.image2d_id
        };
        let sampled_image_type = self.type_sampled_image(image_type);
        let sampled_image = self.op_load(sampled_image_type, tex_id, None);
        let texcoord0_w_id = self.texcoord0_w_id;
        let texcoord0_w = self.op_load(f32_id, texcoord0_w_id, None);
        let texcoord_s = self.composite_extract(f32_id, texcoord, 0);
        let texcoord_t = self.composite_extract(f32_id, texcoord, 1);
        let coord = self.op_composite_construct(vec3, &[texcoord_s, texcoord_t, texcoord0_w]);
        if projection {
            self.op_image_sample_proj_implicit_lod(vec4, sampled_image, coord, None, &[])
        } else {
            self.op_image_sample_implicit_lod(vec4, sampled_image, coord, None, &[])
        }
    }
}
