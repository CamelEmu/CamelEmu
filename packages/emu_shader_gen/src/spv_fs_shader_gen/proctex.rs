use emu_pica_types::regs::texturing::{ProcTexClamp, ProcTexCombiner, ProcTexFilter, ProcTexShift};
use rspirv::spirv::StorageClass;

use super::FragmentModule;
use crate::spv_module::{Id, valid_id};

impl FragmentModule<'_> {
    /// Function for sampling the procedurally generated texture unit.
    pub(super) fn proctex_sampler(&mut self) -> Id {
        let proctex = self.config.proctex;
        let vec2 = self.vec_ids.get(2);
        let f32_id = self.f32_id;
        let i32_id = self.i32_id;

        // Define noise tables at the beginning of the function
        if proctex.noise_enable() != 0 {
            let length = self.const_u32(16);
            let table_type = self.m.type_array(i32_id, length);
            self.noise1d_table = self.define_var(table_type, StorageClass::Function, false);
            let length = self.const_u32(16);
            let table_type = self.m.type_array(i32_id, length);
            self.noise2d_table = self.define_var(table_type, StorageClass::Function, false);
        }
        let length = self.const_u32(8);
        let table_type = self.m.type_array(i32_id, length);
        self.lut_offsets = self.define_var(table_type, StorageClass::Function, false);

        let mut uv = if proctex.coord() < 3 {
            let texcoord = self
                .m
                .op_load(vec2, self.texcoord_id[proctex.coord() as usize]);
            self.m.op_f_abs(vec2, texcoord)
        } else {
            log::error!(target: "Render", "Unexpected proctex.coord >= 3");
            let texcoord = self.m.op_load(vec2, self.texcoord_id[0]);
            self.m.op_f_abs(vec2, texcoord)
        };

        // This LOD formula is the same as the LOD upper limit defined in OpenGL.
        // f(x, y) <= m_u + m_v + m_w
        // (See OpenGL 4.6 spec, 8.14.1 - Scale Factor and Level-of-Detail)
        // Note: this is different from the one normal 2D textures use.
        let dpdx_uv = self.m.op_dpdx(vec2, uv);
        let uv_1 = self.m.op_f_abs(vec2, dpdx_uv);
        let dpdy_uv = self.m.op_dpdy(vec2, uv);
        let uv_2 = self.m.op_f_abs(vec2, dpdy_uv);
        let duv = self.m.op_f_max(vec2, uv_1, uv_2);

        // unlike normal texture, the bias is inside the log2
        let member = self.const_s32(16);
        let proctex_bias = self.get_shader_data_member(f32_id, &[member]);
        let lut_width = self.const_f32(proctex.lut_width as f32);
        let bias = self.m.op_f_mul(f32_id, lut_width, proctex_bias);
        let duv_x = self.m.op_composite_extract(f32_id, duv, 0);
        let duv_y = self.m.op_composite_extract(f32_id, duv, 1);
        let duv_xy = self.m.op_f_add(f32_id, duv_x, duv_y);

        let abs_bias = self.m.op_f_abs(f32_id, bias);
        let bias_mul_duv = self.m.op_f_mul(f32_id, abs_bias, duv_xy);
        let mut lod = self.m.op_log2(f32_id, bias_mul_duv);
        let zero = self.const_f32(0.0);
        let is_bias_zero = self.m.op_f_ord_equal(self.bool_id, proctex_bias, zero);
        let zero = self.const_f32(0.0);
        lod = self.m.op_select(f32_id, is_bias_zero, zero, lod);
        let lod_min = self.const_f32(f32::from(proctex.lod_min).max(0.0));
        let lod_max = self.const_f32(f32::from(proctex.lod_max).min(7.0));
        lod = self.m.op_f_clamp(f32_id, lod, lod_min, lod_max);

        // Get shift offset before noise generation
        let uv_y = self.m.op_composite_extract(f32_id, uv, 1);
        let u_shift = self.append_proctex_shift_offset(uv_y, proctex.u_shift(), proctex.u_clamp());
        let uv_x = self.m.op_composite_extract(f32_id, uv, 0);
        let v_shift = self.append_proctex_shift_offset(uv_x, proctex.v_shift(), proctex.v_clamp());

        // Generate noise
        if proctex.noise_enable() != 0 {
            let member = self.const_s32(21);
            let proctex_noise_a = self.get_shader_data_member(vec2, &[member]);
            let noise_coef = self.proctex_noise_coef(uv);
            let noise = self
                .m
                .op_vector_times_scalar(vec2, proctex_noise_a, noise_coef);
            uv = self.m.op_f_add(vec2, uv, noise);
            uv = self.m.op_f_abs(vec2, uv);
        }

        // Shift
        let uv_x = self.m.op_composite_extract(f32_id, uv, 0);
        let mut u = self.m.op_f_add(f32_id, uv_x, u_shift);
        let uv_y = self.m.op_composite_extract(f32_id, uv, 1);
        let mut v = self.m.op_f_add(f32_id, uv_y, v_shift);

        // Clamp
        u = self.append_proctex_clamp(u, proctex.u_clamp());
        v = self.append_proctex_clamp(v, proctex.v_clamp());

        // Combine and map
        let member = self.const_s32(12);
        let proctex_color_map_offset = self.get_shader_data_member(i32_id, &[member]);
        let lut_coord = self.append_proctex_combine_and_map(
            proctex.color_combiner(),
            u,
            v,
            proctex_color_map_offset,
        );

        let mut final_color = 0;
        match proctex.lut_filter() {
            ProcTexFilter::LINEAR | ProcTexFilter::NEAREST => {
                let level = self.const_s32(0);
                final_color = self.sample_proctex_color(lut_coord, level);
            }
            ProcTexFilter::NEAREST_MIPMAP_NEAREST | ProcTexFilter::LINEAR_MIPMAP_NEAREST => {
                let lod_rounded = self.m.op_round(f32_id, lod);
                let level = self.m.op_convert_f_to_s(i32_id, lod_rounded);
                final_color = self.sample_proctex_color(lut_coord, level);
            }
            ProcTexFilter::NEAREST_MIPMAP_LINEAR | ProcTexFilter::LINEAR_MIPMAP_LINEAR => {
                let lod_i = self.m.op_convert_f_to_s(i32_id, lod);
                let lod_f = self.m.op_fract(f32_id, lod);
                let color1 = self.sample_proctex_color(lut_coord, lod_i);
                let one = self.const_s32(1);
                let lod_i_next = self.m.op_i_add(i32_id, lod_i, one);
                let color2 = self.sample_proctex_color(lut_coord, lod_i_next);
                final_color = self.m.op_f_mix(f32_id, color1, color2, lod_f);
            }
            _ => {}
        }

        if proctex.separate_alpha() != 0 {
            let member = self.const_s32(13);
            let proctex_alpha_map_offset = self.get_shader_data_member(i32_id, &[member]);
            let final_alpha = self.append_proctex_combine_and_map(
                proctex.alpha_combiner(),
                u,
                v,
                proctex_alpha_map_offset,
            );
            final_color =
                self.m
                    .op_composite_insert(self.vec_ids.get(4), final_alpha, final_color, 3);
        }

        final_color
    }

    fn append_proctex_shift_offset(
        &mut self,
        v: Id,
        mode: ProcTexShift,
        clamp_mode: ProcTexClamp,
    ) -> Id {
        let offset = if clamp_mode == ProcTexClamp::MIRRORED_REPEAT {
            self.const_f32(1.0)
        } else {
            self.const_f32(0.5)
        };
        let v_i32 = self.m.op_convert_f_to_s(self.i32_id, v);

        let shift = |module: &mut Self, even: bool| -> Id {
            let i32_id = module.i32_id;
            let numerator = if even {
                let one = module.const_s32(1);
                module.m.op_i_add(i32_id, v_i32, one)
            } else {
                v_i32
            };
            let two = module.const_s32(2);
            let temp1 = module.m.op_s_div(i32_id, numerator, two);
            let two = module.const_s32(2);
            let temp1_mod = module.m.op_s_mod(i32_id, temp1, two);
            let temp2 = module.m.op_convert_s_to_f(module.f32_id, temp1_mod);
            module.m.op_f_mul(module.f32_id, offset, temp2)
        };

        match mode {
            ProcTexShift::NONE => self.const_f32(0.0),
            ProcTexShift::ODD => shift(self, false),
            ProcTexShift::EVEN => shift(self, true),
            _ => {
                log::error!(target: "Render", "Unknown shift mode {}", mode.0);
                self.const_f32(0.0)
            }
        }
    }

    fn append_proctex_clamp(&mut self, var: Id, mode: ProcTexClamp) -> Id {
        let f32_id = self.f32_id;
        let zero = self.const_f32(0.0);
        let one = self.const_f32(1.0);

        let mirrored_repeat = |module: &mut Self| -> Id {
            let fract = module.m.op_fract(f32_id, var);
            let var_i32 = module.m.op_convert_f_to_s(module.i32_id, var);
            let two = module.const_s32(2);
            let var_mod = module.m.op_s_mod(module.i32_id, var_i32, two);
            let zero_i32 = module.const_s32(0);
            let cond = module.m.op_i_equal(module.bool_id, var_mod, zero_i32);
            let one_minus_fract = module.m.op_f_sub(f32_id, one, fract);
            module.m.op_select(f32_id, cond, fract, one_minus_fract)
        };

        match mode {
            ProcTexClamp::TO_ZERO => {
                let cond = self.m.op_f_ord_greater_than(self.bool_id, var, one);
                self.m.op_select(f32_id, cond, zero, var)
            }
            ProcTexClamp::TO_EDGE => self.m.op_f_min(f32_id, var, one),
            ProcTexClamp::SYMMETRICAL_REPEAT => self.m.op_fract(f32_id, var),
            ProcTexClamp::MIRRORED_REPEAT => mirrored_repeat(self),
            ProcTexClamp::PULSE => {
                let half = self.const_f32(0.5);
                let cond = self.m.op_f_ord_greater_than(self.bool_id, var, half);
                self.m.op_select(f32_id, cond, one, zero)
            }
            _ => {
                log::error!(target: "Render", "Unknown clamp mode {}", mode.0);
                self.m.op_f_min(f32_id, var, one)
            }
        }
    }

    fn append_proctex_combine_and_map(
        &mut self,
        combiner: ProcTexCombiner,
        u: Id,
        v: Id,
        offset: Id,
    ) -> Id {
        let f32_id = self.f32_id;
        let v_mul_v = self.m.op_f_mul(f32_id, v, v);
        let u2v2 = self.m.op_fma(f32_id, u, u, v_mul_v);
        let combined = match combiner {
            ProcTexCombiner::U => u,
            ProcTexCombiner::U2 => self.m.op_f_mul(f32_id, u, u),
            ProcTexCombiner::V => v,
            ProcTexCombiner::V2 => self.m.op_f_mul(f32_id, v, v),
            ProcTexCombiner::ADD => {
                let sum = self.m.op_f_add(f32_id, u, v);
                let half = self.const_f32(0.5);
                self.m.op_f_mul(f32_id, sum, half)
            }
            ProcTexCombiner::ADD2 => {
                let half = self.const_f32(0.5);
                self.m.op_f_mul(f32_id, u2v2, half)
            }
            ProcTexCombiner::SQRT_ADD2 => {
                let sqrt = self.m.op_sqrt(f32_id, u2v2);
                let one = self.const_f32(1.0);
                self.m.op_f_min(f32_id, sqrt, one)
            }
            ProcTexCombiner::MIN => self.m.op_f_min(f32_id, u, v),
            ProcTexCombiner::MAX => self.m.op_f_max(f32_id, u, v),
            ProcTexCombiner::R_MAX => {
                let sum = self.m.op_f_add(f32_id, u, v);
                let half = self.const_f32(0.5);
                let sqrt = self.m.op_sqrt(f32_id, u2v2);
                let r = self.m.op_fma(f32_id, sum, half, sqrt);
                let half = self.const_f32(0.5);
                let r_half = self.m.op_f_mul(f32_id, r, half);
                let one = self.const_f32(1.0);
                self.m.op_f_min(f32_id, r_half, one)
            }
            _ => {
                log::error!(target: "Render", "Unknown combiner {}", combiner.0);
                self.const_f32(0.0)
            }
        };

        self.proctex_lookup_lut(offset, combined)
    }

    /// LUT sampling uitlity
    /// For NoiseLUT/ColorMap/AlphaMap, coord=0.0 is lut[0], coord=127.0/128.0 is lut[127] and
    /// coord=1.0 is lut[127]+lut_diff[127]. For other indices, the result is interpolated using
    /// value entries and difference entries.
    fn proctex_lookup_lut(&mut self, offset: Id, coord: Id) -> Id {
        let f32_id = self.f32_id;
        let i32_id = self.i32_id;
        let scale = self.const_f32(128.0);
        let coord = self.m.op_f_mul(f32_id, coord, scale);
        let coord_floor = self.m.op_floor(f32_id, coord);
        let min = self.const_f32(0.0);
        let max = self.const_f32(127.0);
        let index_i = self.m.op_f_clamp(f32_id, coord_floor, min, max);
        let index_f = self.m.op_f_sub(f32_id, coord, index_i);
        let index_i_s32 = self.m.op_convert_f_to_s(i32_id, index_i);
        let p = self.m.op_i_add(i32_id, index_i_s32, offset);
        if !valid_id(self.texture_buffer_lut_rg) {
            self.texture_buffer_lut_rg = self
                .m
                .op_load(self.image_buffer_id, self.texture_buffer_lut_rg_id);
        }
        let entry = self
            .m
            .op_image_fetch(self.vec_ids.get(4), self.texture_buffer_lut_rg, p);
        let entry_r = self.m.op_composite_extract(f32_id, entry, 0);
        let entry_g = self.m.op_composite_extract(f32_id, entry, 1);
        let value = self.m.op_fma(f32_id, entry_g, index_f, entry_r);
        let zero = self.const_f32(0.0);
        let one = self.const_f32(1.0);
        self.m.op_f_clamp(f32_id, value, zero, one)
    }

    fn proctex_noise_rand1d(&mut self, v: Id) -> Id {
        let i32_id = self.i32_id;
        self.init_table_s32(
            self.noise1d_table,
            &[0, 4, 10, 8, 4, 9, 7, 12, 5, 15, 13, 14, 11, 15, 2, 11],
        );
        let table_ptr = self.m.type_pointer(StorageClass::Function, i32_id);
        let nine = self.const_s32(9);
        let v_mod_9 = self.m.op_s_mod(i32_id, v, nine);
        let two = self.const_s32(2);
        let left_tmp = self.m.op_i_add(i32_id, v_mod_9, two);
        let three = self.const_s32(3);
        let left_mul = self.m.op_i_mul(i32_id, left_tmp, three);
        let mask = self.const_s32(0xF);
        let left = self.m.op_bitwise_and(i32_id, left_mul, mask);
        let nine = self.const_s32(9);
        let v_div_9 = self.m.op_s_div(i32_id, v, nine);
        let mask = self.const_s32(0xF);
        let table_index = self.m.op_bitwise_and(i32_id, v_div_9, mask);
        let table_value_ptr = self
            .m
            .op_access_chain(table_ptr, self.noise1d_table, &[table_index]);
        let table_value = self.m.op_load(i32_id, table_value_ptr);
        self.m.op_bitwise_xor(i32_id, left, table_value)
    }

    fn proctex_noise_rand2d(&mut self, point: Id) -> Id {
        let i32_id = self.i32_id;
        let f32_id = self.f32_id;
        self.init_table_s32(
            self.noise2d_table,
            &[10, 2, 15, 8, 0, 7, 4, 5, 5, 13, 2, 6, 13, 9, 3, 14],
        );
        let table_ptr = self.m.type_pointer(StorageClass::Function, i32_id);
        let point_x_f32 = self.m.op_composite_extract(f32_id, point, 0);
        let point_x = self.m.op_convert_f_to_s(i32_id, point_x_f32);
        let point_y_f32 = self.m.op_composite_extract(f32_id, point, 1);
        let point_y = self.m.op_convert_f_to_s(i32_id, point_y_f32);
        let u2 = self.proctex_noise_rand1d(point_x);
        let three = self.const_s32(3);
        let u2_and_3 = self.m.op_bitwise_and(i32_id, u2, three);
        let one = self.const_s32(1);
        let cond = self.m.op_i_equal(self.bool_id, u2_and_3, one);
        let table_value_ptr = self.m.op_access_chain(table_ptr, self.noise2d_table, &[u2]);
        let table_value = self.m.op_load(i32_id, table_value_ptr);
        let mut v2 = self.proctex_noise_rand1d(point_y);
        let four = self.const_s32(4);
        let zero = self.const_s32(0);
        let increment = self.m.op_select(i32_id, cond, four, zero);
        v2 = self.m.op_i_add(i32_id, v2, increment);
        let one = self.const_s32(1);
        let u2_and_1 = self.m.op_bitwise_and(i32_id, u2, one);
        let six = self.const_s32(6);
        let u2_and_1_mul_6 = self.m.op_i_mul(i32_id, u2_and_1, six);
        v2 = self.m.op_bitwise_xor(i32_id, v2, u2_and_1_mul_6);
        let ten = self.const_s32(10);
        let u2_plus_10 = self.m.op_i_add(i32_id, u2, ten);
        v2 = self.m.op_i_add(i32_id, v2, u2_plus_10);
        let mask = self.const_s32(0xF);
        v2 = self.m.op_bitwise_and(i32_id, v2, mask);
        v2 = self.m.op_bitwise_xor(i32_id, v2, table_value);
        let v2_f32 = self.m.op_convert_s_to_f(f32_id, v2);
        let scale = self.const_f32(2.0 / 15.0);
        let bias = self.const_f32(-1.0);
        self.m.op_fma(f32_id, v2_f32, scale, bias)
    }

    /// Generates random noise with proctex
    fn proctex_noise_coef(&mut self, x: Id) -> Id {
        let vec2 = self.vec_ids.get(2);
        let f32_id = self.f32_id;
        let member = self.const_s32(20);
        let proctex_noise_f = self.get_shader_data_member(vec2, &[member]);
        let member = self.const_s32(22);
        let proctex_noise_p = self.get_shader_data_member(vec2, &[member]);
        let nine = self.const_f32(9.0);
        let noise_f_mul_9 = self.m.op_vector_times_scalar(vec2, proctex_noise_f, nine);
        let x_plus_p = self.m.op_f_add(vec2, x, proctex_noise_p);
        let abs_x_plus_p = self.m.op_f_abs(vec2, x_plus_p);
        let grid = self.m.op_f_mul(vec2, noise_f_mul_9, abs_x_plus_p);
        let point = self.m.op_floor(vec2, grid);
        let frac = self.m.op_f_sub(vec2, grid, point);
        let frac_x = self.m.op_composite_extract(f32_id, frac, 0);
        let frac_y = self.m.op_composite_extract(f32_id, frac, 1);
        let frac_x_y = self.m.op_f_add(f32_id, frac_x, frac_y);
        let rand = self.proctex_noise_rand2d(point);
        let g0 = self.m.op_f_mul(f32_id, rand, frac_x_y);
        let one = self.const_f32(1.0);
        let frac_x_y_min_one = self.m.op_f_sub(f32_id, frac_x_y, one);
        let offset = self.const_f32_vec(&[1.0, 0.0]);
        let point_10 = self.m.op_f_add(vec2, point, offset);
        let rand = self.proctex_noise_rand2d(point_10);
        let g1 = self.m.op_f_mul(f32_id, rand, frac_x_y_min_one);
        let offset = self.const_f32_vec(&[0.0, 1.0]);
        let point_01 = self.m.op_f_add(vec2, point, offset);
        let rand = self.proctex_noise_rand2d(point_01);
        let g2 = self.m.op_f_mul(f32_id, rand, frac_x_y_min_one);
        let two = self.const_f32(2.0);
        let frac_x_y_min_two = self.m.op_f_sub(f32_id, frac_x_y, two);
        let offset = self.const_f32_vec(&[1.0, 1.0]);
        let point_11 = self.m.op_f_add(vec2, point, offset);
        let rand = self.proctex_noise_rand2d(point_11);
        let g3 = self.m.op_f_mul(f32_id, rand, frac_x_y_min_two);
        let member = self.const_s32(11);
        let proctex_noise_lut_offset = self.get_shader_data_member(self.i32_id, &[member]);
        let x_noise = self.proctex_lookup_lut(proctex_noise_lut_offset, frac_x);
        let y_noise = self.proctex_lookup_lut(proctex_noise_lut_offset, frac_y);
        let x0 = self.m.op_f_mix(f32_id, g0, g1, x_noise);
        let x1 = self.m.op_f_mix(f32_id, g2, g3, x_noise);
        self.m.op_f_mix(f32_id, x0, x1, y_noise)
    }

    /// Samples a color value from the rgba texture lut
    fn sample_proctex_color(&mut self, lut_coord: Id, level: Id) -> Id {
        let proctex = self.config.proctex;
        let f32_id = self.f32_id;
        let i32_id = self.i32_id;
        let vec4 = self.vec_ids.get(4);

        let width = self.const_s32(proctex.lut_width);
        let lut_width = self.m.op_shift_right_arithmetic(i32_id, width, level);
        let lut_ptr = self.m.type_pointer(StorageClass::Function, i32_id);
        // Offsets for level 4-7 seem to be hardcoded
        self.init_table_s32(
            self.lut_offsets,
            &[
                proctex.lut_offset0,
                proctex.lut_offset1,
                proctex.lut_offset2,
                proctex.lut_offset3,
                0xF0,
                0xF8,
                0xFC,
                0xFE,
            ],
        );
        let lut_offset_ptr = self.m.op_access_chain(lut_ptr, self.lut_offsets, &[level]);
        let lut_offset = self.m.op_load(i32_id, lut_offset_ptr);
        // For the color lut, coord=0.0 is lut[offset] and coord=1.0 is lut[offset+width-1]
        let one = self.const_s32(1);
        let lut_width_minus_one = self.m.op_i_sub(i32_id, lut_width, one);
        let lut_width_minus_one_f32 = self.m.op_convert_s_to_f(f32_id, lut_width_minus_one);
        let mut lut_coord = self.m.op_f_mul(f32_id, lut_coord, lut_width_minus_one_f32);

        if !valid_id(self.texture_buffer_lut_rgba) {
            self.texture_buffer_lut_rgba = self
                .m
                .op_load(self.image_buffer_id, self.texture_buffer_lut_rgba_id);
        }

        let member = self.const_s32(14);
        let proctex_lut_offset = self.get_shader_data_member(i32_id, &[member]);

        match proctex.lut_filter() {
            ProcTexFilter::LINEAR
            | ProcTexFilter::LINEAR_MIPMAP_LINEAR
            | ProcTexFilter::LINEAR_MIPMAP_NEAREST => {
                let lut_coord_s32 = self.m.op_convert_f_to_s(i32_id, lut_coord);
                let lut_index_i = self.m.op_i_add(i32_id, lut_coord_s32, lut_offset);
                let lut_index_f = self.m.op_fract(f32_id, lut_coord);
                let member = self.const_s32(15);
                let proctex_diff_lut_offset = self.get_shader_data_member(i32_id, &[member]);
                let p1 = self.m.op_i_add(i32_id, lut_index_i, proctex_lut_offset);
                let p2 = self
                    .m
                    .op_i_add(i32_id, lut_index_i, proctex_diff_lut_offset);
                let texel1 = self
                    .m
                    .op_image_fetch(vec4, self.texture_buffer_lut_rgba, p1);
                let texel2 = self
                    .m
                    .op_image_fetch(vec4, self.texture_buffer_lut_rgba, p2);
                let texel2_mul_f = self.m.op_vector_times_scalar(vec4, texel2, lut_index_f);
                self.m.op_f_add(vec4, texel1, texel2_mul_f)
            }
            ProcTexFilter::NEAREST
            | ProcTexFilter::NEAREST_MIPMAP_LINEAR
            | ProcTexFilter::NEAREST_MIPMAP_NEAREST => {
                let lut_offset_f32 = self.m.op_convert_s_to_f(f32_id, lut_offset);
                lut_coord = self.m.op_f_add(f32_id, lut_coord, lut_offset_f32);
                let lut_coord_round = self.m.op_round(f32_id, lut_coord);
                let lut_coord_rounded = self.m.op_convert_f_to_s(i32_id, lut_coord_round);
                let p = self
                    .m
                    .op_i_add(i32_id, lut_coord_rounded, proctex_lut_offset);
                self.m.op_image_fetch(vec4, self.texture_buffer_lut_rgba, p)
            }
            _ => 0,
        }
    }
}
