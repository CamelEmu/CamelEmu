use crate::id::Id;
use crate::module::Module;
use crate::operand::Literal;

impl Module {
    pub fn decorate(
        &mut self,
        target: Id,
        decoration: spirv::Decoration,
        literals: &[&dyn Literal],
    ) -> Id {
        self.annotations.emit(
            &mut self.bound,
            3 + literals.len(),
            spirv::Op::Decorate,
            &[&target, &decoration, &literals],
        )
    }

    pub fn member_decorate(
        &mut self,
        structure_type: Id,
        member: &dyn Literal,
        decoration: spirv::Decoration,
        literals: &[&dyn Literal],
    ) -> Id {
        self.annotations.emit(
            &mut self.bound,
            4 + literals.len(),
            spirv::Op::MemberDecorate,
            &[&structure_type, &member, &decoration, &literals],
        )
    }
}
