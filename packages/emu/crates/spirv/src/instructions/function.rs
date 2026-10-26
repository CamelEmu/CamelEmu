use crate::id::Id;
use crate::module::Module;

impl Module {
    pub fn op_function(
        &mut self,
        result_type: Id,
        function_control: spirv::FunctionControl,
        function_type: Id,
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5,
            spirv::Op::Function,
            result_type,
            &[&function_control, &function_type],
        )
    }

    pub fn op_function_end(&mut self) {
        self.code
            .emit(&mut self.bound, 1, spirv::Op::FunctionEnd, &[]);
    }

    pub fn op_function_call(&mut self, result_type: Id, function: Id, arguments: &[Id]) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            4 + arguments.len(),
            spirv::Op::FunctionCall,
            result_type,
            &[&function, &arguments],
        )
    }

    pub fn op_function_parameter(&mut self, result_type: Id) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            3,
            spirv::Op::FunctionParameter,
            result_type,
            &[],
        )
    }
}
