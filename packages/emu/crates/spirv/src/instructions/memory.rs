use crate::id::Id;
use crate::module::Module;
use crate::operand::Literal;

impl Module {
    pub fn op_image_texel_pointer(
        &mut self,
        result_type: Id,
        image: Id,
        coordinate: Id,
        sample: Id,
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            6,
            spirv::Op::ImageTexelPointer,
            result_type,
            &[&image, &coordinate, &sample],
        )
    }

    pub fn op_load(
        &mut self,
        result_type: Id,
        pointer: Id,
        memory_access: Option<spirv::MemoryAccess>,
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5,
            spirv::Op::Load,
            result_type,
            &[&pointer, &memory_access],
        )
    }

    pub fn op_store(
        &mut self,
        pointer: Id,
        object: Id,
        memory_access: Option<spirv::MemoryAccess>,
    ) -> Id {
        self.code.emit(
            &mut self.bound,
            4,
            spirv::Op::Store,
            &[&pointer, &object, &memory_access],
        )
    }

    pub fn op_access_chain(&mut self, result_type: Id, base: Id, indexes: &[Id]) -> Id {
        debug_assert!(!indexes.is_empty());
        self.code.emit_typed(
            &mut self.bound,
            4 + indexes.len(),
            spirv::Op::AccessChain,
            result_type,
            &[&base, &indexes],
        )
    }

    pub fn op_vector_extract_dynamic(&mut self, result_type: Id, vector: Id, index: Id) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5,
            spirv::Op::VectorExtractDynamic,
            result_type,
            &[&vector, &index],
        )
    }

    pub fn op_vector_insert_dynamic(
        &mut self,
        result_type: Id,
        vector: Id,
        component: Id,
        index: Id,
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            6,
            spirv::Op::VectorInsertDynamic,
            result_type,
            &[&vector, &component, &index],
        )
    }

    pub fn op_vector_shuffle(
        &mut self,
        result_type: Id,
        vector_1: Id,
        vector_2: Id,
        components: &[&dyn Literal],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5 + components.len(),
            spirv::Op::VectorShuffle,
            result_type,
            &[&vector_1, &vector_2, &components],
        )
    }

    pub fn op_composite_insert(
        &mut self,
        result_type: Id,
        object: Id,
        composite: Id,
        indexes: &[&dyn Literal],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5 + indexes.len(),
            spirv::Op::CompositeInsert,
            result_type,
            &[&object, &composite, &indexes],
        )
    }

    pub fn op_composite_extract(
        &mut self,
        result_type: Id,
        composite: Id,
        indexes: &[&dyn Literal],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            4 + indexes.len(),
            spirv::Op::CompositeExtract,
            result_type,
            &[&composite, &indexes],
        )
    }

    pub fn op_composite_construct(&mut self, result_type: Id, ids: &[Id]) -> Id {
        debug_assert!(!ids.is_empty());
        self.code.emit_typed(
            &mut self.bound,
            3 + ids.len(),
            spirv::Op::CompositeConstruct,
            result_type,
            &[&ids],
        )
    }
}
