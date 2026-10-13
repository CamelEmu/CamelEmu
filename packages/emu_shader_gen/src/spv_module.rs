use std::collections::HashMap;

use rspirv::binary::Assemble;
use rspirv::dr::{self, Builder, InsertPoint, Operand};
use rspirv::spirv::{
    self, AddressingModel, Capability, Decoration, Dim, ExecutionMode, ExecutionModel,
    FunctionControl, GlslStd450Op, ImageFormat, ImageOperands, MemoryModel, SelectionControl,
    StorageClass, Word,
};

/// The id of a SPIR-V instruction. Zero is never a valid id.
pub type Id = Word;

pub fn valid_id(id: Id) -> bool {
    id != 0
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ConstantKey {
    Scalar(Id, u32),
    Composite(Id, Vec<Id>),
}

fn ok<T>(result: Result<T, dr::Error>) -> T {
    result.expect("Invalid SPIR-V module structure")
}

macro_rules! unary_op {
    ($($name:ident => $method:ident),* $(,)?) => {
        $(
            pub fn $name(&mut self, result_type: Id, operand: Id) -> Id {
                ok(self.builder.$method(result_type, None, operand))
            }
        )*
    };
}

macro_rules! binary_op {
    ($($name:ident => $method:ident),* $(,)?) => {
        $(
            pub fn $name(&mut self, result_type: Id, operand_1: Id, operand_2: Id) -> Id {
                ok(self.builder.$method(result_type, None, operand_1, operand_2))
            }
        )*
    };
}

macro_rules! glsl_op {
    ($($name:ident => $op:ident ($($arg:ident),*)),* $(,)?) => {
        $(
            pub fn $name(&mut self, result_type: Id, $($arg: Id),*) -> Id {
                self.ext_glsl(result_type, GlslStd450Op::$op, &[$($arg),*])
            }
        )*
    };
}

/// A SPIR-V module builder that behaves like `Sirit::Module`: types and constants are
/// deduplicated, labels can be allocated before they are added, and the GLSL.std.450
/// instruction set is imported when it is first used.
pub struct SpvModule {
    builder: Builder,
    constants: HashMap<ConstantKey, Id>,
    glsl_std_450: Id,
}

impl SpvModule {
    pub fn new(version: u32) -> Self {
        let mut builder = Builder::new();
        builder.set_version((version >> 16) as u8, (version >> 8) as u8);
        Self {
            builder,
            constants: HashMap::new(),
            glsl_std_450: 0,
        }
    }

    pub fn assemble(self) -> Vec<u32> {
        self.builder.module().assemble()
    }

    pub fn add_capability(&mut self, capability: Capability) {
        self.builder.capability(capability);
    }

    pub fn add_extension(&mut self, extension: &str) {
        self.builder.extension(extension);
    }

    pub fn set_memory_model(
        &mut self,
        addressing_model: AddressingModel,
        memory_model: MemoryModel,
    ) {
        self.builder.memory_model(addressing_model, memory_model);
    }

    pub fn add_entry_point(
        &mut self,
        execution_model: ExecutionModel,
        entry_point: Id,
        name: &str,
        interface: &[Id],
    ) {
        self.builder
            .entry_point(execution_model, entry_point, name, interface);
    }

    pub fn add_execution_mode(&mut self, entry_point: Id, mode: ExecutionMode) {
        self.builder.execution_mode(entry_point, mode, []);
    }

    pub fn name(&mut self, target: Id, name: &str) -> Id {
        self.builder.name(target, name);
        target
    }

    pub fn decorate(&mut self, target: Id, decoration: Decoration, literals: &[u32]) {
        self.builder.decorate(
            target,
            decoration,
            literals
                .iter()
                .map(|&literal| Operand::LiteralBit32(literal)),
        );
    }

    pub fn decorate_built_in(&mut self, target: Id, built_in: spirv::BuiltIn) {
        self.builder
            .decorate(target, Decoration::BuiltIn, [Operand::BuiltIn(built_in)]);
    }

    pub fn member_decorate(
        &mut self,
        structure_type: Id,
        member: u32,
        decoration: Decoration,
        literals: &[u32],
    ) {
        self.builder.member_decorate(
            structure_type,
            member,
            decoration,
            literals
                .iter()
                .map(|&literal| Operand::LiteralBit32(literal)),
        );
    }

    pub fn type_void(&mut self) -> Id {
        self.builder.type_void()
    }

    pub fn type_bool(&mut self) -> Id {
        self.builder.type_bool()
    }

    pub fn type_float(&mut self, width: u32) -> Id {
        self.builder.type_float(width, None)
    }

    pub fn type_s_int(&mut self, width: u32) -> Id {
        self.builder.type_int(width, 1)
    }

    pub fn type_u_int(&mut self, width: u32) -> Id {
        self.builder.type_int(width, 0)
    }

    pub fn type_vector(&mut self, component_type: Id, component_count: u32) -> Id {
        self.builder.type_vector(component_type, component_count)
    }

    pub fn type_array(&mut self, element_type: Id, length: Id) -> Id {
        self.builder.type_array(element_type, length)
    }

    pub fn type_struct(&mut self, members: &[Id]) -> Id {
        self.builder.type_struct(members.iter().copied())
    }

    pub fn type_pointer(&mut self, storage_class: StorageClass, pointee_type: Id) -> Id {
        self.builder.type_pointer(None, storage_class, pointee_type)
    }

    pub fn type_function(&mut self, return_type: Id) -> Id {
        self.builder.type_function(return_type, [])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn type_image(
        &mut self,
        sampled_type: Id,
        dim: Dim,
        depth: u32,
        arrayed: u32,
        ms: u32,
        sampled: u32,
        image_format: ImageFormat,
    ) -> Id {
        self.builder.type_image(
            sampled_type,
            dim,
            depth,
            arrayed,
            ms,
            sampled,
            image_format,
            None,
        )
    }

    pub fn type_sampled_image(&mut self, image_type: Id) -> Id {
        self.builder.type_sampled_image(image_type)
    }

    pub fn type_sampler(&mut self) -> Id {
        self.builder.type_sampler()
    }

    pub fn constant(&mut self, result_type: Id, value: u32) -> Id {
        let key = ConstantKey::Scalar(result_type, value);
        if let Some(&id) = self.constants.get(&key) {
            return id;
        }
        let id = self.builder.constant_bit32(result_type, value);
        self.constants.insert(key, id);
        id
    }

    pub fn constant_composite(&mut self, result_type: Id, constituents: &[Id]) -> Id {
        let key = ConstantKey::Composite(result_type, constituents.to_vec());
        if let Some(&id) = self.constants.get(&key) {
            return id;
        }
        let id = self
            .builder
            .constant_composite(result_type, constituents.iter().copied());
        self.constants.insert(key, id);
        id
    }

    pub fn add_global_variable(&mut self, pointer_type: Id, storage_class: StorageClass) -> Id {
        let id = self.builder.id();
        let inst = dr::Instruction::new(
            spirv::Op::Variable,
            Some(pointer_type),
            Some(id),
            vec![Operand::StorageClass(storage_class)],
        );
        self.builder
            .insert_types_global_values(InsertPoint::End, inst);
        id
    }

    pub fn add_local_variable(&mut self, pointer_type: Id, storage_class: StorageClass) -> Id {
        self.builder
            .variable(pointer_type, None, storage_class, None)
    }

    pub fn op_function(
        &mut self,
        result_type: Id,
        control: FunctionControl,
        function_type: Id,
    ) -> Id {
        ok(self
            .builder
            .begin_function(result_type, None, control, function_type))
    }

    pub fn op_function_end(&mut self) {
        ok(self.builder.end_function());
    }

    /// Allocates the id of a label that is added later with `add_label`
    pub fn op_label(&mut self) -> Id {
        self.builder.id()
    }

    pub fn add_label(&mut self, label: Id) -> Id {
        ok(self.builder.begin_block(Some(label)))
    }

    pub fn op_kill(&mut self) {
        ok(self.builder.kill());
    }

    pub fn op_return(&mut self) {
        ok(self.builder.ret());
    }

    pub fn op_return_value(&mut self, value: Id) {
        ok(self.builder.ret_value(value));
    }

    pub fn op_branch(&mut self, target_label: Id) {
        ok(self.builder.branch(target_label));
    }

    pub fn op_branch_conditional(&mut self, condition: Id, true_label: Id, false_label: Id) {
        ok(self
            .builder
            .branch_conditional(condition, true_label, false_label, []));
    }

    pub fn op_selection_merge(&mut self, merge_block: Id, selection_control: SelectionControl) {
        ok(self.builder.selection_merge(merge_block, selection_control));
    }

    pub fn op_phi(&mut self, result_type: Id, variable_parent: &[(Id, Id)]) -> Id {
        ok(self
            .builder
            .phi(result_type, None, variable_parent.iter().copied()))
    }

    pub fn op_function_call(&mut self, result_type: Id, function: Id) -> Id {
        ok(self.builder.function_call(result_type, None, function, []))
    }

    pub fn op_load(&mut self, result_type: Id, pointer: Id) -> Id {
        ok(self.builder.load(result_type, None, pointer, None, []))
    }

    pub fn op_store(&mut self, pointer: Id, object: Id) {
        ok(self.builder.store(pointer, object, None, []));
    }

    pub fn op_access_chain(&mut self, result_type: Id, base: Id, indexes: &[Id]) -> Id {
        ok(self
            .builder
            .access_chain(result_type, None, base, indexes.iter().copied()))
    }

    pub fn op_vector_shuffle(
        &mut self,
        result_type: Id,
        vector_1: Id,
        vector_2: Id,
        components: &[u32],
    ) -> Id {
        ok(self.builder.vector_shuffle(
            result_type,
            None,
            vector_1,
            vector_2,
            components.iter().copied(),
        ))
    }

    pub fn op_composite_construct(&mut self, result_type: Id, constituents: &[Id]) -> Id {
        ok(self
            .builder
            .composite_construct(result_type, None, constituents.iter().copied()))
    }

    pub fn op_composite_extract(&mut self, result_type: Id, composite: Id, index: u32) -> Id {
        ok(self
            .builder
            .composite_extract(result_type, None, composite, [index]))
    }

    pub fn op_composite_insert(
        &mut self,
        result_type: Id,
        object: Id,
        composite: Id,
        index: u32,
    ) -> Id {
        ok(self
            .builder
            .composite_insert(result_type, None, object, composite, [index]))
    }

    pub fn op_select(&mut self, result_type: Id, condition: Id, object_1: Id, object_2: Id) -> Id {
        ok(self
            .builder
            .select(result_type, None, condition, object_1, object_2))
    }

    pub fn op_image_fetch(&mut self, result_type: Id, image: Id, coordinate: Id) -> Id {
        ok(self
            .builder
            .image_fetch(result_type, None, image, coordinate, None, []))
    }

    pub fn op_image_read(&mut self, result_type: Id, image: Id, coordinate: Id) -> Id {
        ok(self
            .builder
            .image_read(result_type, None, image, coordinate, None, []))
    }

    pub fn op_image_query_size(&mut self, result_type: Id, image: Id) -> Id {
        ok(self.builder.image_query_size(result_type, None, image))
    }

    pub fn op_image_query_size_lod(
        &mut self,
        result_type: Id,
        image: Id,
        level_of_detail: Id,
    ) -> Id {
        ok(self
            .builder
            .image_query_size_lod(result_type, None, image, level_of_detail))
    }

    pub fn op_image(&mut self, result_type: Id, sampled_image: Id) -> Id {
        ok(self.builder.image(result_type, None, sampled_image))
    }

    pub fn op_image_sample_explicit_lod(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
        image_operands: ImageOperands,
        operand: Id,
    ) -> Id {
        ok(self.builder.image_sample_explicit_lod(
            result_type,
            None,
            sampled_image,
            coordinate,
            image_operands,
            [Operand::IdRef(operand)],
        ))
    }

    pub fn op_image_sample_implicit_lod(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
    ) -> Id {
        ok(self.builder.image_sample_implicit_lod(
            result_type,
            None,
            sampled_image,
            coordinate,
            None,
            [],
        ))
    }

    pub fn op_image_sample_proj_implicit_lod(
        &mut self,
        result_type: Id,
        sampled_image: Id,
        coordinate: Id,
    ) -> Id {
        ok(self.builder.image_sample_proj_implicit_lod(
            result_type,
            None,
            sampled_image,
            coordinate,
            None,
            [],
        ))
    }

    unary_op! {
        op_f_negate => f_negate,
        op_convert_s_to_f => convert_s_to_f,
        op_convert_f_to_s => convert_f_to_s,
        op_convert_u_to_f => convert_u_to_f,
        op_bitcast => bitcast,
        op_all => all,
        op_any => any,
        op_logical_not => logical_not,
        op_dpdx => d_pdx,
        op_dpdy => d_pdy,
    }

    binary_op! {
        op_f_add => f_add,
        op_f_sub => f_sub,
        op_f_mul => f_mul,
        op_f_div => f_div,
        op_i_add => i_add,
        op_i_sub => i_sub,
        op_i_mul => i_mul,
        op_s_div => s_div,
        op_s_mod => s_mod,
        op_vector_times_scalar => vector_times_scalar,
        op_dot => dot,
        op_shift_right_logical => shift_right_logical,
        op_shift_right_arithmetic => shift_right_arithmetic,
        op_bitwise_and => bitwise_and,
        op_bitwise_xor => bitwise_xor,
        op_f_ord_equal => f_ord_equal,
        op_f_ord_less_than => f_ord_less_than,
        op_f_ord_greater_than => f_ord_greater_than,
        op_f_ord_greater_than_equal => f_ord_greater_than_equal,
        op_i_equal => i_equal,
        op_i_not_equal => i_not_equal,
        op_s_less_than => s_less_than,
        op_s_less_than_equal => s_less_than_equal,
        op_s_greater_than => s_greater_than,
        op_s_greater_than_equal => s_greater_than_equal,
        op_u_less_than_equal => u_less_than_equal,
    }

    fn ext_glsl(&mut self, result_type: Id, op: GlslStd450Op, operands: &[Id]) -> Id {
        if !valid_id(self.glsl_std_450) {
            self.glsl_std_450 = self.builder.ext_inst_import("GLSL.std.450");
        }
        ok(self.builder.ext_inst(
            result_type,
            None,
            self.glsl_std_450,
            op as u32,
            operands.iter().map(|&operand| Operand::IdRef(operand)),
        ))
    }

    glsl_op! {
        op_round => Round(x),
        op_f_abs => FAbs(x),
        op_f_sign => FSign(x),
        op_floor => Floor(x),
        op_fract => Fract(x),
        op_sqrt => Sqrt(x),
        op_log2 => Log2(x),
        op_f_min => FMin(x, y),
        op_f_max => FMax(x, y),
        op_s_max => SMax(x, y),
        op_f_clamp => FClamp(x, min_val, max_val),
        op_f_mix => FMix(x, y, a),
        op_fma => Fma(a, b, c),
        op_length => Length(x),
        op_cross => Cross(x, y),
        op_normalize => Normalize(x),
    }
}
