use emu_spirv::{Id, glsl};

use super::FragmentModule;
use crate::config::LutConfig;
use crate::pica::{self, LightingSampler, LutInputContext};

impl FragmentModule {
    pub(crate) fn perturbation(&mut self) -> Id {
        let (vec3, vec4) = (self.vec_ids.get(3), self.vec_ids.get(4));
        let function = self.sample_tex_unit_func[self.config.lighting.bump_selector() as usize];
        let texel = self.op_function_call(vec4, function, &[]);
        let texel_rgb = self.vector_shuffle(vec3, texel, texel, &[0, 1, 2]);
        let two = self.const_f32(2.0);
        let rgb_mul_two = self.op_vector_times_scalar(vec3, texel_rgb, two);
        let ones = self.const_f32s(&[1.0, 1.0, 1.0]);
        self.op_f_sub(vec3, rgb_mul_two, ones)
    }

    fn quaternion_rotate(&mut self, q: Id, v: Id) -> Id {
        let (vec3, f32_id) = (self.vec_ids.get(3), self.f32_id);
        let q_xyz = self.vector_shuffle(vec3, q, q, &[0, 1, 2]);
        let q_xyz_cross_v = self.glsl::<glsl::Cross>(vec3, &[q_xyz, v]);
        let q_w = self.composite_extract(f32_id, q, 3);
        let v_times_w = self.op_vector_times_scalar(vec3, v, q_w);
        let val1 = self.op_f_add(vec3, q_xyz_cross_v, v_times_w);
        let cross = self.glsl::<glsl::Cross>(vec3, &[q_xyz, val1]);
        let two = self.const_f32(2.0);
        let val2 = self.op_vector_times_scalar(vec3, cross, two);
        self.op_f_add(vec3, v, val2)
    }

    fn are_quaternions_opposite(&mut self, qa: Id, qb: Id) -> Id {
        let (f32_id, bool_id, bvec4) = (self.f32_id, self.bool_id, self.bvec_ids.get(4));
        let dot_q = self.op_dot(f32_id, qa, qb);
        let zero = self.const_f32(0.0);
        let dot_le_zero = self.op_f_ord_less_than(bool_id, dot_q, zero);
        self.op_composite_construct(
            bvec4,
            &[dot_le_zero, dot_le_zero, dot_le_zero, dot_le_zero],
        )
    }

    fn load_normquat(&mut self, input_pointer_id: Id, index: i32) -> Id {
        let vec4 = self.vec_ids.get(4);
        let normquat_id = self.normquat_id;
        let index = self.const_s32(index);
        let access_chain = self.op_access_chain(input_pointer_id, normquat_id, &[index]);
        self.op_load(vec4, access_chain, None)
    }

    fn lookup_lighting_lut_unsigned(&mut self, lut_index: Id, pos: Id) -> Id {
        let (f32_id, i32_id) = (self.f32_id, self.i32_id);
        let settings = self.settings.lighting.clone();
        let scale = self.const_f32(settings.unsigned_scale);
        let scaled = self.op_f_mul(f32_id, pos, scale);
        let pos_floor = self.glsl::<glsl::Floor>(f32_id, &[scaled]);
        let zero = self.const_f32(0.0);
        let max = self.const_f32(settings.unsigned_index_max);
        let index_float = self.glsl::<glsl::FClamp>(f32_id, &[pos_floor, zero, max]);
        let index = self.op_convert_f_to_s(i32_id, index_float);
        let index_f32 = self.op_convert_s_to_f(f32_id, index);
        let neg_index = self.op_f_negate(f32_id, index_f32);
        let scale = self.const_f32(settings.unsigned_scale);
        let delta = self.glsl::<glsl::Fma>(f32_id, &[pos, scale, neg_index]);
        self.lookup_lighting_lut(lut_index, index, delta)
    }

    fn lookup_lighting_lut_signed(&mut self, lut_index: Id, pos: Id) -> Id {
        let (f32_id, i32_id, bool_id) = (self.f32_id, self.i32_id, self.bool_id);
        let settings = self.settings.lighting.clone();
        let scale = self.const_f32(settings.signed_scale);
        let scaled = self.op_f_mul(f32_id, pos, scale);
        let pos_floor = self.glsl::<glsl::Floor>(f32_id, &[scaled]);
        let min = self.const_f32(settings.signed_index_min);
        let max = self.const_f32(settings.signed_index_max);
        let index_float = self.glsl::<glsl::FClamp>(f32_id, &[pos_floor, min, max]);
        let index = self.op_convert_f_to_s(i32_id, index_float);
        let index_f32 = self.op_convert_s_to_f(f32_id, index);
        let neg_index = self.op_f_negate(f32_id, index_f32);
        let scale = self.const_f32(settings.signed_scale);
        let delta = self.glsl::<glsl::Fma>(f32_id, &[pos, scale, neg_index]);
        let zero = self.const_s32(0);
        let negative = self.op_s_less_than(bool_id, index, zero);
        let increment_value = self.const_s32(settings.signed_negative_increment);
        let zero = self.const_s32(0);
        let increment = self.op_select(i32_id, negative, increment_value, zero);
        let shifted_index = self.op_i_add(i32_id, index, increment);
        self.lookup_lighting_lut(lut_index, shifted_index, delta)
    }

    fn get_lut_value(
        &mut self,
        context: &LutInputContext,
        sampler_value: u32,
        light_num: u32,
        input_raw: u32,
        abs: bool,
    ) -> Id {
        let f32_id = self.f32_id;
        let mut index = pica::lighting_lut_input(input_raw).index(self, context);

        let sampler_index = self.const_u32(sampler_value);
        if abs {
            index = if self.config.lighting.lights[light_num as usize].two_sided_diffuse() {
                self.glsl::<glsl::FAbs>(f32_id, &[index])
            } else {
                let zero = self.const_f32(0.0);
                self.glsl::<glsl::FMax>(f32_id, &[index, zero])
            };
            self.lookup_lighting_lut_unsigned(sampler_index, index)
        } else {
            self.lookup_lighting_lut_signed(sampler_index, index)
        }
    }

    fn scaled_lut_value(
        &mut self,
        context: &LutInputContext,
        sampler_value: u32,
        light_num: u32,
        lut: LutConfig,
    ) -> Id {
        let f32_id = self.f32_id;
        let value = self.get_lut_value(
            context,
            sampler_value,
            light_num,
            lut.input_type(),
            lut.abs_input(),
        );
        let scale = self.const_f32(lut.scale);
        self.op_f_mul(f32_id, scale, value)
    }

    fn get_light_member(&mut self, light_index: usize, member: i32) -> Id {
        let uniform = self.settings.uniform.clone();
        let member_type = if member < uniform.light_vec3_members {
            self.vec_ids.get(3)
        } else {
            self.f32_id
        };
        let num = self.config.lighting.lights[light_index].num() as i32;
        let light_num = self.const_s32(num);
        let light_src = self.const_s32(uniform.light_src);
        let member = self.const_s32(member);
        self.get_shader_data_member(member_type, &[light_src, light_num, member])
    }

    pub(crate) fn write_lighting(&mut self) {
        if !self.config.lighting.enable() {
            return;
        }

        let (f32_id, vec3, vec4) = (self.f32_id, self.vec_ids.get(3), self.vec_ids.get(4));
        let bool_id = self.bool_id;
        let uniform = self.settings.uniform.clone();
        let lighting = self.config.lighting.clone();
        let lighting_config = lighting.config();

        let mut diffuse_sum = self.const_f32s(&[0.0, 0.0, 0.0, 1.0]);
        let mut specular_sum = self.const_f32s(&[0.0, 0.0, 0.0, 1.0]);
        self.const_f32s(&[0.0, 0.0, 0.0]);
        self.const_f32(0.0);
        self.const_f32s(&[0.0, 0.0, 0.0]);
        self.const_f32s(&[0.0, 0.0, 0.0]);
        self.const_f32(0.0);
        let mut clamp_highlights = self.const_f32(1.0);
        let mut geo_factor = self.const_f32(1.0);

        let (surface_normal, surface_tangent) =
            pica::bump_mode(lighting.bump_mode()).surface(self);

        let normquat = if self.use_fragment_shader_barycentric {
            let input_pointer_id = self.type_pointer(spirv::StorageClass::Input, vec4);
            let normquat_0 = self.load_normquat(input_pointer_id, 0);
            let normquat_1 = self.load_normquat(input_pointer_id, 1);
            let opposite_1 = self.are_quaternions_opposite(normquat_0, normquat_1);
            let negated_1 = self.op_f_negate(vec4, normquat_1);
            let normquat_1_correct = self.op_select(vec4, opposite_1, negated_1, normquat_1);
            let normquat_2 = self.load_normquat(input_pointer_id, 2);
            let opposite_2 = self.are_quaternions_opposite(normquat_0, normquat_2);
            let negated_2 = self.op_f_negate(vec4, normquat_2);
            let normquat_2_correct = self.op_select(vec4, opposite_2, negated_2, normquat_2);
            let gl_bary_coord_id = self.gl_bary_coord_id;
            let bary_coord = self.op_load(vec3, gl_bary_coord_id, None);
            let bary_coord_x = self.composite_extract(f32_id, bary_coord, 0);
            let bary_coord_y = self.composite_extract(f32_id, bary_coord, 1);
            let bary_coord_z = self.composite_extract(f32_id, bary_coord, 2);
            let normquat_0_final = self.op_vector_times_scalar(vec4, normquat_0, bary_coord_x);
            let normquat_1_final =
                self.op_vector_times_scalar(vec4, normquat_1_correct, bary_coord_y);
            let normquat_2_final =
                self.op_vector_times_scalar(vec4, normquat_2_correct, bary_coord_z);
            let sum = self.op_f_add(vec4, normquat_1_final, normquat_2_final);
            self.op_f_add(vec4, normquat_0_final, sum)
        } else {
            let normquat_id = self.normquat_id;
            self.op_load(vec4, normquat_id, None)
        };

        let normalized_normquat = self.glsl::<glsl::Normalize>(vec4, &[normquat]);
        let normal = self.quaternion_rotate(normalized_normquat, surface_normal);
        let tangent = self.quaternion_rotate(normalized_normquat, surface_tangent);

        let mut shadow = self.const_f32s(&[1.0, 1.0, 1.0, 1.0]);
        if lighting.enable_shadow() {
            let function = self.sample_tex_unit_func[lighting.shadow_selector() as usize];
            shadow = self.op_function_call(vec4, function, &[]);
            if lighting.shadow_invert() {
                let ones = self.const_f32s(&[1.0, 1.0, 1.0, 1.0]);
                shadow = self.op_f_sub(vec4, ones, shadow);
            }
        }

        let view_id = self.view_id;
        let view = self.op_load(vec3, view_id, None);

        for light_index in 0..lighting.src_num() as usize {
            let light_config = lighting.lights[light_index];
            let light_num = light_config.num();

            let light_position = self.get_light_member(light_index, uniform.light_position);
            let light_vector = if light_config.directional() {
                light_position
            } else {
                self.op_f_add(vec3, light_position, view)
            };

            let light_distance = self.glsl::<glsl::Length>(f32_id, &[light_vector]);
            let light_vector = self.glsl::<glsl::Normalize>(vec3, &[light_vector]);

            let spot_dir = self.get_light_member(light_index, uniform.light_spot_direction);
            let normalized_view = self.glsl::<glsl::Normalize>(vec3, &[view]);
            let half_vector = self.op_f_add(vec3, normalized_view, light_vector);

            let dot_product = if light_config.two_sided_diffuse() {
                let dot = self.op_dot(f32_id, light_vector, normal);
                self.glsl::<glsl::FAbs>(f32_id, &[dot])
            } else {
                let dot = self.op_dot(f32_id, light_vector, normal);
                let zero = self.const_f32(0.0);
                self.glsl::<glsl::FMax>(f32_id, &[dot, zero])
            };

            if lighting.clamp_highlights() {
                clamp_highlights = self.glsl::<glsl::FSign>(f32_id, &[dot_product]);
            }

            let context = LutInputContext {
                normal,
                tangent,
                view,
                half_vector,
                light_vector,
                spot_dir,
                config: lighting_config,
            };

            let mut spot_atten = self.const_f32(1.0);
            let spotlight = pica::SpotlightAttenuation;
            if light_config.spot_atten_enable() && spotlight.is_supported(lighting_config) {
                spot_atten =
                    self.scaled_lut_value(&context, spotlight.value(light_num), light_num, lighting.lut_sp);
            }

            let mut dist_atten = self.const_f32(1.0);
            if light_config.dist_atten_enable() {
                let dist_atten_scale = self.get_light_member(light_index, uniform.light_dist_atten_scale);
                let dist_atten_bias = self.get_light_member(light_index, uniform.light_dist_atten_bias);
                let index = self.glsl::<glsl::Fma>(
                    f32_id,
                    &[dist_atten_scale, light_distance, dist_atten_bias],
                );
                let zero = self.const_f32(0.0);
                let one = self.const_f32(1.0);
                let clamped_index = self.glsl::<glsl::FClamp>(f32_id, &[index, zero, one]);
                let sampler = pica::DistanceAttenuation.value(light_num) as i32;
                let sampler = self.const_s32(sampler);
                dist_atten = self.lookup_lighting_lut_unsigned(sampler, clamped_index);
            }

            if light_config.geometric_factor_0() || light_config.geometric_factor_1() {
                geo_factor = self.op_dot(f32_id, half_vector, half_vector);
                let quotient = self.op_f_div(f32_id, dot_product, geo_factor);
                let one = self.const_f32(1.0);
                let dot_div_geo = self.glsl::<glsl::FMin>(f32_id, &[quotient, one]);
                let zero = self.const_f32(0.0);
                let is_geo_factor_zero = self.op_f_ord_equal(bool_id, geo_factor, zero);
                let zero = self.const_f32(0.0);
                geo_factor = self.op_select(f32_id, is_geo_factor_zero, zero, dot_div_geo);
            }

            let mut d0_lut_value = self.const_f32(1.0);
            let distribution_0 = pica::Distribution0;
            if lighting.lut_d0.enable() && distribution_0.is_supported(lighting_config) {
                d0_lut_value = self.scaled_lut_value(
                    &context,
                    distribution_0.value(light_num),
                    light_num,
                    lighting.lut_d0,
                );
            }

            let specular_0_member = self.get_light_member(light_index, uniform.light_specular_0);
            let mut specular_0 = self.op_vector_times_scalar(vec3, specular_0_member, d0_lut_value);
            if light_config.geometric_factor_0() {
                specular_0 = self.op_vector_times_scalar(vec3, specular_0, geo_factor);
            }

            let mut refl_value_r = self.const_f32(1.0);
            let reflect_red = pica::ReflectRed;
            if lighting.lut_rr.enable() && reflect_red.is_supported(lighting_config) {
                refl_value_r = self.scaled_lut_value(
                    &context,
                    reflect_red.value(light_num),
                    light_num,
                    lighting.lut_rr,
                );
            }

            let mut refl_value_g = refl_value_r;
            let reflect_green = pica::ReflectGreen;
            if lighting.lut_rg.enable() && reflect_green.is_supported(lighting_config) {
                refl_value_g = self.scaled_lut_value(
                    &context,
                    reflect_green.value(light_num),
                    light_num,
                    lighting.lut_rg,
                );
            }

            let mut refl_value_b = refl_value_r;
            let reflect_blue = pica::ReflectBlue;
            if lighting.lut_rb.enable() && reflect_blue.is_supported(lighting_config) {
                refl_value_b = self.scaled_lut_value(
                    &context,
                    reflect_blue.value(light_num),
                    light_num,
                    lighting.lut_rb,
                );
            }

            let mut d1_lut_value = self.const_f32(1.0);
            let distribution_1 = pica::Distribution1;
            if lighting.lut_d1.enable() && distribution_1.is_supported(lighting_config) {
                d1_lut_value = self.scaled_lut_value(
                    &context,
                    distribution_1.value(light_num),
                    light_num,
                    lighting.lut_d1,
                );
            }

            let refl_value =
                self.op_composite_construct(vec3, &[refl_value_r, refl_value_g, refl_value_b]);
            let light_specular_1 = self.get_light_member(light_index, uniform.light_specular_1);
            let refl_times_d1 = self.op_vector_times_scalar(vec3, refl_value, d1_lut_value);
            let mut specular_1 = self.op_f_mul(vec3, refl_times_d1, light_specular_1);
            if light_config.geometric_factor_1() {
                specular_1 = self.op_vector_times_scalar(vec3, specular_1, geo_factor);
            }

            let fresnel = pica::Fresnel;
            if light_index == lighting.src_num() as usize - 1
                && lighting.lut_fr.enable()
                && fresnel.is_supported(lighting_config)
            {
                let value = self.scaled_lut_value(
                    &context,
                    fresnel.value(light_num),
                    light_num,
                    lighting.lut_fr,
                );

                if lighting.enable_primary_alpha() {
                    diffuse_sum = self.op_composite_insert(vec4, value, diffuse_sum, &[&3i32]);
                }

                if lighting.enable_secondary_alpha() {
                    specular_sum = self.op_composite_insert(vec4, value, specular_sum, &[&3i32]);
                }
            }

            let shadow_primary_enable = lighting.shadow_primary() && light_config.shadow_enable();
            let shadow_secondary_enable =
                lighting.shadow_secondary() && light_config.shadow_enable();
            let shadow_rgb = self.vector_shuffle(vec3, shadow, shadow, &[0, 1, 2]);

            let light_diffuse = self.get_light_member(light_index, uniform.light_diffuse);
            let light_ambient = self.get_light_member(light_index, uniform.light_ambient);
            let diffuse_mul_dot = self.op_vector_times_scalar(vec3, light_diffuse, dot_product);

            let mut diffuse_sum_rgb = self.op_f_add(vec3, diffuse_mul_dot, light_ambient);
            diffuse_sum_rgb = self.op_vector_times_scalar(vec3, diffuse_sum_rgb, dist_atten);
            diffuse_sum_rgb = self.op_vector_times_scalar(vec3, diffuse_sum_rgb, spot_atten);
            if shadow_primary_enable {
                diffuse_sum_rgb = self.op_f_mul(vec3, diffuse_sum_rgb, shadow_rgb);
            }

            let specular_01 = self.op_f_add(vec3, specular_0, specular_1);
            let mut specular_sum_rgb =
                self.op_vector_times_scalar(vec3, specular_01, clamp_highlights);
            specular_sum_rgb = self.op_vector_times_scalar(vec3, specular_sum_rgb, dist_atten);
            specular_sum_rgb = self.op_vector_times_scalar(vec3, specular_sum_rgb, spot_atten);
            if shadow_secondary_enable {
                specular_sum_rgb = self.op_f_mul(vec3, specular_sum_rgb, shadow_rgb);
            }

            let diffuse_sum_rgba = self.pad_vector_f32(diffuse_sum_rgb, vec4, 0.0);
            let specular_sum_rgba = self.pad_vector_f32(specular_sum_rgb, vec4, 0.0);
            diffuse_sum = self.op_f_add(vec4, diffuse_sum, diffuse_sum_rgba);
            specular_sum = self.op_f_add(vec4, specular_sum, specular_sum_rgba);
        }

        if lighting.shadow_alpha() {
            let shadow_a = self.composite_extract(f32_id, shadow, 3);
            let ones = self.const_f32s(&[1.0, 1.0, 1.0]);
            let shadow_a_vec = self.op_composite_construct(vec4, &[ones, shadow_a]);
            if lighting.enable_primary_alpha() {
                diffuse_sum = self.op_f_mul(vec4, diffuse_sum, shadow_a_vec);
            }
            if lighting.enable_secondary_alpha() {
                specular_sum = self.op_f_mul(vec4, specular_sum, shadow_a_vec);
            }
        }

        let member = self.const_s32(uniform.lighting_global_ambient);
        let lighting_global_ambient = self.get_shader_data_member(vec3, &[member]);
        let lighting_global_ambient_rgba = self.pad_vector_f32(lighting_global_ambient, vec4, 0.0);
        let zero_vec = self.const_f32s(&[0.0, 0.0, 0.0, 0.0]);
        let one_vec = self.const_f32s(&[1.0, 1.0, 1.0, 1.0]);
        diffuse_sum = self.op_f_add(vec4, diffuse_sum, lighting_global_ambient_rgba);
        self.primary_fragment_color =
            self.glsl::<glsl::FClamp>(vec4, &[diffuse_sum, zero_vec, one_vec]);
        self.secondary_fragment_color =
            self.glsl::<glsl::FClamp>(vec4, &[specular_sum, zero_vec, one_vec]);
    }

    fn lookup_lighting_lut(&mut self, lut_index: Id, index: Id, delta: Id) -> Id {
        let (f32_id, i32_id, vec4) = (self.f32_id, self.i32_id, self.vec_ids.get(4));
        let settings = self.settings.lighting.clone();
        let lighting_lut = self.settings.uniform.lighting_lut;

        if self.texture_buffer_lut_lf.value == 0 {
            let (image_buffer_id, lut_id) = (self.image_buffer_id, self.texture_buffer_lut_lf_id);
            self.texture_buffer_lut_lf = self.op_load(image_buffer_id, lut_id, None);
        }

        let shift = self.const_s32(settings.lut_group_shift);
        let lut_index_x = self.op_shift_right_arithmetic(i32_id, lut_index, shift);
        let mask = self.const_s32(settings.lut_group_mask);
        let lut_index_y = self.op_bitwise_and(i32_id, lut_index, mask);
        let member = self.const_s32(lighting_lut);
        let lut_offset = self.get_shader_data_member(i32_id, &[member, lut_index_x, lut_index_y]);
        let coord = self.op_i_add(i32_id, lut_offset, index);
        let lut_image = self.texture_buffer_lut_lf;
        let entry = self.op_image_fetch(vec4, lut_image, coord, None, &[]);
        let entry_r = self.composite_extract(f32_id, entry, 0);
        let entry_g = self.composite_extract(f32_id, entry, 1);
        self.glsl::<glsl::Fma>(f32_id, &[entry_g, delta, entry_r])
    }
}
