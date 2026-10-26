use emu_spirv::{Id, Literal};

use super::FragmentModule;

impl FragmentModule {
    pub(crate) fn const_u32(&mut self, value: u32) -> Id {
        let type_id = self.u32_id;
        self.constant(type_id, &value)
    }

    pub(crate) fn const_s32(&mut self, value: i32) -> Id {
        let type_id = self.i32_id;
        self.constant(type_id, &value)
    }

    pub(crate) fn const_f32(&mut self, value: f32) -> Id {
        let type_id = self.f32_id;
        self.constant(type_id, &value)
    }

    pub(crate) fn const_s32s(&mut self, values: &[i32]) -> Id {
        let type_id = self.i32_id;
        let constituents: Vec<Id> = values
            .iter()
            .map(|value| self.constant(type_id, value))
            .collect();
        let size = values.len() as u32;
        let composite_type = if size <= 4 {
            self.ivec_ids.get(size)
        } else {
            let length = self.const_u32(size);
            self.type_array(type_id, length)
        };
        self.constant_composite(composite_type, &constituents)
    }

    pub(crate) fn const_f32s(&mut self, values: &[f32]) -> Id {
        let type_id = self.f32_id;
        let constituents: Vec<Id> = values
            .iter()
            .map(|value| self.constant(type_id, value))
            .collect();
        let size = values.len() as u32;
        let composite_type = if size <= 4 {
            self.vec_ids.get(size)
        } else {
            let length = self.const_u32(size);
            self.type_array(type_id, length)
        };
        self.constant_composite(composite_type, &constituents)
    }

    pub(crate) fn init_table_s32(&mut self, table: Id, elems: &[i32]) {
        let table_const = self.const_s32s(elems);
        self.op_store(table, table_const, None);
    }

    pub(crate) fn get_shader_data_member(&mut self, type_id: Id, ids: &[Id]) -> Id {
        let uniform_ptr = self.type_pointer(spirv::StorageClass::Uniform, type_id);
        let shader_data_id = self.shader_data_id;
        let access_chain = self.op_access_chain(uniform_ptr, shader_data_id, ids);
        self.op_load(type_id, access_chain, None)
    }

    pub(crate) fn pad_vector_f32(&mut self, vector: Id, pad_type_id: Id, pad: f32) -> Id {
        let pad_const = self.const_f32(pad);
        self.op_composite_construct(pad_type_id, &[vector, pad_const])
    }

    pub(crate) fn define_input(&mut self, type_id: Id, location: u32) -> Id {
        let input_id = self.define_var(type_id, spirv::StorageClass::Input, true);
        self.decorate(input_id, spirv::Decoration::Location, &[&location]);
        input_id
    }

    pub(crate) fn define_output(&mut self, type_id: Id, location: u32) -> Id {
        let output_id = self.define_var(type_id, spirv::StorageClass::Output, true);
        self.decorate(output_id, spirv::Decoration::Location, &[&location]);
        output_id
    }

    pub(crate) fn define_uniform_const(
        &mut self,
        type_id: Id,
        set: u32,
        binding: u32,
        readonly: bool,
    ) -> Id {
        let uniform_id = self.define_var(type_id, spirv::StorageClass::UniformConstant, true);
        self.decorate(uniform_id, spirv::Decoration::DescriptorSet, &[&set]);
        self.decorate(uniform_id, spirv::Decoration::Binding, &[&binding]);
        if readonly {
            self.decorate(uniform_id, spirv::Decoration::NonWritable, &[]);
        }
        uniform_id
    }

    pub(crate) fn define_var(
        &mut self,
        type_id: Id,
        storage_class: spirv::StorageClass,
        global: bool,
    ) -> Id {
        let pointer_type_id = self.type_pointer(storage_class, type_id);
        if global {
            self.add_global_variable(pointer_type_id, storage_class, None)
        } else {
            self.add_local_variable(pointer_type_id, storage_class, None)
        }
    }

    pub(crate) fn composite_extract(&mut self, type_id: Id, composite: Id, index: i32) -> Id {
        self.op_composite_extract(type_id, composite, &[&index])
    }

    pub(crate) fn vector_shuffle(
        &mut self,
        type_id: Id,
        vector_1: Id,
        vector_2: Id,
        components: &[i32],
    ) -> Id {
        let literals: Vec<&dyn Literal> = components
            .iter()
            .map(|component| component as &dyn Literal)
            .collect();
        self.op_vector_shuffle(type_id, vector_1, vector_2, &literals)
    }

    pub(crate) fn byteround(&mut self, variable_id: Id, size: u32) -> Id {
        let byte_scale = self.settings.color.byte_scale;
        if size > 1 {
            let vector_type = self.vec_ids.get(size);
            let scale = self.const_f32(byte_scale);
            let scaled_vec_id = self.op_vector_times_scalar(vector_type, variable_id, scale);
            let rounded_id = self.glsl::<emu_spirv::glsl::Round>(vector_type, &[scaled_vec_id]);
            let inverse_scale = self.const_f32(1.0 / byte_scale);
            self.op_vector_times_scalar(vector_type, rounded_id, inverse_scale)
        } else {
            let f32_id = self.f32_id;
            let scale = self.const_f32(byte_scale);
            let scaled = self.op_f_mul(f32_id, variable_id, scale);
            let rounded_id = self.glsl::<emu_spirv::glsl::Round>(f32_id, &[scaled]);
            let inverse_scale = self.const_f32(1.0 / byte_scale);
            self.op_f_mul(f32_id, rounded_id, inverse_scale)
        }
    }
}
