use emu_pica_types::nihstro::shader_bytecode::{
    CompareOp, Instruction, OpCodeId, OpCodeInfo, OpCodeType, SwizzlePattern,
};

use super::control_flow::ExitMethod;
use super::generator::GlslGenerator;
use super::writer::{get_selector_src1, get_selector_src2, get_selector_src3};
use super::{DecompileFail, PROGRAM_END};

impl GlslGenerator<'_, '_> {
    /**
     * Compiles a single instruction from PICA to GLSL.
     * @param offset the offset of the PICA shader instruction.
     * @return the offset of the next instruction to execute. Usually it is the current offset + 1.
     * If the current instruction is IF or LOOP, the next instruction is after the IF or LOOP block.
     * If the current instruction always terminates the program, returns PROGRAM_END.
     */
    pub(super) fn compile_instr(&mut self, offset: u32) -> Result<u32, DecompileFail> {
        let instr = Instruction {
            hex: self.program_code[offset as usize],
        };
        let info = instr.opcode().get_info();

        let swizzle_offset = if info.ty == OpCodeType::MultiplyAdd {
            instr.mad().operand_desc_id()
        } else {
            instr.common().operand_desc_id()
        };
        let swizzle = SwizzlePattern {
            hex: self.swizzle_data[swizzle_offset as usize],
        };

        self.shader
            .add_line(format!("// {}: {}", offset, info.name));

        let offset = match info.ty {
            OpCodeType::Arithmetic => {
                self.compile_arithmetic(instr, &swizzle)?;
                offset
            }
            OpCodeType::MultiplyAdd => {
                self.compile_multiply_add(instr, &swizzle)?;
                offset
            }
            _ => self.compile_flow_control(instr, offset)?,
        };
        Ok(offset.wrapping_add(1))
    }

    fn compile_arithmetic(
        &mut self,
        instr: Instruction,
        swizzle: &SwizzlePattern,
    ) -> Result<(), DecompileFail> {
        let opcode = instr.opcode();
        let is_inverted = 0 != (opcode.get_info().subtype & OpCodeInfo::SRC_INVERSED);
        let common = instr.common();

        let mut src1 = String::from(if swizzle.negate_src1() { "-" } else { "" });
        src1 += &self.get_source_register(
            common.get_src1(is_inverted),
            u32::from(!is_inverted) * common.address_register_index(),
        );
        src1 += &format!(".{}", get_selector_src1(swizzle));

        let mut src2 = String::from(if swizzle.negate_src2() { "-" } else { "" });
        src2 += &self.get_source_register(
            common.get_src2(is_inverted),
            u32::from(is_inverted) * common.address_register_index(),
        );
        src2 += &format!(".{}", get_selector_src2(swizzle));

        let dest_reg = self.get_dest_register(common.dest());

        match opcode.effective_op_code() {
            OpCodeId::ADD => {
                self.set_dest(swizzle, &dest_reg, &format!("{} + {}", src1, src2), 4, 4);
            }

            OpCodeId::MUL => {
                if self.sanitize_mul {
                    self.set_dest(
                        swizzle,
                        &dest_reg,
                        &format!("sanitize_mul({}, {})", src1, src2),
                        4,
                        4,
                    );
                } else {
                    self.set_dest(swizzle, &dest_reg, &format!("{} * {}", src1, src2), 4, 4);
                }
            }

            OpCodeId::FLR => {
                self.set_dest(swizzle, &dest_reg, &format!("floor({})", src1), 4, 4);
            }

            OpCodeId::MAX => {
                if self.sanitize_mul {
                    self.set_dest(
                        swizzle,
                        &dest_reg,
                        &format!("mix({1}, {0}, greaterThan({0}, {1}))", src1, src2),
                        4,
                        4,
                    );
                } else {
                    self.set_dest(
                        swizzle,
                        &dest_reg,
                        &format!("max({}, {})", src1, src2),
                        4,
                        4,
                    );
                }
            }

            OpCodeId::MIN => {
                if self.sanitize_mul {
                    self.set_dest(
                        swizzle,
                        &dest_reg,
                        &format!("mix({1}, {0}, lessThan({0}, {1}))", src1, src2),
                        4,
                        4,
                    );
                } else {
                    self.set_dest(
                        swizzle,
                        &dest_reg,
                        &format!("min({}, {})", src1, src2),
                        4,
                        4,
                    );
                }
            }

            OpCodeId::DP3 | OpCodeId::DP4 | OpCodeId::DPH | OpCodeId::DPHI => {
                let opcode = opcode.effective_op_code();
                let dot = if opcode == OpCodeId::DP3 {
                    if self.sanitize_mul {
                        format!("dot(vec3(sanitize_mul({}, {})), vec3(1.0))", src1, src2)
                    } else {
                        format!("dot(vec3({}), vec3({}))", src1, src2)
                    }
                } else if self.sanitize_mul {
                    let src1_ = if opcode == OpCodeId::DPH || opcode == OpCodeId::DPHI {
                        format!("vec4({}.xyz, 1.0)", src1)
                    } else {
                        src1
                    };

                    format!("dot(sanitize_mul({}, {}), vec4(1.0))", src1_, src2)
                } else {
                    format!("dot({}, {})", src1, src2)
                };

                self.set_dest(swizzle, &dest_reg, &dot, 4, 1);
            }

            OpCodeId::RCP => {
                if !self.sanitize_mul {
                    // When accurate multiplication is OFF, NaN are not really handled. This is a
                    // workaround to cheaply avoid NaN. Fixes graphical issues in Ocarina of Time.
                    self.shader.add_line(format!("if ({}.x != 0.0)", src1));
                }
                self.set_dest(swizzle, &dest_reg, &format!("(1.0 / {}.x)", src1), 4, 1);
            }

            OpCodeId::RSQ => {
                if !self.sanitize_mul {
                    // When accurate multiplication is OFF, NaN are not really handled. This is a
                    // workaround to cheaply avoid NaN. Fixes graphical issues in Ocarina of Time.
                    self.shader.add_line(format!("if ({}.x > 0.0)", src1));
                }
                self.set_dest(
                    swizzle,
                    &dest_reg,
                    &format!("inversesqrt({}.x)", src1),
                    4,
                    1,
                );
            }

            OpCodeId::MOVA => {
                self.set_dest(
                    swizzle,
                    "address_registers",
                    &format!("ivec2({})", src1),
                    2,
                    2,
                );
            }

            OpCodeId::MOV => {
                self.set_dest(swizzle, &dest_reg, &src1, 4, 4);
            }

            OpCodeId::SGE | OpCodeId::SGEI => {
                self.set_dest(
                    swizzle,
                    &dest_reg,
                    &format!("vec4(greaterThanEqual({}, {}))", src1, src2),
                    4,
                    4,
                );
            }

            OpCodeId::SLT | OpCodeId::SLTI => {
                self.set_dest(
                    swizzle,
                    &dest_reg,
                    &format!("vec4(lessThan({}, {}))", src1, src2),
                    4,
                    4,
                );
            }

            OpCodeId::CMP => {
                self.compile_compare(instr, &src1, &src2);
            }

            OpCodeId::EX2 => {
                self.set_dest(swizzle, &dest_reg, &format!("exp2({}.x)", src1), 4, 1);
            }

            OpCodeId::LG2 => {
                self.set_dest(swizzle, &dest_reg, &format!("log2({}.x)", src1), 4, 1);
            }

            _ => {
                self.unhandled("arithmetic ", instr);
                return Err(DecompileFail("Unhandled instruction"));
            }
        }
        Ok(())
    }

    fn compile_compare(&mut self, instr: Instruction, src1: &str, src2: &str) {
        let cmp_ops = |op: CompareOp| -> Option<(&'static str, &'static str)> {
            match op {
                CompareOp::EQUAL => Some(("==", "equal")),
                CompareOp::NOT_EQUAL => Some(("!=", "notEqual")),
                CompareOp::LESS_THAN => Some(("<", "lessThan")),
                CompareOp::LESS_EQUAL => Some(("<=", "lessThanEqual")),
                CompareOp::GREATER_THAN => Some((">", "greaterThan")),
                CompareOp::GREATER_EQUAL => Some((">=", "greaterThanEqual")),
                _ => None,
            }
        };

        let op_x = instr.common().compare_op().x();
        let op_y = instr.common().compare_op().y();

        match (cmp_ops(op_x), cmp_ops(op_y)) {
            (None, _) => {
                log::error!(target: "HW_GPU", "Unknown compare mode {:x}", op_x.0);
            }
            (_, None) => {
                log::error!(target: "HW_GPU", "Unknown compare mode {:x}", op_y.0);
            }
            (Some(x), Some(y)) => {
                if op_x != op_y {
                    self.shader.add_line(format!(
                        "conditional_code.x = {}.x {} {}.x;",
                        src1, x.0, src2
                    ));
                    self.shader.add_line(format!(
                        "conditional_code.y = {}.y {} {}.y;",
                        src1, y.0, src2
                    ));
                } else {
                    self.shader.add_line(format!(
                        "conditional_code = {}(vec2({}), vec2({}));",
                        x.1, src1, src2
                    ));
                }
            }
        }
    }

    fn compile_multiply_add(
        &mut self,
        instr: Instruction,
        swizzle: &SwizzlePattern,
    ) -> Result<(), DecompileFail> {
        let opcode = instr.opcode();
        if opcode.effective_op_code() != OpCodeId::MAD
            && opcode.effective_op_code() != OpCodeId::MADI
        {
            self.unhandled("multiply-add ", instr);
            return Err(DecompileFail("Unhandled instruction"));
        }

        let is_inverted = opcode.effective_op_code() == OpCodeId::MADI;
        let mad = instr.mad();

        let mut src1 = String::from(if swizzle.negate_src1() { "-" } else { "" });
        src1 += &self.get_source_register(mad.get_src1(is_inverted), 0);
        src1 += &format!(".{}", get_selector_src1(swizzle));

        let mut src2 = String::from(if swizzle.negate_src2() { "-" } else { "" });
        src2 += &self.get_source_register(
            mad.get_src2(is_inverted),
            u32::from(!is_inverted) * mad.address_register_index(),
        );
        src2 += &format!(".{}", get_selector_src2(swizzle));

        let mut src3 = String::from(if swizzle.negate_src3() { "-" } else { "" });
        src3 += &self.get_source_register(
            mad.get_src3(is_inverted),
            u32::from(is_inverted) * mad.address_register_index(),
        );
        src3 += &format!(".{}", get_selector_src3(swizzle));

        let dest = mad.dest();
        let dest_reg = if dest.0 < 0x10 {
            (self.outputreg_getter)(dest.get_index() as u32)
        } else if dest.0 < 0x20 {
            format!("reg_tmp{}", dest.get_index())
        } else {
            String::new()
        };

        if self.sanitize_mul {
            self.set_dest(
                swizzle,
                &dest_reg,
                &format!("sanitize_mul({}, {}) + {}", src1, src2, src3),
                4,
                4,
            );
        } else {
            self.set_dest(
                swizzle,
                &dest_reg,
                &format!("{} * {} + {}", src1, src2, src3),
                4,
                4,
            );
        }
        Ok(())
    }

    /// Compiles the instructions that are handled on their own, and returns the offset of the
    /// last instruction they cover.
    fn compile_flow_control(
        &mut self,
        instr: Instruction,
        mut offset: u32,
    ) -> Result<u32, DecompileFail> {
        let opcode = instr.opcode();
        let flow_control = instr.flow_control();
        match opcode.id() {
            OpCodeId::END => {
                self.shader.add_line("return true;");
                offset = PROGRAM_END - 1;
            }

            OpCodeId::JMPC | OpCodeId::JMPU => {
                let condition = if opcode.id() == OpCodeId::JMPC {
                    Self::evaluate_condition(flow_control)
                } else {
                    let invert_test = flow_control.num_instructions() & 1 != 0;
                    format!(
                        "{}{}",
                        if invert_test { "!" } else { "" },
                        Self::get_uniform_bool(flow_control.bool_uniform_id())
                    )
                };

                self.shader.add_line(format!("if ({}) {{", condition));
                self.shader.scope += 1;
                self.shader.add_line(format!(
                    "{{ jmp_to = {}u; break; }}",
                    flow_control.dest_offset()
                ));

                self.shader.scope -= 1;
                self.shader.add_line("}");
            }

            OpCodeId::CALL | OpCodeId::CALLC | OpCodeId::CALLU => {
                let condition = if opcode.id() == OpCodeId::CALLC {
                    Self::evaluate_condition(flow_control)
                } else if opcode.id() == OpCodeId::CALLU {
                    Self::get_uniform_bool(flow_control.bool_uniform_id())
                } else {
                    String::new()
                };

                if condition.is_empty() {
                    self.shader.add_line("{");
                } else {
                    self.shader.add_line(format!("if ({}) {{", condition));
                }
                self.shader.scope += 1;

                let call_sub = self.get_subroutine(
                    flow_control.dest_offset(),
                    flow_control.dest_offset() + flow_control.num_instructions(),
                );

                self.call_subroutine(call_sub);
                if opcode.id() == OpCodeId::CALL && call_sub.exit_method == ExitMethod::AlwaysEnd {
                    offset = PROGRAM_END - 1;
                }

                self.shader.scope -= 1;
                self.shader.add_line("}");
            }

            OpCodeId::NOP => {}

            OpCodeId::IFC | OpCodeId::IFU => {
                let condition = if opcode.id() == OpCodeId::IFC {
                    Self::evaluate_condition(flow_control)
                } else {
                    Self::get_uniform_bool(flow_control.bool_uniform_id())
                };

                let if_offset = offset + 1;
                let else_offset = flow_control.dest_offset();
                let endif_offset = flow_control.dest_offset() + flow_control.num_instructions();

                self.shader.add_line(format!("if ({}) {{", condition));
                self.shader.scope += 1;

                let if_sub = self.get_subroutine(if_offset, else_offset);
                self.call_subroutine(if_sub);
                offset = else_offset.wrapping_sub(1);

                if flow_control.num_instructions() != 0 {
                    self.shader.scope -= 1;
                    self.shader.add_line("} else {");
                    self.shader.scope += 1;

                    let else_sub = self.get_subroutine(else_offset, endif_offset);
                    self.call_subroutine(else_sub);
                    offset = endif_offset.wrapping_sub(1);

                    if if_sub.exit_method == ExitMethod::AlwaysEnd
                        && else_sub.exit_method == ExitMethod::AlwaysEnd
                    {
                        offset = PROGRAM_END - 1;
                    }
                }

                self.shader.scope -= 1;
                self.shader.add_line("}");
            }

            OpCodeId::LOOP => {
                let int_uniform = format!("uniforms.i[{}]", flow_control.int_uniform_id());

                self.shader
                    .add_line(format!("address_registers.z = int({}.y);", int_uniform));

                let loop_var = format!("loop{}", offset);
                self.shader.add_line(format!(
                    "for (uint {} = 0u; {} <= {}.x; address_registers.z += int({}.z), ++{}) {{",
                    loop_var, loop_var, int_uniform, int_uniform, loop_var
                ));
                self.shader.scope += 1;

                let loop_sub = self.get_subroutine(offset + 1, flow_control.dest_offset() + 1);
                self.call_subroutine(loop_sub);
                offset = flow_control.dest_offset();

                self.shader.scope -= 1;
                self.shader.add_line("}");

                if loop_sub.exit_method == ExitMethod::AlwaysEnd {
                    offset = PROGRAM_END - 1;
                }
            }

            OpCodeId::EMIT | OpCodeId::SETEMIT => {
                log::error!(
                    target: "HW_GPU",
                    "Geometry shader operation detected in vertex shader"
                );
            }

            _ => {
                self.unhandled("", instr);
                return Err(DecompileFail("Unhandled instruction"));
            }
        }
        Ok(offset)
    }
}
