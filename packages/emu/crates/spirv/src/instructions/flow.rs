use crate::id::Id;
use crate::module::Module;
use crate::operand::Literal;

impl Module {
    pub fn op_phi(&mut self, result_type: Id, operands: &[Id]) -> Id {
        debug_assert!(operands.len().is_multiple_of(2));
        self.code.emit_typed(
            &mut self.bound,
            3 + operands.len(),
            spirv::Op::Phi,
            result_type,
            &[&operands],
        )
    }

    pub fn deferred_op_phi(&mut self, result_type: Id, blocks: &[Id]) -> Id {
        self.deferred_phi_nodes.push(self.code.local_address());
        let zero = 0u32;
        let mut operands: Vec<&dyn crate::operand::Operand> = Vec::with_capacity(blocks.len() * 2);
        for block in blocks {
            operands.push(&zero);
            operands.push(block);
        }
        self.code.emit_typed(
            &mut self.bound,
            3 + blocks.len() * 2,
            spirv::Op::Phi,
            result_type,
            &operands,
        )
    }

    pub fn op_loop_merge(
        &mut self,
        merge_block: Id,
        continue_target: Id,
        loop_control: spirv::LoopControl,
        literals: &[Id],
    ) -> Id {
        self.code.emit(
            &mut self.bound,
            4 + literals.len(),
            spirv::Op::LoopMerge,
            &[&merge_block, &continue_target, &loop_control, &literals],
        )
    }

    pub fn op_selection_merge(
        &mut self,
        merge_block: Id,
        selection_control: spirv::SelectionControl,
    ) -> Id {
        self.code.emit(
            &mut self.bound,
            3,
            spirv::Op::SelectionMerge,
            &[&merge_block, &selection_control],
        )
    }

    pub fn op_label(&mut self) -> Id {
        self.bound += 1;
        Id { value: self.bound }
    }

    pub fn op_label_named(&mut self, label_name: &str) -> Id {
        let label = self.op_label();
        self.name(label, label_name)
    }

    pub fn op_branch(&mut self, target_label: Id) -> Id {
        self.code
            .emit(&mut self.bound, 2, spirv::Op::Branch, &[&target_label])
    }

    pub fn op_branch_conditional(
        &mut self,
        condition: Id,
        true_label: Id,
        false_label: Id,
        true_weight: u32,
        false_weight: u32,
    ) -> Id {
        let weights = (true_weight != 0 || false_weight != 0).then_some([true_weight, false_weight]);
        let weights: &[u32] = weights.as_ref().map_or(&[], |weights| weights.as_slice());
        self.code.emit(
            &mut self.bound,
            6,
            spirv::Op::BranchConditional,
            &[&condition, &true_label, &false_label, &weights],
        )
    }

    pub fn op_switch(
        &mut self,
        selector: Id,
        default_label: Id,
        literals: &[&dyn Literal],
        labels: &[Id],
    ) -> Id {
        debug_assert!(literals.len() == labels.len());
        let mut operands: Vec<&dyn crate::operand::Operand> = vec![&selector, &default_label];
        for (literal, label) in literals.iter().zip(labels) {
            operands.push(literal);
            operands.push(label);
        }
        self.code.emit(
            &mut self.bound,
            3 + literals.len() * 2,
            spirv::Op::Switch,
            &operands,
        )
    }

    pub fn op_return(&mut self) {
        self.code.emit(&mut self.bound, 1, spirv::Op::Return, &[]);
    }

    pub fn op_unreachable(&mut self) {
        self.code
            .emit(&mut self.bound, 1, spirv::Op::Unreachable, &[]);
    }

    pub fn op_return_value(&mut self, value: Id) -> Id {
        self.code
            .emit(&mut self.bound, 2, spirv::Op::ReturnValue, &[&value])
    }

    pub fn op_kill(&mut self) {
        self.code.emit(&mut self.bound, 1, spirv::Op::Kill, &[]);
    }

    pub fn op_demote_to_helper_invocation(&mut self) {
        self.code
            .emit(&mut self.bound, 1, spirv::Op::DemoteToHelperInvocation, &[]);
    }

    pub fn op_demote_to_helper_invocation_ext(&mut self) {
        self.op_demote_to_helper_invocation();
    }

    pub fn op_terminate_invocation(&mut self) {
        self.code
            .emit(&mut self.bound, 1, spirv::Op::TerminateInvocation, &[]);
    }
}
