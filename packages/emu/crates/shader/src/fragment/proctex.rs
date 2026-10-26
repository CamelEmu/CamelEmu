use emu_spirv::{Id, glsl};

use super::FragmentModule;
use crate::pica::{self, LutFetch};

impl FragmentModule {
    pub(crate) fn proctex_sampler(&mut self) -> Id {
        let (f32_id, i32_id, bool_id) = (self.f32_id, self.i32_id, self.bool_id);
        let (vec2, vec4) = (self.vec_ids.get(2), self.vec_ids.get(4));
        let settings = self.settings.proctex.clone();
        let uniform = self.settings.uniform.clone();
        let proctex = self.config.proctex;

        if proctex.noise_enable() {
            let length = self.const_u32(settings.noise_table_len);
            let table_type = self.type_array(i32_id, length);
            self.noise1d_table = self.define_var(table_type, spirv::StorageClass::Function, false);
            let length = self.const_u32(settings.noise_table_len);
            let table_type = self.type_array(i32_id, length);
            self.noise2d_table = self.define_var(table_type, spirv::StorageClass::Function, false);
        }
        let length = self.const_u32(settings.offset_table_len);
        let table_type = self.type_array(i32_id, length);
        self.lut_offsets = self.define_var(table_type, spirv::StorageClass::Function, false);

        let texcoord_index = if proctex.coord() < 3 {
            proctex.coord() as usize
        } else {
            0
        };
        let texcoord_input = self.texcoord_id[texcoord_index];
        let texcoord = self.op_load(vec2, texcoord_input, None);
        let mut uv = self.glsl::<glsl::FAbs>(vec2, &[texcoord]);

        let dpdx = self.op_dpdx(vec2, uv);
        let uv_1 = self.glsl::<glsl::FAbs>(vec2, &[dpdx]);
        let dpdy = self.op_dpdy(vec2, uv);
        let uv_2 = self.glsl::<glsl::FAbs>(vec2, &[dpdy]);
        let duv = self.glsl::<glsl::FMax>(vec2, &[uv_1, uv_2]);

        let member = self.const_s32(uniform.proctex_bias);
        let proctex_bias = self.get_shader_data_member(f32_id, &[member]);
        let lut_width = self.const_f32(proctex.lut_width as f32);
        let bias = self.op_f_mul(f32_id, lut_width, proctex_bias);
        let duv_x = self.composite_extract(f32_id, duv, 0);
        let duv_y = self.composite_extract(f32_id, duv, 1);
        let duv_xy = self.op_f_add(f32_id, duv_x, duv_y);

        let abs_bias = self.glsl::<glsl::FAbs>(f32_id, &[bias]);
        let scaled = self.op_f_mul(f32_id, abs_bias, duv_xy);
        let mut lod = self.glsl::<glsl::Log2>(f32_id, &[scaled]);
        let zero = self.const_f32(0.0);
        let bias_is_zero = self.op_f_ord_equal(bool_id, proctex_bias, zero);
        let zero = self.const_f32(0.0);
        lod = self.op_select(f32_id, bias_is_zero, zero, lod);
        let lod_min = self.const_f32(settings.lod_min_limit.max(f32::from(proctex.lod_min)));
        let lod_max = self.const_f32(settings.lod_max_limit.min(f32::from(proctex.lod_max)));
        lod = self.glsl::<glsl::FClamp>(f32_id, &[lod, lod_min, lod_max]);

        let uv_y = self.composite_extract(f32_id, uv, 1);
        let u_shift = self.append_proctex_shift_offset(uv_y, proctex.u_shift(), proctex.u_clamp());
        let uv_x = self.composite_extract(f32_id, uv, 0);
        let v_shift = self.append_proctex_shift_offset(uv_x, proctex.v_shift(), proctex.v_clamp());

        if proctex.noise_enable() {
            let member = self.const_s32(uniform.proctex_noise_a);
            let proctex_noise_a = self.get_shader_data_member(vec2, &[member]);
            let noise_coef = self.proctex_noise_coef(uv);
            let noise = self.op_vector_times_scalar(vec2, proctex_noise_a, noise_coef);
            uv = self.op_f_add(vec2, uv, noise);
            uv = self.glsl::<glsl::FAbs>(vec2, &[uv]);
        }

        let uv_x = self.composite_extract(f32_id, uv, 0);
        let mut u = self.op_f_add(f32_id, uv_x, u_shift);
        let uv_y = self.composite_extract(f32_id, uv, 1);
        let mut v = self.op_f_add(f32_id, uv_y, v_shift);

        u = self.append_proctex_clamp(u, proctex.u_clamp());
        v = self.append_proctex_clamp(v, proctex.v_clamp());

        let member = self.const_s32(uniform.proctex_color_map_offset);
        let proctex_color_map_offset = self.get_shader_data_member(i32_id, &[member]);
        let lut_coord = self.append_proctex_combine_and_map(
            proctex.color_combiner(),
            u,
            v,
            proctex_color_map_offset,
        );

        let mut final_color = pica::proctex_filter(proctex.lut_filter()).final_color(
            self,
            lut_coord,
            lod,
        );

        if proctex.separate_alpha() {
            let member = self.const_s32(uniform.proctex_alpha_map_offset);
            let proctex_alpha_map_offset = self.get_shader_data_member(i32_id, &[member]);
            let final_alpha = self.append_proctex_combine_and_map(
                proctex.alpha_combiner(),
                u,
                v,
                proctex_alpha_map_offset,
            );
            final_color = self.op_composite_insert(vec4, final_alpha, final_color, &[&3i32]);
        }

        final_color
    }

    fn append_proctex_shift_offset(&mut self, v: Id, shift_raw: u32, clamp_raw: u32) -> Id {
        let i32_id = self.i32_id;
        let offset = if pica::proctex_clamp(clamp_raw).is_mirrored_repeat() {
            self.const_f32(1.0)
        } else {
            self.const_f32(0.5)
        };
        let v_i32 = self.op_convert_f_to_s(i32_id, v);
        pica::proctex_shift(shift_raw).offset(self, offset, v_i32)
    }

    fn append_proctex_clamp(&mut self, var: Id, mode_raw: u32) -> Id {
        let zero = self.const_f32(0.0);
        let one = self.const_f32(1.0);
        pica::proctex_clamp(mode_raw).apply(self, var, zero, one)
    }

    fn append_proctex_combine_and_map(
        &mut self,
        combiner_raw: u32,
        u: Id,
        v: Id,
        offset: Id,
    ) -> Id {
        let f32_id = self.f32_id;
        let v_squared = self.op_f_mul(f32_id, v, v);
        let u2v2 = self.glsl::<glsl::Fma>(f32_id, &[u, u, v_squared]);
        let combined = pica::proctex_combiner(combiner_raw).combine(self, u, v, u2v2);

        self.proctex_lookup_lut(offset, combined)
    }

    fn proctex_lookup_lut(&mut self, offset: Id, coord: Id) -> Id {
        let (f32_id, i32_id, vec4) = (self.f32_id, self.i32_id, self.vec_ids.get(4));
        let settings = self.settings.proctex.clone();

        let scale = self.const_f32(settings.lut_scale);
        let coord = self.op_f_mul(f32_id, coord, scale);
        let floor = self.glsl::<glsl::Floor>(f32_id, &[coord]);
        let zero = self.const_f32(0.0);
        let max = self.const_f32(settings.lut_index_max);
        let index_i = self.glsl::<glsl::FClamp>(f32_id, &[floor, zero, max]);
        let index_f = self.op_f_sub(f32_id, coord, index_i);
        let index_i_s32 = self.op_convert_f_to_s(i32_id, index_i);
        let p = self.op_i_add(i32_id, index_i_s32, offset);
        if self.texture_buffer_lut_rg.value == 0 {
            let (image_buffer_id, lut_id) = (self.image_buffer_id, self.texture_buffer_lut_rg_id);
            self.texture_buffer_lut_rg = self.op_load(image_buffer_id, lut_id, None);
        }
        let lut_image = self.texture_buffer_lut_rg;
        let entry = self.op_image_fetch(vec4, lut_image, p, None, &[]);
        let entry_r = self.composite_extract(f32_id, entry, 0);
        let entry_g = self.composite_extract(f32_id, entry, 1);
        let lerp = self.glsl::<glsl::Fma>(f32_id, &[entry_g, index_f, entry_r]);
        let zero = self.const_f32(0.0);
        let one = self.const_f32(1.0);
        self.glsl::<glsl::FClamp>(f32_id, &[lerp, zero, one])
    }

    fn proctex_rand_1d(&mut self, v: Id) -> Id {
        let i32_id = self.i32_id;
        let settings = self.settings.proctex.clone();

        let noise1d_table = self.noise1d_table;
        self.init_table_s32(noise1d_table, &settings.noise1d_table);
        let table_ptr = self.type_pointer(spirv::StorageClass::Function, i32_id);
        let modulus = self.const_s32(settings.noise_modulus);
        let remainder = self.op_s_mod(i32_id, v, modulus);
        let bias = self.const_s32(settings.noise_bias);
        let left_tmp = self.op_i_add(i32_id, remainder, bias);
        let multiplier = self.const_s32(settings.noise_multiplier);
        let product = self.op_i_mul(i32_id, left_tmp, multiplier);
        let mask = self.const_s32(settings.noise_mask);
        let left = self.op_bitwise_and(i32_id, product, mask);
        let modulus = self.const_s32(settings.noise_modulus);
        let quotient = self.op_s_div(i32_id, v, modulus);
        let mask = self.const_s32(settings.noise_mask);
        let table_index = self.op_bitwise_and(i32_id, quotient, mask);
        let access_chain = self.op_access_chain(table_ptr, noise1d_table, &[table_index]);
        let table_value = self.op_load(i32_id, access_chain, None);
        self.op_bitwise_xor(i32_id, left, table_value)
    }

    fn proctex_rand_2d(&mut self, point: Id) -> Id {
        let (f32_id, i32_id, bool_id) = (self.f32_id, self.i32_id, self.bool_id);
        let settings = self.settings.proctex.clone();

        let noise2d_table = self.noise2d_table;
        self.init_table_s32(noise2d_table, &settings.noise2d_table);
        let table_ptr = self.type_pointer(spirv::StorageClass::Function, i32_id);
        let point_x_f32 = self.composite_extract(f32_id, point, 0);
        let point_x = self.op_convert_f_to_s(i32_id, point_x_f32);
        let point_y_f32 = self.composite_extract(f32_id, point, 1);
        let point_y = self.op_convert_f_to_s(i32_id, point_y_f32);
        let u2 = self.proctex_rand_1d(point_x);
        let three = self.const_s32(3);
        let masked = self.op_bitwise_and(i32_id, u2, three);
        let one = self.const_s32(settings.noise_mix_condition);
        let cond = self.op_i_equal(bool_id, masked, one);
        let access_chain = self.op_access_chain(table_ptr, noise2d_table, &[u2]);
        let table_value = self.op_load(i32_id, access_chain, None);
        let mut v2 = self.proctex_rand_1d(point_y);
        let increment = self.const_s32(settings.noise_mix_increment);
        let zero = self.const_s32(0);
        let selected = self.op_select(i32_id, cond, increment, zero);
        v2 = self.op_i_add(i32_id, v2, selected);
        let one = self.const_s32(1);
        let low_bit = self.op_bitwise_and(i32_id, u2, one);
        let xor_multiplier = self.const_s32(settings.noise_xor_multiplier);
        let scaled = self.op_i_mul(i32_id, low_bit, xor_multiplier);
        v2 = self.op_bitwise_xor(i32_id, v2, scaled);
        let add = self.const_s32(settings.noise_add);
        let u2_plus = self.op_i_add(i32_id, u2, add);
        v2 = self.op_i_add(i32_id, v2, u2_plus);
        let mask = self.const_s32(settings.noise_mask);
        v2 = self.op_bitwise_and(i32_id, v2, mask);
        v2 = self.op_bitwise_xor(i32_id, v2, table_value);
        let v2_f32 = self.op_convert_s_to_f(f32_id, v2);
        let scale = self.const_f32(settings.noise_result_scale);
        let bias = self.const_f32(settings.noise_result_bias);
        self.glsl::<glsl::Fma>(f32_id, &[v2_f32, scale, bias])
    }

    fn proctex_noise_coef(&mut self, x: Id) -> Id {
        let (f32_id, i32_id, vec2) = (self.f32_id, self.i32_id, self.vec_ids.get(2));
        let settings = self.settings.proctex.clone();
        let uniform = self.settings.uniform.clone();

        let member = self.const_s32(uniform.proctex_noise_f);
        let proctex_noise_f = self.get_shader_data_member(vec2, &[member]);
        let member = self.const_s32(uniform.proctex_noise_p);
        let proctex_noise_p = self.get_shader_data_member(vec2, &[member]);
        let grid_scale = self.const_f32(settings.noise_grid_scale);
        let scaled_f = self.op_vector_times_scalar(vec2, proctex_noise_f, grid_scale);
        let shifted = self.op_f_add(vec2, x, proctex_noise_p);
        let abs_shifted = self.glsl::<glsl::FAbs>(vec2, &[shifted]);
        let grid = self.op_f_mul(vec2, scaled_f, abs_shifted);
        let point = self.glsl::<glsl::Floor>(vec2, &[grid]);
        let frac = self.op_f_sub(vec2, grid, point);
        let frac_x = self.composite_extract(f32_id, frac, 0);
        let frac_y = self.composite_extract(f32_id, frac, 1);
        let frac_x_y = self.op_f_add(f32_id, frac_x, frac_y);
        let rand_0 = self.proctex_rand_2d(point);
        let g0 = self.op_f_mul(f32_id, rand_0, frac_x_y);
        let one = self.const_f32(1.0);
        let frac_x_y_min_one = self.op_f_sub(f32_id, frac_x_y, one);
        let offset_10 = self.const_f32s(&[1.0, 0.0]);
        let point_10 = self.op_f_add(vec2, point, offset_10);
        let rand_1 = self.proctex_rand_2d(point_10);
        let g1 = self.op_f_mul(f32_id, rand_1, frac_x_y_min_one);
        let offset_01 = self.const_f32s(&[0.0, 1.0]);
        let point_01 = self.op_f_add(vec2, point, offset_01);
        let rand_2 = self.proctex_rand_2d(point_01);
        let g2 = self.op_f_mul(f32_id, rand_2, frac_x_y_min_one);
        let two = self.const_f32(2.0);
        let frac_x_y_min_two = self.op_f_sub(f32_id, frac_x_y, two);
        let offset_11 = self.const_f32s(&[1.0, 1.0]);
        let point_11 = self.op_f_add(vec2, point, offset_11);
        let rand_3 = self.proctex_rand_2d(point_11);
        let g3 = self.op_f_mul(f32_id, rand_3, frac_x_y_min_two);
        let member = self.const_s32(uniform.proctex_noise_lut_offset);
        let proctex_noise_lut_offset = self.get_shader_data_member(i32_id, &[member]);
        let x_noise = self.proctex_lookup_lut(proctex_noise_lut_offset, frac_x);
        let y_noise = self.proctex_lookup_lut(proctex_noise_lut_offset, frac_y);
        let x0 = self.glsl::<glsl::FMix>(f32_id, &[g0, g1, x_noise]);
        let x1 = self.glsl::<glsl::FMix>(f32_id, &[g2, g3, x_noise]);
        self.glsl::<glsl::FMix>(f32_id, &[x0, x1, y_noise])
    }

    pub(crate) fn sample_proctex_color(&mut self, lut_coord: Id, level: Id) -> Id {
        let (f32_id, i32_id) = (self.f32_id, self.i32_id);
        let settings = self.settings.proctex.clone();
        let uniform = self.settings.uniform.clone();
        let proctex = self.config.proctex;

        let width = self.const_s32(proctex.lut_width);
        let lut_width = self.op_shift_right_arithmetic(i32_id, width, level);
        let lut_ptr = self.type_pointer(spirv::StorageClass::Function, i32_id);
        let lut_offsets = self.lut_offsets;
        let offsets = [
            proctex.lut_offset0,
            proctex.lut_offset1,
            proctex.lut_offset2,
            proctex.lut_offset3,
            settings.fixed_lut_offsets[0],
            settings.fixed_lut_offsets[1],
            settings.fixed_lut_offsets[2],
            settings.fixed_lut_offsets[3],
        ];
        self.init_table_s32(lut_offsets, &offsets);
        let access_chain = self.op_access_chain(lut_ptr, lut_offsets, &[level]);
        let lut_offset = self.op_load(i32_id, access_chain, None);
        let one = self.const_s32(1);
        let width_minus_one = self.op_i_sub(i32_id, lut_width, one);
        let width_f32 = self.op_convert_s_to_f(f32_id, width_minus_one);
        let lut_coord = self.op_f_mul(f32_id, lut_coord, width_f32);

        if self.texture_buffer_lut_rgba.value == 0 {
            let (image_buffer_id, lut_id) = (self.image_buffer_id, self.texture_buffer_lut_rgba_id);
            self.texture_buffer_lut_rgba = self.op_load(image_buffer_id, lut_id, None);
        }

        let member = self.const_s32(uniform.proctex_lut_offset);
        let proctex_lut_offset = self.get_shader_data_member(i32_id, &[member]);

        pica::proctex_filter(proctex.lut_filter()).fetch(
            self,
            &LutFetch {
                lut_coord,
                lut_offset,
                proctex_lut_offset,
            },
        )
    }
}
