use crate::id::Id;
use crate::module::Module;

impl Module {
    pub fn op_undef(&mut self, result_type: Id) -> Id {
        self.code
            .emit_typed(&mut self.bound, 3, spirv::Op::Undef, result_type, &[])
    }

    pub fn op_emit_vertex(&mut self) {
        self.code
            .emit(&mut self.bound, 1, spirv::Op::EmitVertex, &[]);
    }

    pub fn op_end_primitive(&mut self) {
        self.code
            .emit(&mut self.bound, 1, spirv::Op::EndPrimitive, &[]);
    }

    pub fn op_emit_stream_vertex(&mut self, stream: Id) {
        self.code
            .emit(&mut self.bound, 2, spirv::Op::EmitStreamVertex, &[&stream]);
    }

    pub fn op_end_stream_primitive(&mut self, stream: Id) {
        self.code.emit(
            &mut self.bound,
            2,
            spirv::Op::EndStreamPrimitive,
            &[&stream],
        );
    }
}
