mod arithmetic;
mod flow_control;
mod stacks;

use stacks::{CallStackElement, CircularBuffer, IfStackElement, LoopStackElement};

use crate::nihstro::shader_bytecode::{Instruction, OpCodeType, SwizzlePattern};
use crate::shader::ShaderEngine;
use crate::shader_setup::{MAX_PROGRAM_CODE_LENGTH, ShaderSetup};
use crate::shader_unit::ShaderUnit;

struct Interpreter<'a> {
    setup: &'a ShaderSetup,
    state: &'a mut ShaderUnit,
    if_stack: CircularBuffer<IfStackElement>,
    call_stack: CircularBuffer<CallStackElement>,
    loop_stack: CircularBuffer<LoopStackElement>,
    program_counter: u32,
    is_break: bool,
    should_stop: bool,
}

impl<'a> Interpreter<'a> {
    fn new(setup: &'a ShaderSetup, state: &'a mut ShaderUnit, entry_point: u32) -> Self {
        Self {
            setup,
            state,
            if_stack: CircularBuffer::new(8),
            call_stack: CircularBuffer::new(4),
            loop_stack: CircularBuffer::new(4),
            program_counter: entry_point,
            is_break: false,
            should_stop: false,
        }
    }

    fn run(&mut self) {
        while !self.should_stop {
            self.is_break = false;
            let old_program_counter = self.program_counter;

            let instr = Instruction {
                hex: self.setup.program_code[self.program_counter as usize],
            };
            let swizzle = SwizzlePattern {
                hex: self.setup.swizzle_data[instr.common().operand_desc_id() as usize],
            };

            match instr.opcode().get_info().ty {
                OpCodeType::Arithmetic => self.run_arithmetic(instr, swizzle),
                OpCodeType::MultiplyAdd => self.run_multiply_add(instr),
                _ => self.run_flow_control(instr),
            }

            self.program_counter = self.program_counter.wrapping_add(1);

            self.pop_stacks(old_program_counter);
        }
    }

    fn pop_stacks(&mut self, old_program_counter: u32) {
        // Stacks are checked in the order CALL -> IF -> LOOP. The CALL stack
        // can be popped multiple times per instruction. A JMP at the end of a
        // scope is never taken, this is why we compare against
        // old_program_counter + 1 here.
        let mut next_program_counter = old_program_counter.wrapping_add(1);
        for i in 0..4u32 {
            match self.call_stack.back() {
                Some(call) if call.end_address == next_program_counter => {
                    // Hardware bug: when popping four CALL scopes at once, the last
                    // one doesn't update the program counter
                    if i < 3 {
                        self.program_counter = call.return_address;
                        next_program_counter = self.program_counter;
                    }
                    self.call_stack.pop_back();
                }
                _ => break,
            }
        }

        // The other two stacks can only pop one entry per instruction. They
        // are checked against the original program counter before any CALL
        // scopes were closed and they overwrite any previous program counter
        // updates.
        if let Some(&if_element) = self.if_stack.back()
            && if_element.else_address == old_program_counter.wrapping_add(1)
        {
            self.program_counter = if_element.end_address;
            self.if_stack.pop_back();
        }

        let loop_stack_size = self.loop_stack.len();
        let is_break = self.is_break;
        if let Some(lp) = self.loop_stack.back_mut()
            && (lp.end_address == old_program_counter.wrapping_add(1) || is_break)
        {
            let state = &mut *self.state;
            state.address_registers[2] =
                state.address_registers[2].wrapping_add(i32::from(lp.address_increment));
            let downcounter = lp.loop_downcounter;
            lp.loop_downcounter = downcounter.wrapping_sub(1);
            if !is_break && downcounter != 0 {
                self.program_counter = lp.entry_address;
            } else {
                self.program_counter = lp.end_address;
                // Only restore previous value if there is a surrounding LOOP scope.
                if loop_stack_size > 1 {
                    state.address_registers[2] = i32::from(lp.previous_aL);
                }
                self.loop_stack.pop_back();
            }
        }
    }
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
        Interpreter::new(setup, state, setup.entry_point).run();
    }
}
