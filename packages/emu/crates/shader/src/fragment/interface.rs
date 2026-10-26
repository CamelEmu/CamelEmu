use emu_spirv::{Id, Literal};

use super::FragmentModule;

impl FragmentModule {
    pub(crate) fn define_arithmetic_types(&mut self) {
        let void_type = self.type_void();
        self.void_id = self.name(void_type, "void_id");
        let bool_type = self.type_bool();
        self.bool_id = self.name(bool_type, "bool_id");
        let f32_type = self.type_float(32);
        self.f32_id = self.name(f32_type, "f32_id");
        let i32_type = self.type_s_int(32);
        self.i32_id = self.name(i32_type, "i32_id");
        let u32_type = self.type_u_int(32);
        self.u32_id = self.name(u32_type, "u32_id");

        for size in 2..=4u32 {
            let i = (size - 2) as usize;
            let (f32_id, i32_id, u32_id, bool_id) =
                (self.f32_id, self.i32_id, self.u32_id, self.bool_id);

            let vec_type = self.type_vector(f32_id, size as i32);
            self.vec_ids.ids[i] = self.name(vec_type, &format!("vec{size}_id"));
            let ivec_type = self.type_vector(i32_id, size as i32);
            self.ivec_ids.ids[i] = self.name(ivec_type, &format!("ivec{size}_id"));
            let uvec_type = self.type_vector(u32_id, size as i32);
            self.uvec_ids.ids[i] = self.name(uvec_type, &format!("uvec{size}_id"));
            let bvec_type = self.type_vector(bool_id, size as i32);
            self.bvec_ids.ids[i] = self.name(bvec_type, &format!("bvec{size}_id"));
        }
    }

    pub(crate) fn define_entry_point(&mut self) {
        self.add_capability(spirv::Capability::Shader);
        self.add_capability(spirv::Capability::SampledBuffer);
        self.add_capability(spirv::Capability::ImageQuery);
        if self.use_fragment_shader_barycentric {
            self.add_capability(spirv::Capability::FragmentBarycentricKHR);
            self.add_extension("SPV_KHR_fragment_shader_barycentric");
        }
        self.set_memory_model(
            spirv::AddressingModel::Logical,
            spirv::MemoryModel::GLSL450,
        );

        let void_type = self.type_void();
        let main_type = self.type_function(void_type, &[]);
        let void_type = self.type_void();
        let main_func = self.op_function(void_type, spirv::FunctionControl::NONE, main_type);

        let mut interface_ids: Vec<Id> = vec![
            self.primary_color_id,
            self.texcoord_id[0],
            self.texcoord_id[1],
            self.texcoord_id[2],
            self.texcoord0_w_id,
            self.normquat_id,
            self.view_id,
            self.color_id,
            self.gl_frag_coord_id,
            self.gl_frag_depth_id,
        ];
        if self.use_fragment_shader_barycentric {
            interface_ids.push(self.gl_bary_coord_id);
        }

        self.add_entry_point(
            spirv::ExecutionModel::Fragment,
            main_func,
            "main",
            &interface_ids,
        );
        self.add_execution_mode(main_func, spirv::ExecutionMode::OriginUpperLeft, &[]);
        self.add_execution_mode(main_func, spirv::ExecutionMode::DepthReplacing, &[]);
    }

    pub(crate) fn define_uniform_structs(&mut self) {
        let layout = self.settings.uniform.clone();
        let (vec2, vec3, vec4) = (
            self.vec_ids.get(2),
            self.vec_ids.get(3),
            self.vec_ids.get(4),
        );
        let (f32_id, i32_id) = (self.f32_id, self.i32_id);
        let ivec4 = self.ivec_ids.get(4);

        let light_src_struct_id =
            self.type_struct(&[vec3, vec3, vec3, vec3, vec3, vec3, f32_id, f32_id]);

        let num_lights = self.const_u32(self.settings.num_lights);
        let light_src_array_id = self.type_array(light_src_struct_id, num_lights);
        let num_lut_groups = self.const_u32(self.settings.num_lighting_samplers / 4);
        let lighting_lut_array_id = self.type_array(ivec4, num_lut_groups);
        let num_tev_stages = self.const_u32(self.settings.num_tev_stages);
        let const_color_array_id = self.type_array(vec4, num_tev_stages);
        let num_border_units = self.const_u32(self.settings.num_non_proc_tex_units);
        let border_color_array_id = self.type_array(vec4, num_border_units);

        let shader_data_struct_id = self.type_struct(&[
            i32_id,
            i32_id,
            f32_id,
            f32_id,
            f32_id,
            f32_id,
            i32_id,
            i32_id,
            i32_id,
            i32_id,
            i32_id,
            i32_id,
            i32_id,
            i32_id,
            i32_id,
            i32_id,
            f32_id,
            i32_id,
            lighting_lut_array_id,
            vec3,
            vec2,
            vec2,
            vec2,
            vec3,
            light_src_array_id,
            const_color_array_id,
            vec4,
            vec3,
            border_color_array_id,
            vec4,
        ]);

        self.decorate(
            lighting_lut_array_id,
            spirv::Decoration::ArrayStride,
            &[&layout.lighting_lut_array_stride],
        );
        self.decorate(
            light_src_array_id,
            spirv::Decoration::ArrayStride,
            &[&layout.light_src_array_stride],
        );
        self.decorate(
            const_color_array_id,
            spirv::Decoration::ArrayStride,
            &[&layout.const_color_array_stride],
        );
        self.decorate(
            border_color_array_id,
            spirv::Decoration::ArrayStride,
            &[&layout.border_color_array_stride],
        );
        for (i, offset) in layout.light_src_offsets.iter().enumerate() {
            let member = i as u32;
            self.member_decorate(
                light_src_struct_id,
                &member,
                spirv::Decoration::Offset,
                &[offset as &dyn Literal],
            );
        }
        for (i, offset) in layout.shader_data_offsets.iter().enumerate() {
            let member = i as u32;
            self.member_decorate(
                shader_data_struct_id,
                &member,
                spirv::Decoration::Offset,
                &[offset as &dyn Literal],
            );
        }
        self.decorate(shader_data_struct_id, spirv::Decoration::Block, &[]);

        let uniform_pointer = self.type_pointer(spirv::StorageClass::Uniform, shader_data_struct_id);
        self.shader_data_id =
            self.add_global_variable(uniform_pointer, spirv::StorageClass::Uniform, None);
        let shader_data = self.settings.interface.shader_data.clone();
        let shader_data_id = self.shader_data_id;
        self.decorate(
            shader_data_id,
            spirv::Decoration::DescriptorSet,
            &[&shader_data.set],
        );
        self.decorate(
            shader_data_id,
            spirv::Decoration::Binding,
            &[&shader_data.binding],
        );
    }

    pub(crate) fn define_interface(&mut self) {
        let interface = self.settings.interface.clone();
        let (vec2, vec3, vec4) = (
            self.vec_ids.get(2),
            self.vec_ids.get(3),
            self.vec_ids.get(4),
        );
        let (f32_id, u32_id) = (self.f32_id, self.u32_id);

        self.primary_color_id = self.define_input(vec4, interface.primary_color_location);
        for (unit, location) in interface.texcoord_locations.iter().enumerate() {
            self.texcoord_id[unit] = self.define_input(vec2, *location);
        }
        self.texcoord0_w_id = self.define_input(f32_id, interface.texcoord0_w_location);
        if self.use_fragment_shader_barycentric {
            let vertices = self.const_u32(interface.normquat_vertices);
            let normquat_type = self.type_array(vec4, vertices);
            self.normquat_id = self.define_input(normquat_type, interface.normquat_location);
            let normquat_id = self.normquat_id;
            self.decorate(normquat_id, spirv::Decoration::PerVertexKHR, &[]);
        } else {
            self.normquat_id = self.define_input(vec4, interface.normquat_location);
        }
        self.view_id = self.define_input(vec3, interface.view_location);
        self.color_id = self.define_output(vec4, interface.color_location);

        self.image_buffer_id = self.type_image(
            f32_id,
            spirv::Dim::DimBuffer,
            0,
            false,
            false,
            interface.image_sampled as i32,
            spirv::ImageFormat::Unknown,
            None,
        );
        self.image2d_id = self.type_image(
            f32_id,
            spirv::Dim::Dim2D,
            0,
            false,
            false,
            interface.image_sampled as i32,
            spirv::ImageFormat::Unknown,
            None,
        );
        self.image_cube_id = self.type_image(
            f32_id,
            spirv::Dim::DimCube,
            0,
            false,
            false,
            interface.image_sampled as i32,
            spirv::ImageFormat::Unknown,
            None,
        );
        self.image_r32_id = self.type_image(
            u32_id,
            spirv::Dim::Dim2D,
            0,
            false,
            false,
            interface.image_storage as i32,
            spirv::ImageFormat::R32ui,
            None,
        );
        self.sampler_id = self.type_sampler();

        let image_buffer_id = self.image_buffer_id;
        self.texture_buffer_lut_lf_id = self.define_uniform_const(
            image_buffer_id,
            interface.lut_lf.set,
            interface.lut_lf.binding,
            false,
        );
        self.texture_buffer_lut_rg_id = self.define_uniform_const(
            image_buffer_id,
            interface.lut_rg.set,
            interface.lut_rg.binding,
            false,
        );
        self.texture_buffer_lut_rgba_id = self.define_uniform_const(
            image_buffer_id,
            interface.lut_rgba.set,
            interface.lut_rgba.binding,
            false,
        );

        let texture0 = crate::pica::texture0_type(self.config.texture.texture0_type());
        let tex0_type = if texture0.uses_cube_image() {
            self.image_cube_id
        } else {
            self.image2d_id
        };
        let sampled_tex0 = self.type_sampled_image(tex0_type);
        self.tex0_id =
            self.define_uniform_const(sampled_tex0, interface.tex0.set, interface.tex0.binding, false);
        let image2d_id = self.image2d_id;
        let sampled_tex1 = self.type_sampled_image(image2d_id);
        self.tex1_id =
            self.define_uniform_const(sampled_tex1, interface.tex1.set, interface.tex1.binding, false);
        let sampled_tex2 = self.type_sampled_image(image2d_id);
        self.tex2_id =
            self.define_uniform_const(sampled_tex2, interface.tex2.set, interface.tex2.binding, false);

        let image_r32_id = self.image_r32_id;
        self.shadow_texture_px_id = self.define_uniform_const(
            image_r32_id,
            interface.shadow_texture_px.set,
            interface.shadow_texture_px.binding,
            true,
        );

        self.gl_frag_coord_id = self.define_var(vec4, spirv::StorageClass::Input, true);
        self.gl_frag_depth_id = self.define_var(f32_id, spirv::StorageClass::Output, true);
        let (gl_frag_coord_id, gl_frag_depth_id) = (self.gl_frag_coord_id, self.gl_frag_depth_id);
        self.decorate(
            gl_frag_coord_id,
            spirv::Decoration::BuiltIn,
            &[&(spirv::BuiltIn::FragCoord as u32)],
        );
        self.decorate(
            gl_frag_depth_id,
            spirv::Decoration::BuiltIn,
            &[&(spirv::BuiltIn::FragDepth as u32)],
        );
        if self.use_fragment_shader_barycentric {
            self.gl_bary_coord_id = self.define_var(vec3, spirv::StorageClass::Input, true);
            let gl_bary_coord_id = self.gl_bary_coord_id;
            self.decorate(
                gl_bary_coord_id,
                spirv::Decoration::BuiltIn,
                &[&(spirv::BuiltIn::BaryCoordKHR as u32)],
            );
        }
    }
}
