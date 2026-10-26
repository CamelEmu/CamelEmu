use crate::id::Id;
use crate::module::Module;
use crate::operand::Literal;

impl Module {
    pub fn constant_true(&mut self, result_type: Id) -> Id {
        self.declarations.emit_typed(
            &mut self.bound,
            3,
            spirv::Op::ConstantTrue,
            result_type,
            &[],
        )
    }

    pub fn constant_false(&mut self, result_type: Id) -> Id {
        self.declarations.emit_typed(
            &mut self.bound,
            3,
            spirv::Op::ConstantFalse,
            result_type,
            &[],
        )
    }

    pub fn constant(&mut self, result_type: Id, literal: &dyn Literal) -> Id {
        self.declarations.emit_typed(
            &mut self.bound,
            3 + 2,
            spirv::Op::Constant,
            result_type,
            &[&literal],
        )
    }

    pub fn constant_composite(&mut self, result_type: Id, constituents: &[Id]) -> Id {
        self.declarations.emit_typed(
            &mut self.bound,
            3 + constituents.len(),
            spirv::Op::ConstantComposite,
            result_type,
            &[&constituents],
        )
    }

    pub fn constant_sampler(
        &mut self,
        result_type: Id,
        addressing_mode: spirv::SamplerAddressingMode,
        normalized: bool,
        filter_mode: spirv::SamplerFilterMode,
    ) -> Id {
        self.declarations.emit_typed(
            &mut self.bound,
            6,
            spirv::Op::ConstantSampler,
            result_type,
            &[&addressing_mode, &normalized, &filter_mode],
        )
    }

    pub fn constant_null(&mut self, result_type: Id) -> Id {
        self.declarations.emit_typed(
            &mut self.bound,
            3,
            spirv::Op::ConstantNull,
            result_type,
            &[],
        )
    }
}
