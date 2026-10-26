use crate::id::Id;
use crate::module::Module;
use crate::operand::Literal;
use crate::stream::words_in_string;

impl Module {
    pub fn name(&mut self, target: Id, name: &str) -> Id {
        self.debug.emit(
            &mut self.bound,
            3 + words_in_string(name),
            spirv::Op::Name,
            &[&target, &name],
        );
        target
    }

    pub fn member_name(&mut self, type_id: Id, member: u32, name: &str) -> Id {
        self.debug.emit(
            &mut self.bound,
            4 + words_in_string(name),
            spirv::Op::MemberName,
            &[&type_id, &member, &name],
        );
        type_id
    }

    pub fn string(&mut self, string: &str) -> Id {
        self.debug.emit_id(
            &mut self.bound,
            3 + words_in_string(string),
            spirv::Op::String,
            &[&string],
        )
    }

    pub fn op_line(&mut self, file: Id, line: &dyn Literal, column: &dyn Literal) -> Id {
        self.debug.emit(
            &mut self.bound,
            4,
            spirv::Op::Line,
            &[&file, &line, &column],
        )
    }
}
