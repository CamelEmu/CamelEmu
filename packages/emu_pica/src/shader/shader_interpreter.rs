use std::collections::VecDeque;

use emu_pica_types::{F24, Vec4};

use crate::nihstro::shader_bytecode::{
    CompareOp, DestRegister, FlowControl, FlowControlOp, Instruction, OpCodeId, OpCodeInfo,
    OpCodeType, RegisterType, Selector, SourceRegister, SwizzlePattern,
};
use crate::shader::ShaderEngine;
use crate::shader_setup::{MAX_PROGRAM_CODE_LENGTH, ShaderSetup};
use crate::shader_unit::ShaderUnit;
use crate::vector::component_mut;

#[derive(Clone, Copy)]
struct IfStackElement {
    else_address: u32,
    end_address: u32,
}

#[derive(Clone, Copy)]
struct CallStackElement {
    end_address: u32,
    return_address: u32,
}

#[allow(non_snake_case)]
#[derive(Clone, Copy)]
struct LoopStackElement {
    entry_address: u32,
    end_address: u32,
    loop_downcounter: u8,
    address_increment: u8,
    previous_aL: u8,
}

/// A stack of a fixed capacity that drops its oldest element when it is full, as
/// `boost::circular_buffer` does.
struct CircularBuffer<T> {
    elements: VecDeque<T>,
    capacity: usize,
}

impl<T> CircularBuffer<T> {
    fn new(capacity: usize) -> Self {
        Self {
            elements: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    fn push_back(&mut self, element: T) {
        if self.elements.len() == self.capacity {
            self.elements.pop_front();
        }
        self.elements.push_back(element);
    }

    fn pop_back(&mut self) {
        self.elements.pop_back();
    }

    fn back(&self) -> Option<&T> {
        self.elements.back()
    }

    fn back_mut(&mut self) -> Option<&mut T> {
        self.elements.back_mut()
    }

    fn len(&self) -> usize {
        self.elements.len()
    }
}

fn to_array(vector: &Vec4<F24>) -> [F24; 4] {
    [vector.x, vector.y, vector.z, vector.w]
}

fn swizzle_source(source: &[F24; 4], selectors: [Selector; 4], negate: bool) -> [F24; 4] {
    let mut result = selectors.map(|selector| source[selector.0 as usize]);
    if negate {
        result = result.map(|value| -value);
    }
    result
}

fn dest_register<'a>(
    state: &'a mut ShaderUnit,
    dest: DestRegister,
    dummy: &'a mut Vec4<F24>,
) -> &'a mut Vec4<F24> {
    if dest.0 < 0x10 {
        &mut state.output[dest.get_index() as usize]
    } else if dest.0 < 0x20 {
        &mut state.temporary[dest.get_index() as usize]
    } else {
        dummy
    }
}

fn run_interpreter(setup: &ShaderSetup, state: &mut ShaderUnit, entry_point: u32) {
    let mut if_stack = CircularBuffer::<IfStackElement>::new(8);
    let mut call_stack = CircularBuffer::<CallStackElement>::new(4);
    let mut loop_stack = CircularBuffer::<LoopStackElement>::new(4);
    let mut program_counter = entry_point;

    let evaluate_condition = |state: &ShaderUnit, flow_control: FlowControl| -> bool {
        let result_x = flow_control.refx() == state.conditional_code[0];
        let result_y = flow_control.refy() == state.conditional_code[1];

        match flow_control.op() {
            FlowControlOp::OR => result_x || result_y,
            FlowControlOp::AND => result_x && result_y,
            FlowControlOp::JUST_X => result_x,
            FlowControlOp::JUST_Y => result_y,
            _ => unreachable!(),
        }
    };

    let uniforms = &setup.uniforms;
    let swizzle_data = &setup.swizzle_data;
    let program_code = &setup.program_code;

    // Constants for handling invalid inputs
    let dummy_vec4_float24_zeros = [F24::zero(); 4];
    let dummy_vec4_float24_ones = [F24::one(); 4];

    let mut should_stop = false;
    while !should_stop {
        let mut is_break = false;
        let old_program_counter = program_counter;

        let instr = Instruction {
            hex: program_code[program_counter as usize],
        };
        let swizzle = SwizzlePattern {
            hex: swizzle_data[instr.common().operand_desc_id() as usize],
        };

        let lookup_source_register =
            |state: &ShaderUnit, source_reg: SourceRegister, address_register_index: u32| {
                let mut index = source_reg.get_index();
                match source_reg.get_register_type() {
                    RegisterType::Input => to_array(&state.input[index as usize]),

                    RegisterType::Temporary => to_array(&state.temporary[index as usize]),

                    RegisterType::FloatUniform => {
                        if address_register_index != 0 {
                            let mut offset =
                                state.address_registers[address_register_index as usize - 1];
                            if offset < i32::from(i8::MIN) || offset > i32::from(i8::MAX) {
                                offset = 0;
                            }
                            index = (index + offset) & 0x7F;
                            // If the index is above 96, the result is all one.
                            if index >= 96 {
                                return dummy_vec4_float24_ones;
                            }
                        }
                        to_array(&uniforms.f[index as usize])
                    }

                    _ => dummy_vec4_float24_zeros,
                }
            };

        let opcode = instr.opcode();
        match opcode.get_info().ty {
            OpCodeType::Arithmetic => {
                let is_inverted = 0 != (opcode.get_info().subtype & OpCodeInfo::SRC_INVERSED);

                let common = instr.common();
                let src1_ = lookup_source_register(
                    state,
                    common.get_src1(is_inverted),
                    u32::from(!is_inverted) * common.address_register_index(),
                );
                let src2_ = lookup_source_register(
                    state,
                    common.get_src2(is_inverted),
                    u32::from(is_inverted) * common.address_register_index(),
                );

                let negate_src1 = swizzle.negate_src1();
                let negate_src2 = swizzle.negate_src2();

                let mut src1 = swizzle_source(
                    &src1_,
                    [
                        swizzle.src1_selector_0(),
                        swizzle.src1_selector_1(),
                        swizzle.src1_selector_2(),
                        swizzle.src1_selector_3(),
                    ],
                    negate_src1,
                );
                let src2 = swizzle_source(
                    &src2_,
                    [
                        swizzle.src2_selector_0(),
                        swizzle.src2_selector_1(),
                        swizzle.src2_selector_2(),
                        swizzle.src2_selector_3(),
                    ],
                    negate_src2,
                );

                let mut dummy = Vec4::default();

                match opcode.effective_op_code() {
                    OpCodeId::ADD => {
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            let i = i as usize;
                            *component_mut(dest, i) = src1[i] + src2[i];
                        }
                    }

                    OpCodeId::MUL => {
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            let i = i as usize;
                            *component_mut(dest, i) = src1[i] * src2[i];
                        }
                    }

                    OpCodeId::FLR => {
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            let i = i as usize;
                            *component_mut(dest, i) =
                                F24::from_float32(src1[i].to_float32().floor());
                        }
                    }

                    OpCodeId::MAX => {
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            let i = i as usize;
                            // NOTE: Exact form required to match NaN semantics to hardware:
                            //   max(0, NaN) -> NaN
                            //   max(NaN, 0) -> 0
                            *component_mut(dest, i) =
                                if src1[i] > src2[i] { src1[i] } else { src2[i] };
                        }
                    }

                    OpCodeId::MIN => {
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            let i = i as usize;
                            // NOTE: Exact form required to match NaN semantics to hardware:
                            //   min(0, NaN) -> NaN
                            //   min(NaN, 0) -> 0
                            *component_mut(dest, i) =
                                if src1[i] < src2[i] { src1[i] } else { src2[i] };
                        }
                    }

                    OpCodeId::DP3 | OpCodeId::DP4 | OpCodeId::DPH | OpCodeId::DPHI => {
                        let opcode = opcode.effective_op_code();
                        if opcode == OpCodeId::DPH || opcode == OpCodeId::DPHI {
                            src1[3] = F24::one();
                        }

                        let num_components = if opcode == OpCodeId::DP3 { 3 } else { 4 };
                        let dot = src1[..num_components]
                            .iter()
                            .zip(&src2[..num_components])
                            .fold(F24::zero(), |dot, (&a, &b)| dot + a * b);

                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            *component_mut(dest, i as usize) = dot;
                        }
                    }

                    // Reciprocal
                    OpCodeId::RCP => {
                        let rcp_res = F24::from_float32(1.0f32 / src1[0].to_float32());
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            *component_mut(dest, i as usize) = rcp_res;
                        }
                    }

                    // Reciprocal Square Root
                    OpCodeId::RSQ => {
                        let rsq_res = F24::from_float32(1.0f32 / src1[0].to_float32().sqrt());
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            *component_mut(dest, i as usize) = rsq_res;
                        }
                    }

                    OpCodeId::MOVA => {
                        for i in 0..2 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            // TODO: Figure out how the rounding is done on hardware
                            state.address_registers[i as usize] =
                                src1[i as usize].to_float32() as i32;
                        }
                    }

                    OpCodeId::MOV => {
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            let i = i as usize;
                            *component_mut(dest, i) = src1[i];
                        }
                    }

                    OpCodeId::SGE | OpCodeId::SGEI => {
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            let i = i as usize;
                            *component_mut(dest, i) = if src1[i] >= src2[i] {
                                F24::one()
                            } else {
                                F24::zero()
                            };
                        }
                    }

                    OpCodeId::SLT | OpCodeId::SLTI => {
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            let i = i as usize;
                            *component_mut(dest, i) = if src1[i] < src2[i] {
                                F24::one()
                            } else {
                                F24::zero()
                            };
                        }
                    }

                    OpCodeId::CMP => {
                        for i in 0..2 {
                            // TODO: Can you restrict to one compare via dest masking?

                            let compare_op = common.compare_op();
                            let op = if i == 0 {
                                compare_op.x()
                            } else {
                                compare_op.y()
                            };

                            match op {
                                CompareOp::EQUAL => {
                                    state.conditional_code[i] = src1[i] == src2[i];
                                }

                                CompareOp::NOT_EQUAL => {
                                    state.conditional_code[i] = src1[i] != src2[i];
                                }

                                CompareOp::LESS_THAN => {
                                    state.conditional_code[i] = src1[i] < src2[i];
                                }

                                CompareOp::LESS_EQUAL => {
                                    state.conditional_code[i] = src1[i] <= src2[i];
                                }

                                CompareOp::GREATER_THAN => {
                                    state.conditional_code[i] = src1[i] > src2[i];
                                }

                                CompareOp::GREATER_EQUAL => {
                                    state.conditional_code[i] = src1[i] >= src2[i];
                                }

                                _ => {
                                    log::error!(
                                        target: "HW_GPU",
                                        "Unknown compare mode {:x}",
                                        op.0
                                    );
                                }
                            }
                        }
                    }

                    OpCodeId::EX2 => {
                        // EX2 only takes first component exp2 and writes it to all dest components
                        let ex2_res = F24::from_float32(src1[0].to_float32().exp2());
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            *component_mut(dest, i as usize) = ex2_res;
                        }
                    }

                    OpCodeId::LG2 => {
                        // LG2 only takes the first component log2 and writes it to all dest components
                        let lg2_res = F24::from_float32(src1[0].to_float32().log2());
                        let dest = dest_register(state, common.dest(), &mut dummy);
                        for i in 0..4 {
                            if !swizzle.dest_component_enabled(i) {
                                continue;
                            }

                            *component_mut(dest, i as usize) = lg2_res;
                        }
                    }

                    _ => {
                        log::error!(
                            target: "HW_GPU",
                            "Unhandled arithmetic instruction: 0x{:02x} ({}): 0x{:08x}",
                            opcode.effective_op_code().0,
                            opcode.get_info().name,
                            instr.hex
                        );
                        debug_assert!(false);
                    }
                }
            }

            OpCodeType::MultiplyAdd => {
                if opcode.effective_op_code() == OpCodeId::MAD
                    || opcode.effective_op_code() == OpCodeId::MADI
                {
                    let mad = instr.mad();
                    let mad_swizzle = SwizzlePattern {
                        hex: swizzle_data[mad.operand_desc_id() as usize],
                    };

                    let is_inverted = opcode.effective_op_code() == OpCodeId::MADI;

                    let src1_ = lookup_source_register(state, mad.get_src1(is_inverted), 0);
                    let src2_ = lookup_source_register(
                        state,
                        mad.get_src2(is_inverted),
                        u32::from(!is_inverted) * mad.address_register_index(),
                    );
                    let src3_ = lookup_source_register(
                        state,
                        mad.get_src3(is_inverted),
                        u32::from(is_inverted) * mad.address_register_index(),
                    );

                    let negate_src1 = mad_swizzle.negate_src1();
                    let negate_src2 = mad_swizzle.negate_src2();
                    let negate_src3 = mad_swizzle.negate_src3();

                    let src1 = swizzle_source(
                        &src1_,
                        [
                            mad_swizzle.src1_selector_0(),
                            mad_swizzle.src1_selector_1(),
                            mad_swizzle.src1_selector_2(),
                            mad_swizzle.src1_selector_3(),
                        ],
                        negate_src1,
                    );
                    let src2 = swizzle_source(
                        &src2_,
                        [
                            mad_swizzle.src2_selector_0(),
                            mad_swizzle.src2_selector_1(),
                            mad_swizzle.src2_selector_2(),
                            mad_swizzle.src2_selector_3(),
                        ],
                        negate_src2,
                    );
                    let src3 = swizzle_source(
                        &src3_,
                        [
                            mad_swizzle.src3_selector_0(),
                            mad_swizzle.src3_selector_1(),
                            mad_swizzle.src3_selector_2(),
                            mad_swizzle.src3_selector_3(),
                        ],
                        negate_src3,
                    );

                    let mut dummy = Vec4::default();
                    let dest = dest_register(state, mad.dest(), &mut dummy);

                    for i in 0..4 {
                        if !mad_swizzle.dest_component_enabled(i) {
                            continue;
                        }

                        let i = i as usize;
                        *component_mut(dest, i) = src1[i] * src2[i] + src3[i];
                    }
                } else {
                    log::error!(
                        target: "HW_GPU",
                        "Unhandled multiply-add instruction: 0x{:02x} ({}): 0x{:08x}",
                        opcode.effective_op_code().0,
                        opcode.get_info().name,
                        instr.hex
                    );
                }
            }

            _ => {
                let flow_control = instr.flow_control();

                // Handle each instruction on its own
                match opcode.id() {
                    OpCodeId::END => {
                        should_stop = true;
                    }

                    OpCodeId::JMPC => {
                        if evaluate_condition(state, flow_control) {
                            program_counter = flow_control.dest_offset().wrapping_sub(1);
                        }
                    }

                    OpCodeId::JMPU => {
                        if uniforms.b[flow_control.bool_uniform_id() as usize]
                            == ((flow_control.num_instructions() & 1) == 0)
                        {
                            program_counter = flow_control.dest_offset().wrapping_sub(1);
                        }
                    }

                    OpCodeId::CALL => {
                        do_call(&mut call_stack, &mut program_counter, flow_control);
                    }

                    OpCodeId::CALLU => {
                        if uniforms.b[flow_control.bool_uniform_id() as usize] {
                            do_call(&mut call_stack, &mut program_counter, flow_control);
                        }
                    }

                    OpCodeId::CALLC => {
                        if evaluate_condition(state, flow_control) {
                            do_call(&mut call_stack, &mut program_counter, flow_control);
                        }
                    }

                    OpCodeId::NOP => {}

                    OpCodeId::IFU => {
                        let cond = uniforms.b[flow_control.bool_uniform_id() as usize];
                        do_if(&mut if_stack, &mut program_counter, flow_control, cond);
                    }

                    OpCodeId::IFC => {
                        // TODO: Do we need to consider swizzlers here?
                        let cond = evaluate_condition(state, flow_control);
                        do_if(&mut if_stack, &mut program_counter, flow_control, cond);
                    }

                    OpCodeId::LOOP => {
                        let loop_param = uniforms.i[flow_control.int_uniform_id() as usize];
                        state.address_registers[2] = i32::from(loop_param.y);

                        do_loop(
                            &mut loop_stack,
                            state,
                            program_counter,
                            flow_control,
                            loop_param,
                        );
                    }

                    OpCodeId::BREAK => {
                        is_break = true;
                    }

                    OpCodeId::BREAKC => {
                        if evaluate_condition(state, flow_control) {
                            is_break = true;
                        }
                    }

                    OpCodeId::EMIT => {
                        let emitter = state.emitter_ptr.expect("Execute EMIT on VS");
                        unsafe { (*emitter.as_ptr()).emit(&state.output) };
                    }

                    OpCodeId::SETEMIT => {
                        let emitter = state.emitter_ptr.expect("Execute SETEMIT on VS");
                        let setemit = instr.setemit();
                        let emitter = unsafe { &mut *emitter.as_ptr() };
                        emitter.vertex_id = setemit.vertex_id() as u8;
                        emitter.prim_emit = setemit.prim_emit() != 0;
                        emitter.winding = setemit.winding() != 0;
                    }

                    _ => {
                        log::error!(
                            target: "HW_GPU",
                            "Unhandled instruction: 0x{:02x} ({}): 0x{:08x}",
                            opcode.effective_op_code().0,
                            opcode.get_info().name,
                            instr.hex
                        );
                    }
                }
            }
        }

        program_counter = program_counter.wrapping_add(1);

        // Stacks are checked in the order CALL -> IF -> LOOP. The CALL stack
        // can be popped multiple times per instruction. A JMP at the end of a
        // scope is never taken, this is why we compare against
        // old_program_counter + 1 here.
        let mut next_program_counter = old_program_counter.wrapping_add(1);
        for i in 0..4u32 {
            match call_stack.back() {
                Some(call) if call.end_address == next_program_counter => {
                    // Hardware bug: when popping four CALL scopes at once, the last
                    // one doesn't update the program counter
                    if i < 3 {
                        program_counter = call.return_address;
                        next_program_counter = program_counter;
                    }
                    call_stack.pop_back();
                }
                _ => break,
            }
        }

        // The other two stacks can only pop one entry per instruction. They
        // are checked against the original program counter before any CALL
        // scopes were closed and they overwrite any previous program counter
        // updates.
        if let Some(&if_element) = if_stack.back()
            && if_element.else_address == old_program_counter.wrapping_add(1)
        {
            program_counter = if_element.end_address;
            if_stack.pop_back();
        }

        let loop_stack_size = loop_stack.len();
        if let Some(lp) = loop_stack.back_mut()
            && (lp.end_address == old_program_counter.wrapping_add(1) || is_break)
        {
            state.address_registers[2] =
                state.address_registers[2].wrapping_add(i32::from(lp.address_increment));
            let downcounter = lp.loop_downcounter;
            lp.loop_downcounter = downcounter.wrapping_sub(1);
            if !is_break && downcounter != 0 {
                program_counter = lp.entry_address;
            } else {
                program_counter = lp.end_address;
                // Only restore previous value if there is a surrounding LOOP scope.
                if loop_stack_size > 1 {
                    state.address_registers[2] = i32::from(lp.previous_aL);
                }
                loop_stack.pop_back();
            }
        }
    }
}

fn do_if(
    if_stack: &mut CircularBuffer<IfStackElement>,
    program_counter: &mut u32,
    flow_control: FlowControl,
    condition: bool,
) {
    if condition {
        if_stack.push_back(IfStackElement {
            else_address: flow_control.dest_offset(),
            end_address: flow_control.dest_offset() + flow_control.num_instructions(),
        });
    } else {
        *program_counter = flow_control.dest_offset().wrapping_sub(1);
    }
}

fn do_call(
    call_stack: &mut CircularBuffer<CallStackElement>,
    program_counter: &mut u32,
    flow_control: FlowControl,
) {
    call_stack.push_back(CallStackElement {
        end_address: flow_control.dest_offset() + flow_control.num_instructions(),
        return_address: program_counter.wrapping_add(1),
    });
    *program_counter = flow_control.dest_offset().wrapping_sub(1);
}

#[allow(non_snake_case)]
fn do_loop(
    loop_stack: &mut CircularBuffer<LoopStackElement>,
    state: &mut ShaderUnit,
    program_counter: u32,
    flow_control: FlowControl,
    loop_param: Vec4<u8>,
) {
    let previous_aL = state.address_registers[2] as u8;
    loop_stack.push_back(LoopStackElement {
        entry_address: program_counter.wrapping_add(1),
        end_address: flow_control.dest_offset() + 1,
        loop_downcounter: loop_param.x,
        address_increment: loop_param.z,
        previous_aL,
    });
    state.address_registers[2] = i32::from(loop_param.y);
}

pub struct InterpreterEngine;

impl ShaderEngine for InterpreterEngine {
    fn setup_batch(&mut self, setup: &mut ShaderSetup, entry_point: u32) {
        assert!(
            (entry_point as usize) < MAX_PROGRAM_CODE_LENGTH,
            "Assertion Failed!"
        );
        setup.entry_point = entry_point;
    }

    fn run(&self, setup: &ShaderSetup, state: &mut ShaderUnit) {
        run_interpreter(setup, state, setup.entry_point);
    }
}
