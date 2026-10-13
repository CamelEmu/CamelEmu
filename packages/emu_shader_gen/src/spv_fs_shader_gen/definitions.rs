use emu_pica_types::regs::texturing::TextureType;
use rspirv::spirv::{
    AddressingModel, BuiltIn, Capability, Decoration, Dim, ExecutionMode, ExecutionModel,
    FunctionControl, ImageFormat, MemoryModel, StorageClass,
};

use super::{
    FragmentModule, NUM_LIGHTING_SAMPLERS, NUM_LIGHTS, NUM_NON_PROC_TEX_UNITS, NUM_TEV_STAGES,
};

impl FragmentModule<'_> {
    pub(super) fn define_arithmetic_types(&mut self) {
        let void = self.m.type_void();
        self.void_id = self.m.name(void, "void_id");
        let bool_type = self.m.type_bool();
        self.bool_id = self.m.name(bool_type, "bool_id");
        let f32_type = self.m.type_float(32);
        self.f32_id = self.m.name(f32_type, "f32_id");
        let i32_type = self.m.type_s_int(32);
        self.i32_id = self.m.name(i32_type, "i32_id");
        let u32_type = self.m.type_u_int(32);
        self.u32_id = self.m.name(u32_type, "u32_id");

        for size in 2..=4u32 {
            let i = (size - 2) as usize;
            let vec = self.m.type_vector(self.f32_id, size);
            self.vec_ids.ids[i] = self.m.name(vec, &format!("vec{}_id", size));
            let ivec = self.m.type_vector(self.i32_id, size);
            self.ivec_ids.ids[i] = self.m.name(ivec, &format!("ivec{}_id", size));
            let uvec = self.m.type_vector(self.u32_id, size);
            self.uvec_ids.ids[i] = self.m.name(uvec, &format!("uvec{}_id", size));
            let bvec = self.m.type_vector(self.bool_id, size);
            self.bvec_ids.ids[i] = self.m.name(bvec, &format!("bvec{}_id", size));
        }
    }

    pub(super) fn define_entry_point(&mut self) {
        self.m.add_capability(Capability::Shader);
        self.m.add_capability(Capability::SampledBuffer);
        self.m.add_capability(Capability::ImageQuery);
        if self.use_fragment_shader_barycentric {
            self.m.add_capability(Capability::FragmentBarycentricKHR);
            self.m.add_extension("SPV_KHR_fragment_shader_barycentric");
        }
        self.m
            .set_memory_model(AddressingModel::Logical, MemoryModel::GLSL450);

        let void = self.m.type_void();
        let main_type = self.m.type_function(void);
        let void = self.m.type_void();
        let main_func = self.m.op_function(void, FunctionControl::NONE, main_type);

        let mut interface_ids = vec![
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

        self.m
            .add_entry_point(ExecutionModel::Fragment, main_func, "main", &interface_ids);
        self.m
            .add_execution_mode(main_func, ExecutionMode::OriginUpperLeft);
        self.m
            .add_execution_mode(main_func, ExecutionMode::DepthReplacing);
    }

    pub(super) fn define_uniform_structs(&mut self) {
        let vec2 = self.vec_ids.get(2);
        let vec3 = self.vec_ids.get(3);
        let vec4 = self.vec_ids.get(4);
        let f32_id = self.f32_id;
        let i32_id = self.i32_id;

        let light_src_struct_id = self
            .m
            .type_struct(&[vec3, vec3, vec3, vec3, vec3, vec3, f32_id, f32_id]);

        let length = self.const_u32(NUM_LIGHTS);
        let light_src_array_id = self.m.type_array(light_src_struct_id, length);
        let length = self.const_u32(NUM_LIGHTING_SAMPLERS / 4);
        let lighting_lut_array_id = self.m.type_array(self.ivec_ids.get(4), length);
        let length = self.const_u32(NUM_TEV_STAGES);
        let const_color_array_id = self.m.type_array(vec4, length);
        let length = self.const_u32(NUM_NON_PROC_TEX_UNITS as u32);
        let border_color_array_id = self.m.type_array(vec4, length);

        let shader_data_struct_id = self.m.type_struct(&[
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

        const LIGHT_SRC_OFFSETS: [u32; 8] = [0, 16, 32, 48, 64, 80, 92, 96];
        const SHADER_DATA_OFFSETS: [u32; 30] = [
            0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 80, 176, 192, 200,
            208, 224, 240, 1136, 1232, 1248, 1264, 1312,
        ];

        self.m
            .decorate(lighting_lut_array_id, Decoration::ArrayStride, &[16]);
        self.m
            .decorate(light_src_array_id, Decoration::ArrayStride, &[112]);
        self.m
            .decorate(const_color_array_id, Decoration::ArrayStride, &[16]);
        self.m
            .decorate(border_color_array_id, Decoration::ArrayStride, &[16]);
        for (i, &offset) in LIGHT_SRC_OFFSETS.iter().enumerate() {
            self.m
                .member_decorate(light_src_struct_id, i as u32, Decoration::Offset, &[offset]);
        }
        for (i, &offset) in SHADER_DATA_OFFSETS.iter().enumerate() {
            self.m.member_decorate(
                shader_data_struct_id,
                i as u32,
                Decoration::Offset,
                &[offset],
            );
        }
        self.m
            .decorate(shader_data_struct_id, Decoration::Block, &[]);

        let pointer_type = self
            .m
            .type_pointer(StorageClass::Uniform, shader_data_struct_id);
        self.shader_data_id = self
            .m
            .add_global_variable(pointer_type, StorageClass::Uniform);
        self.m
            .decorate(self.shader_data_id, Decoration::DescriptorSet, &[0]);
        self.m
            .decorate(self.shader_data_id, Decoration::Binding, &[2]);
    }

    pub(super) fn define_interface(&mut self) {
        // Define interface block
        self.primary_color_id = self.define_input(self.vec_ids.get(4), 1);
        self.texcoord_id[0] = self.define_input(self.vec_ids.get(2), 2);
        self.texcoord_id[1] = self.define_input(self.vec_ids.get(2), 3);
        self.texcoord_id[2] = self.define_input(self.vec_ids.get(2), 4);
        self.texcoord0_w_id = self.define_input(self.f32_id, 5);
        if self.use_fragment_shader_barycentric {
            let length = self.const_u32(3);
            let normquat_array = self.m.type_array(self.vec_ids.get(4), length);
            self.normquat_id = self.define_input(normquat_array, 6);
            self.m
                .decorate(self.normquat_id, Decoration::PerVertexKHR, &[]);
        } else {
            self.normquat_id = self.define_input(self.vec_ids.get(4), 6);
        }
        self.view_id = self.define_input(self.vec_ids.get(3), 7);
        self.color_id = self.define_output(self.vec_ids.get(4), 0);

        // Define the texture unit samplers types
        self.image_buffer_id = self.m.type_image(
            self.f32_id,
            Dim::DimBuffer,
            0,
            0,
            0,
            1,
            ImageFormat::Unknown,
        );
        self.image2d_id =
            self.m
                .type_image(self.f32_id, Dim::Dim2D, 0, 0, 0, 1, ImageFormat::Unknown);
        self.image_cube_id =
            self.m
                .type_image(self.f32_id, Dim::DimCube, 0, 0, 0, 1, ImageFormat::Unknown);
        self.image_r32_id =
            self.m
                .type_image(self.u32_id, Dim::Dim2D, 0, 0, 0, 2, ImageFormat::R32ui);
        self.sampler_id = self.m.type_sampler();

        // Define lighting texture buffers
        self.texture_buffer_lut_lf_id =
            self.define_uniform_const(self.image_buffer_id, 0, 3, false);
        self.texture_buffer_lut_rg_id =
            self.define_uniform_const(self.image_buffer_id, 0, 4, false);
        self.texture_buffer_lut_rgba_id =
            self.define_uniform_const(self.image_buffer_id, 0, 5, false);

        // Define texture unit samplers
        let texture_type = self.config.texture.texture0_type();
        let tex0_type = if texture_type == TextureType::TEXTURE_CUBE {
            self.image_cube_id
        } else {
            self.image2d_id
        };
        let sampled_image = self.m.type_sampled_image(tex0_type);
        self.tex0_id = self.define_uniform_const(sampled_image, 1, 0, false);
        let sampled_image = self.m.type_sampled_image(self.image2d_id);
        self.tex1_id = self.define_uniform_const(sampled_image, 1, 1, false);
        let sampled_image = self.m.type_sampled_image(self.image2d_id);
        self.tex2_id = self.define_uniform_const(sampled_image, 1, 2, false);

        // Define shadow textures
        self.shadow_texture_px_id = self.define_uniform_const(self.image_r32_id, 2, 0, true);

        // Define built-ins
        self.gl_frag_coord_id = self.define_var(self.vec_ids.get(4), StorageClass::Input, true);
        self.gl_frag_depth_id = self.define_var(self.f32_id, StorageClass::Output, true);
        self.m
            .decorate_built_in(self.gl_frag_coord_id, BuiltIn::FragCoord);
        self.m
            .decorate_built_in(self.gl_frag_depth_id, BuiltIn::FragDepth);
        if self.use_fragment_shader_barycentric {
            self.gl_bary_coord_id = self.define_var(self.vec_ids.get(3), StorageClass::Input, true);
            self.m
                .decorate_built_in(self.gl_bary_coord_id, BuiltIn::BaryCoordKHR);
        }
    }
}
