// Copyright 2023 Citra Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

mod definitions;
mod lighting;
mod proctex;
mod tev;
mod texture;

use emu_pica_types::regs::framebuffer::{CompareFunc, LogicOp};
use emu_pica_types::regs::rasterizer::{DepthBuffering, ScissorMode};
use emu_pica_types::regs::texturing::FogMode;
use rspirv::spirv::{Decoration, SelectionControl, StorageClass};

use crate::pica_fs_config::FSConfig;
use crate::profile::Profile;
use crate::spv_module::{Id, SpvModule, valid_id};

const SPIRV_VERSION_1_3: u32 = 0x00010300;

const NUM_TEV_STAGES: u32 = 6;
const NUM_LIGHTS: u32 = 8;
const NUM_LIGHTING_SAMPLERS: u32 = 24;
const NUM_TEX_UNITS: usize = 4;
const NUM_NON_PROC_TEX_UNITS: usize = 3;

#[derive(Clone, Copy, Default)]
struct VectorIds {
    ids: [Id; 3],
}

impl VectorIds {
    /// Returns the type id of the vector with the provided size
    fn get(&self, size: u32) -> Id {
        self.ids[size as usize - 2]
    }
}

pub struct FragmentModule<'a> {
    m: SpvModule,
    config: &'a FSConfig,

    use_fragment_shader_barycentric: bool,

    void_id: Id,
    bool_id: Id,
    f32_id: Id,
    i32_id: Id,
    u32_id: Id,

    vec_ids: VectorIds,
    ivec_ids: VectorIds,
    uvec_ids: VectorIds,
    bvec_ids: VectorIds,

    image2d_id: Id,
    image_cube_id: Id,
    image_buffer_id: Id,
    image_r32_id: Id,
    sampler_id: Id,
    shader_data_id: Id,

    primary_color_id: Id,
    texcoord_id: [Id; NUM_NON_PROC_TEX_UNITS],
    texcoord0_w_id: Id,
    normquat_id: Id,
    view_id: Id,
    color_id: Id,

    gl_frag_coord_id: Id,
    gl_frag_depth_id: Id,
    gl_bary_coord_id: Id,
    depth: Id,

    tex0_id: Id,
    tex1_id: Id,
    tex2_id: Id,
    texture_buffer_lut_lf_id: Id,
    texture_buffer_lut_rg_id: Id,
    texture_buffer_lut_rgba_id: Id,
    shadow_texture_px_id: Id,

    texture_buffer_lut_lf: Id,
    texture_buffer_lut_rg: Id,
    texture_buffer_lut_rgba: Id,

    rounded_primary_color: Id,
    primary_fragment_color: Id,
    secondary_fragment_color: Id,
    combiner_buffer: Id,
    next_combiner_buffer: Id,
    combiner_output: Id,

    color_results_1: Id,
    color_results_2: Id,
    color_results_3: Id,
    alpha_results_1: Id,
    alpha_results_2: Id,
    alpha_results_3: Id,

    sample_tex_unit_func: [Id; NUM_TEX_UNITS],
    noise1d_table: Id,
    noise2d_table: Id,
    lut_offsets: Id,
}

impl<'a> FragmentModule<'a> {
    pub fn new(config: &'a FSConfig, profile: &'a Profile) -> Self {
        let mut module = Self {
            m: SpvModule::new(SPIRV_VERSION_1_3),
            config,
            use_fragment_shader_barycentric: profile.has_fragment_shader_barycentric
                && config.lighting.enable() != 0,
            void_id: 0,
            bool_id: 0,
            f32_id: 0,
            i32_id: 0,
            u32_id: 0,
            vec_ids: VectorIds::default(),
            ivec_ids: VectorIds::default(),
            uvec_ids: VectorIds::default(),
            bvec_ids: VectorIds::default(),
            image2d_id: 0,
            image_cube_id: 0,
            image_buffer_id: 0,
            image_r32_id: 0,
            sampler_id: 0,
            shader_data_id: 0,
            primary_color_id: 0,
            texcoord_id: [0; NUM_NON_PROC_TEX_UNITS],
            texcoord0_w_id: 0,
            normquat_id: 0,
            view_id: 0,
            color_id: 0,
            gl_frag_coord_id: 0,
            gl_frag_depth_id: 0,
            gl_bary_coord_id: 0,
            depth: 0,
            tex0_id: 0,
            tex1_id: 0,
            tex2_id: 0,
            texture_buffer_lut_lf_id: 0,
            texture_buffer_lut_rg_id: 0,
            texture_buffer_lut_rgba_id: 0,
            shadow_texture_px_id: 0,
            texture_buffer_lut_lf: 0,
            texture_buffer_lut_rg: 0,
            texture_buffer_lut_rgba: 0,
            rounded_primary_color: 0,
            primary_fragment_color: 0,
            secondary_fragment_color: 0,
            combiner_buffer: 0,
            next_combiner_buffer: 0,
            combiner_output: 0,
            color_results_1: 0,
            color_results_2: 0,
            color_results_3: 0,
            alpha_results_1: 0,
            alpha_results_2: 0,
            alpha_results_3: 0,
            sample_tex_unit_func: [0; NUM_TEX_UNITS],
            noise1d_table: 0,
            noise2d_table: 0,
            lut_offsets: 0,
        };
        module.define_arithmetic_types();
        module.define_uniform_structs();
        module.define_interface();
        for i in 0..NUM_TEX_UNITS as u32 {
            module.define_tex_sampler(i);
        }
        module.define_entry_point();
        module
    }

    /// Emits SPIR-V bytecode corresponding to the provided pica fragment configuration
    pub fn generate(&mut self) {
        let label = self.m.op_label();
        self.m.add_label(label);

        let primary_color = self.m.op_load(self.vec_ids.get(4), self.primary_color_id);
        self.rounded_primary_color = self.byteround(primary_color, 4);
        self.primary_fragment_color = self.const_f32_vec(&[0.0, 0.0, 0.0, 0.0]);
        self.secondary_fragment_color = self.const_f32_vec(&[0.0, 0.0, 0.0, 0.0]);

        // Do not do any sort of processing if it's obvious we're not going to pass the alpha test
        if self.config.framebuffer.alpha_test_func() == CompareFunc::NEVER {
            self.m.op_kill();
            self.m.op_function_end();
            return;
        }

        // Append the scissor and depth tests
        self.write_depth();
        self.write_scissor();

        // Write shader bytecode to emulate all enabled PICA lights
        self.write_lighting();

        self.combiner_buffer = self.const_f32_vec(&[0.0, 0.0, 0.0, 0.0]);
        let member = self.const_s32(26);
        self.next_combiner_buffer = self.get_shader_data_member(self.vec_ids.get(4), &[member]);
        self.combiner_output = self.const_f32_vec(&[0.0, 0.0, 0.0, 0.0]);

        // Write shader bytecode to emulate PICA TEV stages
        for index in 0..self.config.texture.tev_stages.len() as i32 {
            self.write_tev_stage(index);
        }

        self.write_alpha_test_condition(self.config.framebuffer.alpha_test_func());

        // Emulate the fog
        match self.config.texture.fog_mode() {
            FogMode::FOG => self.write_fog(),
            FogMode::GAS => {
                self.write_gas();
                return;
            }
            _ => {}
        }

        let mut color = self.byteround(self.combiner_output, 4);
        let logic_op = self.config.framebuffer.logic_op();
        match logic_op {
            LogicOp::CLEAR => {
                color = self.const_f32_vec(&[0.0, 0.0, 0.0, 0.0]);
            }
            LogicOp::SET => {
                color = self.const_f32_vec(&[1.0, 1.0, 1.0, 1.0]);
            }
            LogicOp::COPY => {
                // Take the color output as-is
            }
            LogicOp::COPY_INVERTED => {
                // out += "color = ~color;\n";
            }
            LogicOp::NO_OP => {
                // We need to discard the color, but not necessarily the depth. This is not possible
                // with fragment shader alone, so we emulate this behavior with the color mask.
            }
            _ => {
                log::error!(target: "HW_GPU", "Unhandled logic_op {:x}", logic_op.0);
                log::error!(target: "Debug", "Unimplemented code!");
            }
        }

        // Write output color
        self.m.op_store(self.color_id, color);
        self.m.op_return();
        self.m.op_function_end();
    }

    pub fn assemble(self) -> Vec<u32> {
        self.m.assemble()
    }

    /// Undos the host perspective transformation and applies the PICA one
    fn write_depth(&mut self) {
        let input_pointer_id = self.m.type_pointer(StorageClass::Input, self.f32_id);
        let index = self.const_u32(2);
        let frag_coord_z_ptr =
            self.m
                .op_access_chain(input_pointer_id, self.gl_frag_coord_id, &[index]);
        let gl_frag_coord_z = self.m.op_load(self.f32_id, frag_coord_z_ptr);
        let z_over_w = self.m.op_f_negate(self.f32_id, gl_frag_coord_z);
        let member = self.const_s32(2);
        let depth_scale = self.get_shader_data_member(self.f32_id, &[member]);
        let member = self.const_s32(3);
        let depth_offset = self.get_shader_data_member(self.f32_id, &[member]);
        self.depth = self
            .m
            .op_fma(self.f32_id, z_over_w, depth_scale, depth_offset);
        if self.config.framebuffer.depthmap_enable() == DepthBuffering::W_BUFFERING {
            let index = self.const_u32(3);
            let frag_coord_w_ptr =
                self.m
                    .op_access_chain(input_pointer_id, self.gl_frag_coord_id, &[index]);
            let gl_frag_coord_w = self.m.op_load(self.f32_id, frag_coord_w_ptr);
            self.depth = self.m.op_f_div(self.f32_id, self.depth, gl_frag_coord_w);
        }
        self.m.op_store(self.gl_frag_depth_id, self.depth);
    }

    /// Emits code to emulate the scissor rectangle
    fn write_scissor(&mut self) {
        if self.config.framebuffer.scissor_test_mode() == ScissorMode::DISABLED {
            return;
        }

        let vec2 = self.vec_ids.get(2);
        let gl_frag_coord = self.m.op_load(self.vec_ids.get(4), self.gl_frag_coord_id);
        let gl_frag_coord_xy =
            self.m
                .op_vector_shuffle(vec2, gl_frag_coord, gl_frag_coord, &[0, 1]);

        let member = self.const_s32(6);
        let scissor_x1 = self.get_shader_data_member(self.i32_id, &[member]);
        let member = self.const_s32(7);
        let scissor_y1 = self.get_shader_data_member(self.i32_id, &[member]);
        let scissor_x1_f = self.m.op_convert_s_to_f(self.f32_id, scissor_x1);
        let scissor_y1_f = self.m.op_convert_s_to_f(self.f32_id, scissor_y1);
        let scissor_1 = self
            .m
            .op_composite_construct(vec2, &[scissor_x1_f, scissor_y1_f]);

        let member = self.const_s32(8);
        let scissor_x2 = self.get_shader_data_member(self.i32_id, &[member]);
        let member = self.const_s32(9);
        let scissor_y2 = self.get_shader_data_member(self.i32_id, &[member]);
        let scissor_x2_f = self.m.op_convert_s_to_f(self.f32_id, scissor_x2);
        let scissor_y2_f = self.m.op_convert_s_to_f(self.f32_id, scissor_y2);
        let scissor_2 = self
            .m
            .op_composite_construct(vec2, &[scissor_x2_f, scissor_y2_f]);

        let cond1 =
            self.m
                .op_f_ord_greater_than_equal(self.bvec_ids.get(2), gl_frag_coord_xy, scissor_1);
        let cond2 = self
            .m
            .op_f_ord_less_than(self.bvec_ids.get(2), gl_frag_coord_xy, scissor_2);

        let conds = self
            .m
            .op_composite_construct(self.bvec_ids.get(4), &[cond1, cond2]);
        let mut result = self.m.op_all(self.bool_id, conds);
        if self.config.framebuffer.scissor_test_mode() == ScissorMode::INCLUDE {
            result = self.m.op_logical_not(self.bool_id, result);
        }

        let merge_block = self.m.op_label();
        let kill_label = self.m.op_label();
        self.m
            .op_selection_merge(merge_block, SelectionControl::NONE);
        self.m
            .op_branch_conditional(result, kill_label, merge_block);

        self.m.add_label(kill_label);
        self.m.op_kill();

        self.m.add_label(merge_block);
    }

    /// Writes the code to emulate fog
    fn write_fog(&mut self) {
        // Get index into fog LUT
        let fog_index = if self.config.texture.fog_flip() != 0 {
            let one = self.const_f32(1.0);
            let one_minus_depth = self.m.op_f_sub(self.f32_id, one, self.depth);
            let scale = self.const_f32(128.0);
            self.m.op_f_mul(self.f32_id, one_minus_depth, scale)
        } else {
            let scale = self.const_f32(128.0);
            self.m.op_f_mul(self.f32_id, self.depth, scale)
        };

        // Generate clamped fog factor from LUT for given fog index
        let fog_index_floor = self.m.op_floor(self.f32_id, fog_index);
        let zero = self.const_f32(0.0);
        let max = self.const_f32(127.0);
        let fog_i = self.m.op_f_clamp(self.f32_id, fog_index_floor, zero, max);
        let fog_f = self.m.op_f_sub(self.f32_id, fog_index, fog_i);
        let member = self.const_s32(10);
        let fog_lut_offset = self.get_shader_data_member(self.i32_id, &[member]);
        let fog_i_s32 = self.m.op_convert_f_to_s(self.i32_id, fog_i);
        let coord = self.m.op_i_add(self.i32_id, fog_i_s32, fog_lut_offset);
        if !valid_id(self.texture_buffer_lut_lf) {
            self.texture_buffer_lut_lf = self
                .m
                .op_load(self.image_buffer_id, self.texture_buffer_lut_lf_id);
        }
        let fog_lut_entry_rgba =
            self.m
                .op_image_fetch(self.vec_ids.get(4), self.texture_buffer_lut_lf, coord);
        let fog_lut_r = self
            .m
            .op_composite_extract(self.f32_id, fog_lut_entry_rgba, 0);
        let fog_lut_g = self
            .m
            .op_composite_extract(self.f32_id, fog_lut_entry_rgba, 1);
        let mut fog_factor = self.m.op_fma(self.f32_id, fog_f, fog_lut_g, fog_lut_r);
        let zero = self.const_f32(0.0);
        let one = self.const_f32(1.0);
        fog_factor = self.m.op_f_clamp(self.f32_id, fog_factor, zero, one);

        // Blend the fog
        let vec3 = self.vec_ids.get(3);
        let tex_env_rgb =
            self.m
                .op_vector_shuffle(vec3, self.combiner_output, self.combiner_output, &[0, 1, 2]);
        let member = self.const_s32(19);
        let fog_color = self.get_shader_data_member(vec3, &[member]);
        let fog_factor_rgb = self
            .m
            .op_composite_construct(vec3, &[fog_factor, fog_factor, fog_factor]);
        let fog_result = self
            .m
            .op_f_mix(vec3, fog_color, tex_env_rgb, fog_factor_rgb);
        self.combiner_output = self.m.op_vector_shuffle(
            self.vec_ids.get(4),
            fog_result,
            self.combiner_output,
            &[0, 1, 2, 6],
        );
    }

    /// Writes the code to emulate gas rendering
    fn write_gas(&mut self) {
        // TODO: Implement me
        log::error!(target: "Render", "Unimplemented gas mode");
        self.m.op_kill();
        self.m.op_function_end();
    }

    /// Rounds the provided variable to the nearest 1/255th
    fn byteround(&mut self, variable_id: Id, size: u32) -> Id {
        if size > 1 {
            let vec = self.vec_ids.get(size);
            let scale = self.const_f32(255.0);
            let scaled_vec_id = self.m.op_vector_times_scalar(vec, variable_id, scale);
            let rounded_id = self.m.op_round(vec, scaled_vec_id);
            let inv_scale = self.const_f32(1.0 / 255.0);
            self.m.op_vector_times_scalar(vec, rounded_id, inv_scale)
        } else {
            let scale = self.const_f32(255.0);
            let scaled_id = self.m.op_f_mul(self.f32_id, variable_id, scale);
            let rounded_id = self.m.op_round(self.f32_id, scaled_id);
            let inv_scale = self.const_f32(1.0 / 255.0);
            self.m.op_f_mul(self.f32_id, rounded_id, inv_scale)
        }
    }

    /// Creates a constant array of integers
    fn init_table_s32(&mut self, table: Id, elems: &[i32]) {
        let table_const = self.const_s32_vec(elems);
        self.m.op_store(table, table_const);
    }

    /// Loads the member specified from the shader_data uniform struct
    fn get_shader_data_member(&mut self, ty: Id, ids: &[Id]) -> Id {
        let uniform_ptr = self.m.type_pointer(StorageClass::Uniform, ty);
        let member_ptr = self
            .m
            .op_access_chain(uniform_ptr, self.shader_data_id, ids);
        self.m.op_load(ty, member_ptr)
    }

    /// Pads the provided vector by inserting args at the end
    fn pad_vector_f32(&mut self, vector: Id, pad_type_id: Id, arg: f32) -> Id {
        let pad = self.const_f32(arg);
        self.m.op_composite_construct(pad_type_id, &[vector, pad])
    }

    /// Defines a input variable
    fn define_input(&mut self, ty: Id, location: u32) -> Id {
        let input_id = self.define_var(ty, StorageClass::Input, true);
        self.m.decorate(input_id, Decoration::Location, &[location]);
        input_id
    }

    /// Defines a input variable
    fn define_output(&mut self, ty: Id, location: u32) -> Id {
        let output_id = self.define_var(ty, StorageClass::Output, true);
        self.m
            .decorate(output_id, Decoration::Location, &[location]);
        output_id
    }

    /// Defines a uniform constant variable
    fn define_uniform_const(&mut self, ty: Id, set: u32, binding: u32, readonly: bool) -> Id {
        let uniform_id = self.define_var(ty, StorageClass::UniformConstant, true);
        self.m
            .decorate(uniform_id, Decoration::DescriptorSet, &[set]);
        self.m.decorate(uniform_id, Decoration::Binding, &[binding]);
        if readonly {
            self.m.decorate(uniform_id, Decoration::NonWritable, &[]);
        }
        uniform_id
    }

    fn define_var(&mut self, ty: Id, storage_class: StorageClass, global: bool) -> Id {
        let pointer_type_id = self.m.type_pointer(storage_class, ty);
        if global {
            self.m.add_global_variable(pointer_type_id, storage_class)
        } else {
            self.m.add_local_variable(pointer_type_id, storage_class)
        }
    }

    /// Returns the id of a signed integer constant of value
    fn const_u32(&mut self, value: u32) -> Id {
        self.m.constant(self.u32_id, value)
    }

    /// Returns the id of a signed integer constant of value
    fn const_s32(&mut self, value: i32) -> Id {
        self.m.constant(self.i32_id, value as u32)
    }

    fn const_s32_vec(&mut self, values: &[i32]) -> Id {
        let size = values.len() as u32;
        assert!(size >= 2);
        let constituents: Vec<Id> = values
            .iter()
            .map(|&value| self.m.constant(self.i32_id, value as u32))
            .collect();
        let ty = if size <= 4 {
            self.ivec_ids.get(size)
        } else {
            let length = self.const_u32(size);
            self.m.type_array(self.i32_id, length)
        };
        self.m.constant_composite(ty, &constituents)
    }

    /// Returns the id of a float constant of value
    fn const_f32(&mut self, value: f32) -> Id {
        self.m.constant(self.f32_id, value.to_bits())
    }

    fn const_f32_vec(&mut self, values: &[f32]) -> Id {
        let size = values.len() as u32;
        assert!(size >= 2);
        let constituents: Vec<Id> = values
            .iter()
            .map(|&value| self.m.constant(self.f32_id, value.to_bits()))
            .collect();
        let ty = if size <= 4 {
            self.vec_ids.get(size)
        } else {
            let length = self.const_u32(size);
            self.m.type_array(self.f32_id, length)
        };
        self.m.constant_composite(ty, &constituents)
    }
}

/**
 * Generates the SPIR-V fragment shader program source code for the current Pica state
 * @param config ShaderCacheKey object generated for the current Pica state, used for the shader
 *               configuration (NOTE: Use state in this struct only, not the Pica registers!)
 * @param separable_shader generates shader that can be used for separate shader object
 * @returns String of the shader source code
 */
pub fn generate_fragment_shader(config: &FSConfig, profile: &Profile) -> Vec<u32> {
    let mut module = FragmentModule::new(config, profile);
    module.generate();
    module.assemble()
}
