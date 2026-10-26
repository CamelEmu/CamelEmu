use emu_spirv::glsl;

use super::FragmentModule;
use crate::error::Result;
use crate::pica;

impl FragmentModule {
    pub(crate) fn generate(&mut self) -> Result<()> {
        let label = self.op_label();
        self.add_label(label);

        let vec4 = self.vec_ids.get(4);
        let primary_color_id = self.primary_color_id;
        let primary_color = self.op_load(vec4, primary_color_id, None);
        self.rounded_primary_color = self.byteround(primary_color, 4);
        self.primary_fragment_color = self.const_f32s(&[0.0, 0.0, 0.0, 0.0]);
        self.secondary_fragment_color = self.const_f32s(&[0.0, 0.0, 0.0, 0.0]);

        let alpha_test = pica::alpha_test_func(self.config.framebuffer.alpha_test_func());
        if alpha_test.discards_all() {
            self.op_kill();
            self.op_function_end();
            return Ok(());
        }

        self.write_depth();
        self.write_scissor();

        self.write_lighting();

        self.combiner_buffer = self.const_f32s(&[0.0, 0.0, 0.0, 0.0]);
        let combiner_buffer_color = self.settings.uniform.tev_combiner_buffer_color;
        let member = self.const_s32(combiner_buffer_color);
        self.next_combiner_buffer = self.get_shader_data_member(vec4, &[member]);
        self.combiner_output = self.const_f32s(&[0.0, 0.0, 0.0, 0.0]);

        for index in 0..self.config.texture.tev_stages.len() {
            self.write_tev_stage(index as i32);
        }

        self.write_alpha_test_condition();

        let fog = pica::fog_emulation(self.config.texture.fog_mode());
        if fog.write(self) {
            return Ok(());
        }

        let combiner_output = self.combiner_output;
        let color = self.byteround(combiner_output, 4);
        let raw = self.config.framebuffer.logic_op();
        let color = pica::logic_op(raw).apply(self, color, raw)?;

        let color_id = self.color_id;
        self.op_store(color_id, color, None);
        self.op_return();
        self.op_function_end();
        Ok(())
    }

    fn write_depth(&mut self) {
        let f32_id = self.f32_id;
        let gl_frag_coord_id = self.gl_frag_coord_id;
        let input_pointer_id = self.type_pointer(spirv::StorageClass::Input, f32_id);
        let index = self.const_u32(2);
        let access_chain = self.op_access_chain(input_pointer_id, gl_frag_coord_id, &[index]);
        let gl_frag_coord_z = self.op_load(f32_id, access_chain, None);
        let z_over_w = self.op_f_negate(f32_id, gl_frag_coord_z);
        let depth_scale_member = self.settings.uniform.depth_scale;
        let member = self.const_s32(depth_scale_member);
        let depth_scale = self.get_shader_data_member(f32_id, &[member]);
        let depth_offset_member = self.settings.uniform.depth_offset;
        let member = self.const_s32(depth_offset_member);
        let depth_offset = self.get_shader_data_member(f32_id, &[member]);
        self.depth = self.glsl::<glsl::Fma>(f32_id, &[z_over_w, depth_scale, depth_offset]);
        pica::depth_buffering(self.config.framebuffer.depthmap_enable())
            .apply(self, input_pointer_id);
        let (gl_frag_depth_id, depth) = (self.gl_frag_depth_id, self.depth);
        self.op_store(gl_frag_depth_id, depth, None);
    }

    fn write_scissor(&mut self) {
        let scissor_mode = pica::scissor_mode(self.config.framebuffer.scissor_test_mode());
        if !scissor_mode.is_enabled() {
            return;
        }

        let (vec2, vec4, bvec2, bvec4, f32_id, i32_id, bool_id) = (
            self.vec_ids.get(2),
            self.vec_ids.get(4),
            self.bvec_ids.get(2),
            self.bvec_ids.get(4),
            self.f32_id,
            self.i32_id,
            self.bool_id,
        );
        let uniform = self.settings.uniform.clone();

        let gl_frag_coord_id = self.gl_frag_coord_id;
        let gl_frag_coord = self.op_load(vec4, gl_frag_coord_id, None);
        let gl_frag_coord_xy = self.vector_shuffle(vec2, gl_frag_coord, gl_frag_coord, &[0, 1]);

        let member = self.const_s32(uniform.scissor_x1);
        let scissor_x1 = self.get_shader_data_member(i32_id, &[member]);
        let member = self.const_s32(uniform.scissor_y1);
        let scissor_y1 = self.get_shader_data_member(i32_id, &[member]);
        let x1 = self.op_convert_s_to_f(f32_id, scissor_x1);
        let y1 = self.op_convert_s_to_f(f32_id, scissor_y1);
        let scissor_1 = self.op_composite_construct(vec2, &[x1, y1]);

        let member = self.const_s32(uniform.scissor_x2);
        let scissor_x2 = self.get_shader_data_member(i32_id, &[member]);
        let member = self.const_s32(uniform.scissor_y2);
        let scissor_y2 = self.get_shader_data_member(i32_id, &[member]);
        let x2 = self.op_convert_s_to_f(f32_id, scissor_x2);
        let y2 = self.op_convert_s_to_f(f32_id, scissor_y2);
        let scissor_2 = self.op_composite_construct(vec2, &[x2, y2]);

        let cond1 = self.op_f_ord_greater_than_equal(bvec2, gl_frag_coord_xy, scissor_1);
        let cond2 = self.op_f_ord_less_than(bvec2, gl_frag_coord_xy, scissor_2);

        let conditions = self.op_composite_construct(bvec4, &[cond1, cond2]);
        let result = self.op_all(bool_id, conditions);
        let result = scissor_mode.apply(self, result);

        let merge_block = self.op_label();
        let kill_label = self.op_label();
        self.op_selection_merge(merge_block, spirv::SelectionControl::NONE);
        self.op_branch_conditional(result, kill_label, merge_block, 0, 0);

        self.add_label(kill_label);
        self.op_kill();

        self.add_label(merge_block);
    }

    pub(crate) fn write_fog(&mut self) {
        let (f32_id, i32_id, vec3, vec4) = (
            self.f32_id,
            self.i32_id,
            self.vec_ids.get(3),
            self.vec_ids.get(4),
        );
        let fog_settings = self.settings.fog.clone();
        let uniform = self.settings.uniform.clone();

        let depth = self.depth;
        let fog_index = if self.config.texture.fog_flip() {
            let one = self.const_f32(1.0);
            let flipped = self.op_f_sub(f32_id, one, depth);
            let scale = self.const_f32(fog_settings.index_scale);
            self.op_f_mul(f32_id, flipped, scale)
        } else {
            let scale = self.const_f32(fog_settings.index_scale);
            self.op_f_mul(f32_id, depth, scale)
        };

        let floor = self.glsl::<glsl::Floor>(f32_id, &[fog_index]);
        let zero = self.const_f32(0.0);
        let max = self.const_f32(fog_settings.index_max);
        let fog_i = self.glsl::<glsl::FClamp>(f32_id, &[floor, zero, max]);
        let fog_f = self.op_f_sub(f32_id, fog_index, fog_i);
        let member = self.const_s32(uniform.fog_lut_offset);
        let fog_lut_offset = self.get_shader_data_member(i32_id, &[member]);
        let fog_i_s32 = self.op_convert_f_to_s(i32_id, fog_i);
        let coord = self.op_i_add(i32_id, fog_i_s32, fog_lut_offset);
        if self.texture_buffer_lut_lf.value == 0 {
            let (image_buffer_id, lut_id) = (self.image_buffer_id, self.texture_buffer_lut_lf_id);
            self.texture_buffer_lut_lf = self.op_load(image_buffer_id, lut_id, None);
        }
        let lut_image = self.texture_buffer_lut_lf;
        let fog_lut_entry_rgba = self.op_image_fetch(vec4, lut_image, coord, None, &[]);
        let fog_lut_r = self.composite_extract(f32_id, fog_lut_entry_rgba, 0);
        let fog_lut_g = self.composite_extract(f32_id, fog_lut_entry_rgba, 1);
        let fog_factor = self.glsl::<glsl::Fma>(f32_id, &[fog_f, fog_lut_g, fog_lut_r]);
        let zero = self.const_f32(0.0);
        let one = self.const_f32(1.0);
        let fog_factor = self.glsl::<glsl::FClamp>(f32_id, &[fog_factor, zero, one]);

        let combiner_output = self.combiner_output;
        let tex_env_rgb = self.vector_shuffle(vec3, combiner_output, combiner_output, &[0, 1, 2]);
        let member = self.const_s32(uniform.fog_color);
        let fog_color = self.get_shader_data_member(vec3, &[member]);
        let fog_factor_rgb =
            self.op_composite_construct(vec3, &[fog_factor, fog_factor, fog_factor]);
        let fog_result = self.glsl::<glsl::FMix>(vec3, &[fog_color, tex_env_rgb, fog_factor_rgb]);
        self.combiner_output =
            self.vector_shuffle(vec4, fog_result, combiner_output, &[0, 1, 2, 6]);
    }

    pub(crate) fn write_gas(&mut self) {
        self.op_kill();
        self.op_function_end();
    }

    pub(crate) fn write_alpha_test_condition(&mut self) {
        let alpha_test = pica::alpha_test_func(self.config.framebuffer.alpha_test_func());
        if !alpha_test.tests_alpha() {
            return;
        }

        let (f32_id, i32_id) = (self.f32_id, self.i32_id);
        let combiner_output = self.combiner_output;
        let alpha = self.composite_extract(f32_id, combiner_output, 3);
        let byte_scale = self.settings.color.byte_scale;
        let scale = self.const_f32(byte_scale);
        let alpha_scaled = self.op_f_mul(f32_id, alpha, scale);
        let alpha_int = self.op_convert_f_to_s(i32_id, alpha_scaled);
        let member_index = self.settings.uniform.alphatest_ref;
        let member = self.const_s32(member_index);
        let alphatest_ref = self.get_shader_data_member(i32_id, &[member]);
        let alpha_comp_ref = alpha_test.discard_condition(self, alpha_int, alphatest_ref);
        let kill_label = self.op_label();
        let keep_label = self.op_label();
        self.op_selection_merge(keep_label, spirv::SelectionControl::NONE);
        self.op_branch_conditional(alpha_comp_ref, kill_label, keep_label, 0, 0);
        self.add_label(kill_label);
        self.op_kill();
        self.add_label(keep_label);
    }
}
