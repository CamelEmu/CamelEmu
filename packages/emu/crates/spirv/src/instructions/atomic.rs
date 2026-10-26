use crate::id::Id;
use crate::module::Module;

typed_instructions! {
    op_atomic_load(pointer, memory, semantics) => AtomicLoad;
    op_atomic_exchange(pointer, memory, semantics, value) => AtomicExchange;
    op_atomic_compare_exchange(pointer, memory, equal, unequal, value, comparator) => AtomicCompareExchange;
    op_atomic_i_increment(pointer, memory, semantics) => AtomicIIncrement;
    op_atomic_i_decrement(pointer, memory, semantics) => AtomicIDecrement;
    op_atomic_i_add(pointer, memory, semantics, value) => AtomicIAdd;
    op_atomic_i_sub(pointer, memory, semantics, value) => AtomicISub;
    op_atomic_s_min(pointer, memory, semantics, value) => AtomicSMin;
    op_atomic_u_min(pointer, memory, semantics, value) => AtomicUMin;
    op_atomic_s_max(pointer, memory, semantics, value) => AtomicSMax;
    op_atomic_u_max(pointer, memory, semantics, value) => AtomicUMax;
    op_atomic_and(pointer, memory, semantics, value) => AtomicAnd;
    op_atomic_or(pointer, memory, semantics, value) => AtomicOr;
    op_atomic_xor(pointer, memory, semantics, value) => AtomicXor;
}

impl Module {
    pub fn op_atomic_store(&mut self, pointer: Id, memory: Id, semantics: Id, value: Id) -> Id {
        self.code.emit_id(
            &mut self.bound,
            5,
            spirv::Op::AtomicStore,
            &[&pointer, &memory, &semantics, &value],
        )
    }
}
