use emu_pica_types::regs::lighting::{
    LightingBumpMode, LightingConfig, LightingLutInput, LightingSampler,
    distance_attenuation_sampler, is_lighting_sampler_supported, spotlight_attenuation_sampler,
};
use rspirv::spirv::StorageClass;

use super::FragmentModule;
use crate::spv_module::{Id, valid_id};

/// The vectors that the lighting LUT inputs are computed from
#[derive(Clone, Copy)]
struct LutInputs {
    normal: Id,
    tangent: Id,
    view: Id,
    half_vector: Id,
    light_vector: Id,
    spot_dir: Id,
}

impl FragmentModule<'_> {
    /// Writes the code to emulate fragment lighting
    pub(super) fn write_lighting(&mut self) {
        let config = self.config;
        if config.lighting.enable() == 0 {
            return;
        }

        let lighting = &config.lighting;
        let f32_id = self.f32_id;
        let vec3 = self.vec_ids.get(3);
        let vec4 = self.vec_ids.get(4);

        // Define lighting globals
        let mut diffuse_sum = self.const_f32_vec(&[0.0, 0.0, 0.0, 1.0]);
        let mut specular_sum = self.const_f32_vec(&[0.0, 0.0, 0.0, 1.0]);
        let mut light_vector;
        let mut light_distance;
        let mut spot_dir;
        let mut half_vector;
        let mut dot_product;
        let mut clamp_highlights = self.const_f32(1.0);
        let mut geo_factor = self.const_f32(1.0);
        let mut surface_normal;
        let surface_tangent;

        // Compute fragment normals and tangents
        if lighting.bump_mode() == LightingBumpMode::NORMAL_MAP {
            // Bump mapping is enabled using a normal map
            surface_normal = self.perturbation();

            // Recompute Z-component of perturbation if 'renorm' is enabled, this provides a higher
            // precision result
            if lighting.bump_renorm() != 0 {
                let normal_x = self.m.op_composite_extract(f32_id, surface_normal, 0);
                let normal_y = self.m.op_composite_extract(f32_id, surface_normal, 1);
                let y_mul_y = self.m.op_f_mul(f32_id, normal_y, normal_y);
                let one = self.const_f32(1.0);
                let x_mul_x_plus_y_mul_y = self.m.op_fma(f32_id, normal_x, normal_x, y_mul_y);
                let val = self.m.op_f_sub(f32_id, one, x_mul_x_plus_y_mul_y);
                let zero = self.const_f32(0.0);
                let val_max = self.m.op_f_max(f32_id, val, zero);
                let normal_z = self.m.op_sqrt(f32_id, val_max);
                surface_normal = self
                    .m
                    .op_composite_construct(vec3, &[normal_x, normal_y, normal_z]);
            }

            // The tangent vector is not perturbed by the normal map and is just a unit vector.
            surface_tangent = self.const_f32_vec(&[1.0, 0.0, 0.0]);
        } else if lighting.bump_mode() == LightingBumpMode::TANGENT_MAP {
            // Bump mapping is enabled using a tangent map
            surface_tangent = self.perturbation();

            // Mathematically, recomputing Z-component of the tangent vector won't affect the relevant
            // computation below, which is also confirmed on 3DS. So we don't bother recomputing here
            // even if 'renorm' is enabled.

            // The normal vector is not perturbed by the tangent map and is just a unit vector.
            surface_normal = self.const_f32_vec(&[0.0, 0.0, 1.0]);
        } else {
            // No bump mapping - surface local normal and tangent are just unit vectors
            surface_normal = self.const_f32_vec(&[0.0, 0.0, 1.0]);
            surface_tangent = self.const_f32_vec(&[1.0, 0.0, 0.0]);
        }

        // Perform quaternion correction in the fragment shader if fragment_shader_barycentric is
        // supported.
        let normquat = if self.use_fragment_shader_barycentric {
            let input_pointer_id = self.m.type_pointer(StorageClass::Input, vec4);
            let index = self.const_s32(0);
            let normquat_0_ptr =
                self.m
                    .op_access_chain(input_pointer_id, self.normquat_id, &[index]);
            let normquat_0 = self.m.op_load(vec4, normquat_0_ptr);
            let index = self.const_s32(1);
            let normquat_1_ptr =
                self.m
                    .op_access_chain(input_pointer_id, self.normquat_id, &[index]);
            let normquat_1 = self.m.op_load(vec4, normquat_1_ptr);
            let opposite = self.are_quaternions_opposite(normquat_0, normquat_1);
            let negated = self.m.op_f_negate(vec4, normquat_1);
            let normquat_1_correct = self.m.op_select(vec4, opposite, negated, normquat_1);
            let index = self.const_s32(2);
            let normquat_2_ptr =
                self.m
                    .op_access_chain(input_pointer_id, self.normquat_id, &[index]);
            let normquat_2 = self.m.op_load(vec4, normquat_2_ptr);
            let opposite = self.are_quaternions_opposite(normquat_0, normquat_2);
            let negated = self.m.op_f_negate(vec4, normquat_2);
            let normquat_2_correct = self.m.op_select(vec4, opposite, negated, normquat_2);
            let bary_coord = self.m.op_load(vec3, self.gl_bary_coord_id);
            let bary_coord_x = self.m.op_composite_extract(f32_id, bary_coord, 0);
            let bary_coord_y = self.m.op_composite_extract(f32_id, bary_coord, 1);
            let bary_coord_z = self.m.op_composite_extract(f32_id, bary_coord, 2);
            let normquat_0_final = self
                .m
                .op_vector_times_scalar(vec4, normquat_0, bary_coord_x);
            let normquat_1_final =
                self.m
                    .op_vector_times_scalar(vec4, normquat_1_correct, bary_coord_y);
            let normquat_2_final =
                self.m
                    .op_vector_times_scalar(vec4, normquat_2_correct, bary_coord_z);
            let normquat_12 = self.m.op_f_add(vec4, normquat_1_final, normquat_2_final);
            self.m.op_f_add(vec4, normquat_0_final, normquat_12)
        } else {
            self.m.op_load(vec4, self.normquat_id)
        };

        // Rotate the surface-local normal by the interpolated normal quaternion to convert it to
        // eyespace.
        let normalized_normquat = self.m.op_normalize(vec4, normquat);
        let normal = self.quaternion_rotate(normalized_normquat, surface_normal);
        let tangent = self.quaternion_rotate(normalized_normquat, surface_tangent);

        let mut shadow = self.const_f32_vec(&[1.0, 1.0, 1.0, 1.0]);
        if lighting.enable_shadow() != 0 {
            shadow = self.m.op_function_call(
                vec4,
                self.sample_tex_unit_func[lighting.shadow_selector() as usize],
            );
            if lighting.shadow_invert() != 0 {
                let one_vec = self.const_f32_vec(&[1.0, 1.0, 1.0, 1.0]);
                shadow = self.m.op_f_sub(vec4, one_vec, shadow);
            }
        }

        // Samples the specified lookup table for specular lighting
        let view = self.m.op_load(vec3, self.view_id);

        // Write the code to emulate each enabled light
        for light_index in 0..lighting.src_num() {
            let light_config = &lighting.lights[light_index as usize];
            let light_num = u32::from(light_config.num());

            // Compute light vector (directional or positional)
            let light_position = self.get_light_member(light_num, 4);
            if light_config.directional() != 0 {
                light_vector = light_position;
            } else {
                light_vector = self.m.op_f_add(vec3, light_position, view);
            }

            light_distance = self.m.op_length(f32_id, light_vector);
            light_vector = self.m.op_normalize(vec3, light_vector);

            spot_dir = self.get_light_member(light_num, 5);
            let normalized_view = self.m.op_normalize(vec3, view);
            half_vector = self.m.op_f_add(vec3, normalized_view, light_vector);

            // Compute dot product of light_vector and normal, adjust if lighting is one-sided or
            // two-sided
            let light_dot_normal = self.m.op_dot(f32_id, light_vector, normal);
            if light_config.two_sided_diffuse() != 0 {
                dot_product = self.m.op_f_abs(f32_id, light_dot_normal);
            } else {
                let zero = self.const_f32(0.0);
                dot_product = self.m.op_f_max(f32_id, light_dot_normal, zero);
            }

            // If enabled, clamp specular component if lighting result is zero
            if lighting.clamp_highlights() != 0 {
                clamp_highlights = self.m.op_f_sign(f32_id, dot_product);
            }

            let inputs = LutInputs {
                normal,
                tangent,
                view,
                half_vector,
                light_vector,
                spot_dir,
            };

            // If enabled, compute spot light attenuation value
            let mut spot_atten = self.const_f32(1.0);
            if light_config.spot_atten_enable() != 0
                && is_lighting_sampler_supported(
                    lighting.config(),
                    LightingSampler::SPOTLIGHT_ATTENUATION,
                )
            {
                let value = self.get_lut_value(
                    &inputs,
                    spotlight_attenuation_sampler(light_num),
                    light_num,
                    lighting.lut_sp.ty(),
                    lighting.lut_sp.abs_input() != 0,
                );
                let scale = self.const_f32(lighting.lut_sp.scale);
                spot_atten = self.m.op_f_mul(f32_id, scale, value);
            }

            // If enabled, compute distance attenuation value
            let mut dist_atten = self.const_f32(1.0);
            if light_config.dist_atten_enable() != 0 {
                let dist_atten_scale = self.get_light_member(light_num, 7);
                let dist_atten_bias = self.get_light_member(light_num, 6);
                let index =
                    self.m
                        .op_fma(f32_id, dist_atten_scale, light_distance, dist_atten_bias);
                let zero = self.const_f32(0.0);
                let one = self.const_f32(1.0);
                let clamped_index = self.m.op_f_clamp(f32_id, index, zero, one);
                let sampler = self.const_s32(distance_attenuation_sampler(light_num).0 as i32);
                dist_atten = self.lookup_lighting_lut_unsigned(sampler, clamped_index);
            }

            if light_config.geometric_factor_0() != 0 || light_config.geometric_factor_1() != 0 {
                geo_factor = self.m.op_dot(f32_id, half_vector, half_vector);
                let dot_div_geo = self.m.op_f_div(f32_id, dot_product, geo_factor);
                let one = self.const_f32(1.0);
                let dot_div_geo = self.m.op_f_min(f32_id, dot_div_geo, one);
                let zero = self.const_f32(0.0);
                let is_geo_factor_zero = self.m.op_f_ord_equal(self.bool_id, geo_factor, zero);
                let zero = self.const_f32(0.0);
                geo_factor = self
                    .m
                    .op_select(f32_id, is_geo_factor_zero, zero, dot_div_geo);
            }

            // Specular 0 component
            let mut d0_lut_value = self.const_f32(1.0);
            if lighting.lut_d0.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::DISTRIBUTION0)
            {
                // Lookup specular "distribution 0" LUT value
                let value = self.get_lut_value(
                    &inputs,
                    LightingSampler::DISTRIBUTION0,
                    light_num,
                    lighting.lut_d0.ty(),
                    lighting.lut_d0.abs_input() != 0,
                );
                let scale = self.const_f32(lighting.lut_d0.scale);
                d0_lut_value = self.m.op_f_mul(f32_id, scale, value);
            }

            let light_specular_0 = self.get_light_member(light_num, 0);
            let mut specular_0 =
                self.m
                    .op_vector_times_scalar(vec3, light_specular_0, d0_lut_value);
            if light_config.geometric_factor_0() != 0 {
                specular_0 = self.m.op_vector_times_scalar(vec3, specular_0, geo_factor);
            }

            // If enabled, lookup ReflectRed value, otherwise, 1.0 is used
            let mut refl_value_r = self.const_f32(1.0);
            if lighting.lut_rr.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::REFLECT_RED)
            {
                let value = self.get_lut_value(
                    &inputs,
                    LightingSampler::REFLECT_RED,
                    light_num,
                    lighting.lut_rr.ty(),
                    lighting.lut_rr.abs_input() != 0,
                );

                let scale = self.const_f32(lighting.lut_rr.scale);
                refl_value_r = self.m.op_f_mul(f32_id, scale, value);
            }

            // If enabled, lookup ReflectGreen value, otherwise, ReflectRed value is used
            let mut refl_value_g = refl_value_r;
            if lighting.lut_rg.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::REFLECT_GREEN)
            {
                let value = self.get_lut_value(
                    &inputs,
                    LightingSampler::REFLECT_GREEN,
                    light_num,
                    lighting.lut_rg.ty(),
                    lighting.lut_rg.abs_input() != 0,
                );

                let scale = self.const_f32(lighting.lut_rg.scale);
                refl_value_g = self.m.op_f_mul(f32_id, scale, value);
            }

            // If enabled, lookup ReflectBlue value, otherwise, ReflectRed value is used
            let mut refl_value_b = refl_value_r;
            if lighting.lut_rb.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::REFLECT_BLUE)
            {
                let value = self.get_lut_value(
                    &inputs,
                    LightingSampler::REFLECT_BLUE,
                    light_num,
                    lighting.lut_rb.ty(),
                    lighting.lut_rb.abs_input() != 0,
                );
                let scale = self.const_f32(lighting.lut_rb.scale);
                refl_value_b = self.m.op_f_mul(f32_id, scale, value);
            }

            // Specular 1 component
            let mut d1_lut_value = self.const_f32(1.0);
            if lighting.lut_d1.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::DISTRIBUTION1)
            {
                // Lookup specular "distribution 1" LUT value
                let value = self.get_lut_value(
                    &inputs,
                    LightingSampler::DISTRIBUTION1,
                    light_num,
                    lighting.lut_d1.ty(),
                    lighting.lut_d1.abs_input() != 0,
                );
                let scale = self.const_f32(lighting.lut_d1.scale);
                d1_lut_value = self.m.op_f_mul(f32_id, scale, value);
            }

            let refl_value = self
                .m
                .op_composite_construct(vec3, &[refl_value_r, refl_value_g, refl_value_b]);
            let light_specular_1 = self.get_light_member(light_num, 1);
            let refl_mul_d1 = self
                .m
                .op_vector_times_scalar(vec3, refl_value, d1_lut_value);
            let mut specular_1 = self.m.op_f_mul(vec3, refl_mul_d1, light_specular_1);
            if light_config.geometric_factor_1() != 0 {
                specular_1 = self.m.op_vector_times_scalar(vec3, specular_1, geo_factor);
            }

            // Fresnel
            // Note: only the last entry in the light slots applies the Fresnel factor
            if light_index == lighting.src_num() - 1
                && lighting.lut_fr.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::FRESNEL)
            {
                // Lookup fresnel LUT value
                let mut value = self.get_lut_value(
                    &inputs,
                    LightingSampler::FRESNEL,
                    light_num,
                    lighting.lut_fr.ty(),
                    lighting.lut_fr.abs_input() != 0,
                );
                let scale = self.const_f32(lighting.lut_fr.scale);
                value = self.m.op_f_mul(f32_id, scale, value);

                // Enabled for diffuse lighting alpha component
                if lighting.enable_primary_alpha() != 0 {
                    diffuse_sum = self.m.op_composite_insert(vec4, value, diffuse_sum, 3);
                }

                // Enabled for the specular lighting alpha component
                if lighting.enable_secondary_alpha() != 0 {
                    specular_sum = self.m.op_composite_insert(vec4, value, specular_sum, 3);
                }
            }

            let shadow_primary_enable =
                lighting.shadow_primary() != 0 && light_config.shadow_enable() != 0;
            let shadow_secondary_enable =
                lighting.shadow_secondary() != 0 && light_config.shadow_enable() != 0;
            let shadow_rgb = self.m.op_vector_shuffle(vec3, shadow, shadow, &[0, 1, 2]);

            let light_diffuse = self.get_light_member(light_num, 2);
            let light_ambient = self.get_light_member(light_num, 3);
            let diffuse_mul_dot = self
                .m
                .op_vector_times_scalar(vec3, light_diffuse, dot_product);

            // Compute primary fragment color (diffuse lighting) function
            let mut diffuse_sum_rgb = self.m.op_f_add(vec3, diffuse_mul_dot, light_ambient);
            diffuse_sum_rgb = self
                .m
                .op_vector_times_scalar(vec3, diffuse_sum_rgb, dist_atten);
            diffuse_sum_rgb = self
                .m
                .op_vector_times_scalar(vec3, diffuse_sum_rgb, spot_atten);
            if shadow_primary_enable {
                diffuse_sum_rgb = self.m.op_f_mul(vec3, diffuse_sum_rgb, shadow_rgb);
            }

            // Compute secondary fragment color (specular lighting) function
            let specular_01 = self.m.op_f_add(vec3, specular_0, specular_1);
            let mut specular_sum_rgb =
                self.m
                    .op_vector_times_scalar(vec3, specular_01, clamp_highlights);
            specular_sum_rgb = self
                .m
                .op_vector_times_scalar(vec3, specular_sum_rgb, dist_atten);
            specular_sum_rgb = self
                .m
                .op_vector_times_scalar(vec3, specular_sum_rgb, spot_atten);
            if shadow_secondary_enable {
                specular_sum_rgb = self.m.op_f_mul(vec3, specular_sum_rgb, shadow_rgb);
            }

            // Accumulate the fragment colors
            let diffuse_sum_rgba = self.pad_vector_f32(diffuse_sum_rgb, vec4, 0.0);
            let specular_sum_rgba = self.pad_vector_f32(specular_sum_rgb, vec4, 0.0);
            diffuse_sum = self.m.op_f_add(vec4, diffuse_sum, diffuse_sum_rgba);
            specular_sum = self.m.op_f_add(vec4, specular_sum, specular_sum_rgba);
        }

        // Apply shadow attenuation to alpha components if enabled
        if lighting.shadow_alpha() != 0 {
            let shadow_a = self.m.op_composite_extract(f32_id, shadow, 3);
            let one_vec = self.const_f32_vec(&[1.0, 1.0, 1.0]);
            let shadow_a_vec = self.m.op_composite_construct(vec4, &[one_vec, shadow_a]);
            if lighting.enable_primary_alpha() != 0 {
                diffuse_sum = self.m.op_f_mul(vec4, diffuse_sum, shadow_a_vec);
            }
            if lighting.enable_secondary_alpha() != 0 {
                specular_sum = self.m.op_f_mul(vec4, specular_sum, shadow_a_vec);
            }
        }

        // Sum final lighting result
        let member = self.const_s32(23);
        let lighting_global_ambient = self.get_shader_data_member(vec3, &[member]);
        let lighting_global_ambient_rgba = self.pad_vector_f32(lighting_global_ambient, vec4, 0.0);
        let zero_vec = self.const_f32_vec(&[0.0, 0.0, 0.0, 0.0]);
        let one_vec = self.const_f32_vec(&[1.0, 1.0, 1.0, 1.0]);
        diffuse_sum = self
            .m
            .op_f_add(vec4, diffuse_sum, lighting_global_ambient_rgba);
        self.primary_fragment_color = self.m.op_f_clamp(vec4, diffuse_sum, zero_vec, one_vec);
        self.secondary_fragment_color = self.m.op_f_clamp(vec4, specular_sum, zero_vec, one_vec);
    }

    fn perturbation(&mut self) -> Id {
        let vec3 = self.vec_ids.get(3);
        let bump_selector = self.config.lighting.bump_selector() as usize;
        let texel = self.m.op_function_call(
            self.vec_ids.get(4),
            self.sample_tex_unit_func[bump_selector],
        );
        let texel_rgb = self.m.op_vector_shuffle(vec3, texel, texel, &[0, 1, 2]);
        let two = self.const_f32(2.0);
        let rgb_mul_two = self.m.op_vector_times_scalar(vec3, texel_rgb, two);
        let one_vec = self.const_f32_vec(&[1.0, 1.0, 1.0]);
        self.m.op_f_sub(vec3, rgb_mul_two, one_vec)
    }

    /// Rotate the vector v by the quaternion q
    fn quaternion_rotate(&mut self, q: Id, v: Id) -> Id {
        let vec3 = self.vec_ids.get(3);
        let q_xyz = self.m.op_vector_shuffle(vec3, q, q, &[0, 1, 2]);
        let q_xyz_cross_v = self.m.op_cross(vec3, q_xyz, v);
        let q_w = self.m.op_composite_extract(self.f32_id, q, 3);
        let v_mul_q_w = self.m.op_vector_times_scalar(vec3, v, q_w);
        let val1 = self.m.op_f_add(vec3, q_xyz_cross_v, v_mul_q_w);
        let q_xyz_cross_val1 = self.m.op_cross(vec3, q_xyz, val1);
        let two = self.const_f32(2.0);
        let val2 = self.m.op_vector_times_scalar(vec3, q_xyz_cross_val1, two);
        self.m.op_f_add(vec3, v, val2)
    }

    fn are_quaternions_opposite(&mut self, qa: Id, qb: Id) -> Id {
        let dot_q = self.m.op_dot(self.f32_id, qa, qb);
        let zero = self.const_f32(0.0);
        let dot_le_zero = self.m.op_f_ord_less_than(self.bool_id, dot_q, zero);
        self.m.op_composite_construct(
            self.bvec_ids.get(4),
            &[dot_le_zero, dot_le_zero, dot_le_zero, dot_le_zero],
        )
    }

    fn get_light_member(&mut self, light_num: u32, member: i32) -> Id {
        let member_type = if member < 6 {
            self.vec_ids.get(3)
        } else {
            self.f32_id
        };
        let light_src = self.const_s32(24);
        let light_num = self.const_s32(light_num as i32);
        let member = self.const_s32(member);
        self.get_shader_data_member(member_type, &[light_src, light_num, member])
    }

    fn lookup_lighting_lut_unsigned(&mut self, lut_index: Id, pos: Id) -> Id {
        let f32_id = self.f32_id;
        let scale = self.const_f32(256.0);
        let pos_scaled = self.m.op_f_mul(f32_id, pos, scale);
        let pos_floor = self.m.op_floor(f32_id, pos_scaled);
        let min = self.const_f32(0.0);
        let max = self.const_f32(255.0);
        let index_float = self.m.op_f_clamp(f32_id, pos_floor, min, max);
        let index = self.m.op_convert_f_to_s(self.i32_id, index_float);
        let index_f32 = self.m.op_convert_s_to_f(f32_id, index);
        let neg_index = self.m.op_f_negate(f32_id, index_f32);
        let scale = self.const_f32(256.0);
        let delta = self.m.op_fma(f32_id, pos, scale, neg_index);
        self.lookup_lighting_lut(lut_index, index, delta)
    }

    fn lookup_lighting_lut_signed(&mut self, lut_index: Id, pos: Id) -> Id {
        let f32_id = self.f32_id;
        let i32_id = self.i32_id;
        let scale = self.const_f32(128.0);
        let pos_scaled = self.m.op_f_mul(f32_id, pos, scale);
        let pos_floor = self.m.op_floor(f32_id, pos_scaled);
        let min = self.const_f32(-128.0);
        let max = self.const_f32(127.0);
        let index_float = self.m.op_f_clamp(f32_id, pos_floor, min, max);
        let index = self.m.op_convert_f_to_s(i32_id, index_float);
        let index_f32 = self.m.op_convert_s_to_f(f32_id, index);
        let neg_index = self.m.op_f_negate(f32_id, index_f32);
        let scale = self.const_f32(128.0);
        let delta = self.m.op_fma(f32_id, pos, scale, neg_index);
        let zero = self.const_s32(0);
        let is_negative = self.m.op_s_less_than(self.bool_id, index, zero);
        let wrap = self.const_s32(256);
        let zero = self.const_s32(0);
        let increment = self.m.op_select(i32_id, is_negative, wrap, zero);
        let wrapped_index = self.m.op_i_add(i32_id, index, increment);
        self.lookup_lighting_lut(lut_index, wrapped_index, delta)
    }

    /// Samples the specified lookup table for specular lighting
    fn get_lut_value(
        &mut self,
        inputs: &LutInputs,
        sampler: LightingSampler,
        light_num: u32,
        input: LightingLutInput,
        abs: bool,
    ) -> Id {
        let f32_id = self.f32_id;
        let vec3 = self.vec_ids.get(3);
        let mut index = match input {
            LightingLutInput::NH => {
                let normalized_half_vector = self.m.op_normalize(vec3, inputs.half_vector);
                self.m.op_dot(f32_id, inputs.normal, normalized_half_vector)
            }
            LightingLutInput::VH => {
                let normalized_view = self.m.op_normalize(vec3, inputs.view);
                let normalized_half_vector = self.m.op_normalize(vec3, inputs.half_vector);
                self.m
                    .op_dot(f32_id, normalized_view, normalized_half_vector)
            }
            LightingLutInput::NV => {
                let normalized_view = self.m.op_normalize(vec3, inputs.view);
                self.m.op_dot(f32_id, inputs.normal, normalized_view)
            }
            LightingLutInput::LN => self.m.op_dot(f32_id, inputs.light_vector, inputs.normal),
            LightingLutInput::SP => self.m.op_dot(f32_id, inputs.light_vector, inputs.spot_dir),
            LightingLutInput::CP => {
                // CP input is only available with configuration 7
                if self.config.lighting.config() == LightingConfig::CONFIG7 {
                    // Note: even if the normal vector is modified by normal map, which is not the
                    // normal of the tangent plane anymore, the half angle vector is still projected
                    // using the modified normal vector.
                    let normalized_half_vector = self.m.op_normalize(vec3, inputs.half_vector);
                    let normal_dot_half_vector =
                        self.m.op_dot(f32_id, inputs.normal, normalized_half_vector);
                    let normal_mul_dot =
                        self.m
                            .op_vector_times_scalar(vec3, inputs.normal, normal_dot_half_vector);
                    let half_angle_proj =
                        self.m
                            .op_f_sub(vec3, normalized_half_vector, normal_mul_dot);

                    // Note: the half angle vector projection is confirmed not normalized before the dot
                    // product. The result is in fact not cos(phi) as the name suggested.
                    self.m.op_dot(f32_id, half_angle_proj, inputs.tangent)
                } else {
                    self.const_f32(0.0)
                }
            }
            _ => {
                log::error!(target: "HW_GPU", "Unknown lighting LUT input {}", input.0 as i32);
                log::error!(target: "Debug", "Unimplemented code!");
                self.const_f32(0.0)
            }
        };

        let sampler_index = self.const_u32(sampler.0);
        if abs {
            // LUT index is in the range of (0.0, 1.0)
            index = if self.config.lighting.lights[light_num as usize].two_sided_diffuse() != 0 {
                self.m.op_f_abs(f32_id, index)
            } else {
                let zero = self.const_f32(0.0);
                self.m.op_f_max(f32_id, index, zero)
            };
            self.lookup_lighting_lut_unsigned(sampler_index, index)
        } else {
            // LUT index is in the range of (-1.0, 1.0)
            self.lookup_lighting_lut_signed(sampler_index, index)
        }
    }

    /// Lookups the lighting LUT at the provided lut_index
    fn lookup_lighting_lut(&mut self, lut_index: Id, index: Id, delta: Id) -> Id {
        // Only load the texture buffer lut once
        if !valid_id(self.texture_buffer_lut_lf) {
            self.texture_buffer_lut_lf = self
                .m
                .op_load(self.image_buffer_id, self.texture_buffer_lut_lf_id);
        }

        let i32_id = self.i32_id;
        let two = self.const_s32(2);
        let lut_index_x = self.m.op_shift_right_arithmetic(i32_id, lut_index, two);
        let three = self.const_s32(3);
        let lut_index_y = self.m.op_bitwise_and(i32_id, lut_index, three);
        let member = self.const_s32(18);
        let lut_offset = self.get_shader_data_member(i32_id, &[member, lut_index_x, lut_index_y]);
        let coord = self.m.op_i_add(i32_id, lut_offset, index);
        let entry = self
            .m
            .op_image_fetch(self.vec_ids.get(4), self.texture_buffer_lut_lf, coord);
        let entry_r = self.m.op_composite_extract(self.f32_id, entry, 0);
        let entry_g = self.m.op_composite_extract(self.f32_id, entry, 1);
        self.m.op_fma(self.f32_id, entry_g, delta, entry_r)
    }
}
