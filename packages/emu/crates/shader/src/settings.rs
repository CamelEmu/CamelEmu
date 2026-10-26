#[derive(Debug, Clone, PartialEq)]
pub struct FragmentSettings {
    pub spirv_version: u32,
    pub num_tev_stages: u32,
    pub num_lights: u32,
    pub num_lighting_samplers: u32,
    pub num_tex_units: u32,
    pub num_non_proc_tex_units: u32,
    pub interface: InterfaceSettings,
    pub uniform: UniformLayout,
    pub color: ColorSettings,
    pub fog: FogSettings,
    pub lighting: LightingSettings,
    pub shadow: ShadowSettings,
    pub proctex: ProcTexSettings,
}

impl Default for FragmentSettings {
    fn default() -> Self {
        Self {
            spirv_version: 0x0001_0300,
            num_tev_stages: 6,
            num_lights: 8,
            num_lighting_samplers: 24,
            num_tex_units: 4,
            num_non_proc_tex_units: 3,
            interface: InterfaceSettings::default(),
            uniform: UniformLayout::default(),
            color: ColorSettings::default(),
            fog: FogSettings::default(),
            lighting: LightingSettings::default(),
            shadow: ShadowSettings::default(),
            proctex: ProcTexSettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub set: u32,
    pub binding: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceSettings {
    pub primary_color_location: u32,
    pub texcoord_locations: [u32; 3],
    pub texcoord0_w_location: u32,
    pub normquat_location: u32,
    pub view_location: u32,
    pub color_location: u32,
    pub normquat_vertices: u32,
    pub lut_lf: Binding,
    pub lut_rg: Binding,
    pub lut_rgba: Binding,
    pub tex0: Binding,
    pub tex1: Binding,
    pub tex2: Binding,
    pub shadow_texture_px: Binding,
    pub shader_data: Binding,
    pub image_sampled: u32,
    pub image_storage: u32,
}

impl Default for InterfaceSettings {
    fn default() -> Self {
        Self {
            primary_color_location: 1,
            texcoord_locations: [2, 3, 4],
            texcoord0_w_location: 5,
            normquat_location: 6,
            view_location: 7,
            color_location: 0,
            normquat_vertices: 3,
            lut_lf: Binding { set: 0, binding: 3 },
            lut_rg: Binding { set: 0, binding: 4 },
            lut_rgba: Binding { set: 0, binding: 5 },
            tex0: Binding { set: 1, binding: 0 },
            tex1: Binding { set: 1, binding: 1 },
            tex2: Binding { set: 1, binding: 2 },
            shadow_texture_px: Binding { set: 2, binding: 0 },
            shader_data: Binding { set: 0, binding: 2 },
            image_sampled: 1,
            image_storage: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UniformLayout {
    pub alphatest_ref: i32,
    pub depth_scale: i32,
    pub depth_offset: i32,
    pub scissor_x1: i32,
    pub scissor_y1: i32,
    pub scissor_x2: i32,
    pub scissor_y2: i32,
    pub fog_lut_offset: i32,
    pub proctex_noise_lut_offset: i32,
    pub proctex_color_map_offset: i32,
    pub proctex_alpha_map_offset: i32,
    pub proctex_lut_offset: i32,
    pub proctex_diff_lut_offset: i32,
    pub proctex_bias: i32,
    pub shadow_texture_bias: i32,
    pub lighting_lut: i32,
    pub fog_color: i32,
    pub proctex_noise_f: i32,
    pub proctex_noise_a: i32,
    pub proctex_noise_p: i32,
    pub lighting_global_ambient: i32,
    pub light_src: i32,
    pub const_color: i32,
    pub tev_combiner_buffer_color: i32,
    pub tex_lod_bias: i32,
    pub tex_border_color: i32,
    pub light_specular_0: i32,
    pub light_specular_1: i32,
    pub light_diffuse: i32,
    pub light_ambient: i32,
    pub light_position: i32,
    pub light_spot_direction: i32,
    pub light_dist_atten_bias: i32,
    pub light_dist_atten_scale: i32,
    pub light_vec3_members: i32,
    pub light_src_offsets: [u32; 8],
    pub shader_data_offsets: [u32; 30],
    pub lighting_lut_array_stride: u32,
    pub light_src_array_stride: u32,
    pub const_color_array_stride: u32,
    pub border_color_array_stride: u32,
}

impl Default for UniformLayout {
    fn default() -> Self {
        Self {
            alphatest_ref: 1,
            depth_scale: 2,
            depth_offset: 3,
            scissor_x1: 6,
            scissor_y1: 7,
            scissor_x2: 8,
            scissor_y2: 9,
            fog_lut_offset: 10,
            proctex_noise_lut_offset: 11,
            proctex_color_map_offset: 12,
            proctex_alpha_map_offset: 13,
            proctex_lut_offset: 14,
            proctex_diff_lut_offset: 15,
            proctex_bias: 16,
            shadow_texture_bias: 17,
            lighting_lut: 18,
            fog_color: 19,
            proctex_noise_f: 20,
            proctex_noise_a: 21,
            proctex_noise_p: 22,
            lighting_global_ambient: 23,
            light_src: 24,
            const_color: 25,
            tev_combiner_buffer_color: 26,
            tex_lod_bias: 27,
            tex_border_color: 28,
            light_specular_0: 0,
            light_specular_1: 1,
            light_diffuse: 2,
            light_ambient: 3,
            light_position: 4,
            light_spot_direction: 5,
            light_dist_atten_bias: 6,
            light_dist_atten_scale: 7,
            light_vec3_members: 6,
            light_src_offsets: [0, 16, 32, 48, 64, 80, 92, 96],
            shader_data_offsets: [
                0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 80, 176, 192,
                200, 208, 224, 240, 1136, 1232, 1248, 1264, 1312,
            ],
            lighting_lut_array_stride: 16,
            light_src_array_stride: 112,
            const_color_array_stride: 16,
            border_color_array_stride: 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorSettings {
    pub byte_scale: f32,
}

impl Default for ColorSettings {
    fn default() -> Self {
        Self { byte_scale: 255.0 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FogSettings {
    pub index_scale: f32,
    pub index_max: f32,
}

impl Default for FogSettings {
    fn default() -> Self {
        Self {
            index_scale: 128.0,
            index_max: 127.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LightingSettings {
    pub unsigned_scale: f32,
    pub unsigned_index_max: f32,
    pub signed_scale: f32,
    pub signed_index_min: f32,
    pub signed_index_max: f32,
    pub signed_negative_increment: i32,
    pub lut_group_shift: i32,
    pub lut_group_mask: i32,
}

impl Default for LightingSettings {
    fn default() -> Self {
        Self {
            unsigned_scale: 256.0,
            unsigned_index_max: 255.0,
            signed_scale: 128.0,
            signed_index_min: -128.0,
            signed_index_max: 127.0,
            signed_negative_increment: 256,
            lut_group_shift: 2,
            lut_group_mask: 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShadowSettings {
    pub depth_shift: i32,
    pub stencil_mask: u32,
    pub w_scale: f32,
    pub coord_offset: f32,
}

impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            depth_shift: 8,
            stencil_mask: 255,
            w_scale: 16_777_215.0,
            coord_offset: -0.5,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProcTexSettings {
    pub lut_index_max: f32,
    pub lut_scale: f32,
    pub lod_min_limit: f32,
    pub lod_max_limit: f32,
    pub noise_table_len: u32,
    pub offset_table_len: u32,
    pub noise1d_table: [i32; 16],
    pub noise2d_table: [i32; 16],
    pub fixed_lut_offsets: [i32; 4],
    pub noise_grid_scale: f32,
    pub noise_modulus: i32,
    pub noise_bias: i32,
    pub noise_multiplier: i32,
    pub noise_mask: i32,
    pub noise_mix_condition: i32,
    pub noise_mix_increment: i32,
    pub noise_xor_multiplier: i32,
    pub noise_add: i32,
    pub noise_result_scale: f32,
    pub noise_result_bias: f32,
    pub pulse_threshold: f32,
}

impl Default for ProcTexSettings {
    fn default() -> Self {
        Self {
            lut_index_max: 127.0,
            lut_scale: 128.0,
            lod_min_limit: 0.0,
            lod_max_limit: 7.0,
            noise_table_len: 16,
            offset_table_len: 8,
            noise1d_table: [0, 4, 10, 8, 4, 9, 7, 12, 5, 15, 13, 14, 11, 15, 2, 11],
            noise2d_table: [10, 2, 15, 8, 0, 7, 4, 5, 5, 13, 2, 6, 13, 9, 3, 14],
            fixed_lut_offsets: [0xF0, 0xF8, 0xFC, 0xFE],
            noise_grid_scale: 9.0,
            noise_modulus: 9,
            noise_bias: 2,
            noise_multiplier: 3,
            noise_mask: 0xF,
            noise_mix_condition: 1,
            noise_mix_increment: 4,
            noise_xor_multiplier: 6,
            noise_add: 10,
            noise_result_scale: 2.0 / 15.0,
            noise_result_bias: -1.0,
            pulse_threshold: 0.5,
        }
    }
}
