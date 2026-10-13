use emu_pica_types::nihstro::shader_bytecode::{FlowControl, FlowControlOp, Instruction, OpCodeId};

use super::Interpreter;

impl Interpreter<'_> {
    fn evaluate_condition(&self, flow_control: FlowControl) -> bool {
        let result_x = flow_control.refx() == self.state.conditional_code[0];
        let result_y = flow_control.refy() == self.state.conditional_code[1];

        match flow_control.op() {
            FlowControlOp::OR => result_x || result_y,
            FlowControlOp::AND => result_x && result_y,
            FlowControlOp::JUST_X => result_x,
            FlowControlOp::JUST_Y => result_y,
            _ => unreachable!(),
        }
    }

    /// Handles the instructions that are executed on their own.
    pub(super) fn run_flow_control(&mut self, instr: Instruction) {
        let opcode = instr.opcode();
        let flow_control = instr.flow_control();
        let uniforms = &self.setup.uniforms;

        // Handle each instruction on its own
        match opcode.id() {
            OpCodeId::END => {
                self.should_stop = true;
            }

            OpCodeId::JMPC => {
                if self.evaluate_condition(flow_control) {
                    self.program_counter = flow_control.dest_offset().wrapping_sub(1);
                }
            }

            OpCodeId::JMPU => {
                if uniforms.b[flow_control.bool_uniform_id() as usize]
                    == ((flow_control.num_instructions() & 1) == 0)
                {
                    self.program_counter = flow_control.dest_offset().wrapping_sub(1);
                }
            }

            OpCodeId::CALL => {
                self.do_call(flow_control);
            }

            OpCodeId::CALLU => {
                if uniforms.b[flow_control.bool_uniform_id() as usize] {
                    self.do_call(flow_control);
                }
            }

            OpCodeId::CALLC => {
                if self.evaluate_condition(flow_control) {
                    self.do_call(flow_control);
                }
            }

            OpCodeId::NOP => {}

            OpCodeId::IFU => {
                let cond = uniforms.b[flow_control.bool_uniform_id() as usize];
                self.do_if(flow_control, cond);
            }

            OpCodeId::IFC => {
                // TODO: Do we need to consider swizzlers here?
                let cond = self.evaluate_condition(flow_control);
                self.do_if(flow_control, cond);
            }

            OpCodeId::LOOP => {
                let loop_param = uniforms.i[flow_control.int_uniform_id() as usize];
                self.state.address_registers[2] = i32::from(loop_param.y);

                self.do_loop(flow_control, loop_param);
            }

            OpCodeId::BREAK => {
                self.is_break = true;
            }

            OpCodeId::BREAKC => {
                if self.evaluate_condition(flow_control) {
                    self.is_break = true;
                }
            }

            OpCodeId::EMIT => {
                let emitter = self.state.emitter_ptr.expect("Execute EMIT on VS");
                unsafe { (*emitter.as_ptr()).emit(&self.state.output) };
            }

            OpCodeId::SETEMIT => {
                let emitter = self.state.emitter_ptr.expect("Execute SETEMIT on VS");
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
