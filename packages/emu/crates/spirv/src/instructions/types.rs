use crate::id::Id;
use crate::module::Module;
use crate::stream::words_in_string;

impl Module {
    pub fn type_void(&mut self) -> Id {
        self.declarations
            .emit_id(&mut self.bound, 2, spirv::Op::TypeVoid, &[])
    }

    pub fn type_bool(&mut self) -> Id {
        self.declarations
            .emit_id(&mut self.bound, 2, spirv::Op::TypeBool, &[])
    }

    pub fn type_int(&mut self, width: i32, is_signed: bool) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            4,
            spirv::Op::TypeInt,
            &[&width, &is_signed],
        )
    }

    pub fn type_s_int(&mut self, width: i32) -> Id {
        self.type_int(width, true)
    }

    pub fn type_u_int(&mut self, width: i32) -> Id {
        self.type_int(width, false)
    }

    pub fn type_float(&mut self, width: i32) -> Id {
        self.declarations
            .emit_id(&mut self.bound, 3, spirv::Op::TypeFloat, &[&width])
    }

    pub fn type_vector(&mut self, component_type: Id, component_count: i32) -> Id {
        debug_assert!(component_count >= 2);
        self.declarations.emit_id(
            &mut self.bound,
            4,
            spirv::Op::TypeVector,
            &[&component_type, &component_count],
        )
    }

    pub fn type_matrix(&mut self, column_type: Id, column_count: i32) -> Id {
        debug_assert!(column_count >= 2);
        self.declarations.emit_id(
            &mut self.bound,
            4,
            spirv::Op::TypeMatrix,
            &[&column_type, &column_count],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn type_image(
        &mut self,
        sampled_type: Id,
        dim: spirv::Dim,
        depth: i32,
        arrayed: bool,
        ms: bool,
        sampled: i32,
        image_format: spirv::ImageFormat,
        access_qualifier: Option<spirv::AccessQualifier>,
    ) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            10,
            spirv::Op::TypeImage,
            &[
                &sampled_type,
                &dim,
                &depth,
                &arrayed,
                &ms,
                &sampled,
                &image_format,
                &access_qualifier,
            ],
        )
    }

    pub fn type_sampler(&mut self) -> Id {
        self.declarations
            .emit_id(&mut self.bound, 2, spirv::Op::TypeSampler, &[])
    }

    pub fn type_sampled_image(&mut self, image_type: Id) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            3,
            spirv::Op::TypeSampledImage,
            &[&image_type],
        )
    }

    pub fn type_array(&mut self, element_type: Id, length: Id) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            4,
            spirv::Op::TypeArray,
            &[&element_type, &length],
        )
    }

    pub fn type_runtime_array(&mut self, element_type: Id) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            3,
            spirv::Op::TypeRuntimeArray,
            &[&element_type],
        )
    }

    pub fn type_struct(&mut self, members: &[Id]) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            2 + members.len(),
            spirv::Op::TypeStruct,
            &[&members],
        )
    }

    pub fn type_opaque(&mut self, name: &str) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            3 + words_in_string(name),
            spirv::Op::TypeOpaque,
            &[&name],
        )
    }

    pub fn type_pointer(&mut self, storage_class: spirv::StorageClass, type_id: Id) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            4,
            spirv::Op::TypePointer,
            &[&storage_class, &type_id],
        )
    }

    pub fn type_function(&mut self, return_type: Id, arguments: &[Id]) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            3 + arguments.len(),
            spirv::Op::TypeFunction,
            &[&return_type, &arguments],
        )
    }

    pub fn type_event(&mut self) -> Id {
        self.declarations
            .emit_id(&mut self.bound, 2, spirv::Op::TypeEvent, &[])
    }

    pub fn type_device_event(&mut self) -> Id {
        self.declarations
            .emit_id(&mut self.bound, 2, spirv::Op::TypeDeviceEvent, &[])
    }

    pub fn type_reserve_id(&mut self) -> Id {
        self.declarations
            .emit_id(&mut self.bound, 2, spirv::Op::TypeReserveId, &[])
    }

    pub fn type_queue(&mut self) -> Id {
        self.declarations
            .emit_id(&mut self.bound, 2, spirv::Op::TypeQueue, &[])
    }

    pub fn type_pipe(&mut self, access_qualifier: spirv::AccessQualifier) -> Id {
        self.declarations.emit_id(
            &mut self.bound,
            2,
            spirv::Op::TypePipe,
            &[&access_qualifier],
        )
    }
}
