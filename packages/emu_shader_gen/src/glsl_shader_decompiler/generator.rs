use emu_pica_types::nihstro::shader_bytecode::{
    DestRegister, FlowControl, FlowControlOp, Instruction, RegisterType, SourceRegister,
    SwizzlePattern,
};

use super::control_flow::{ExitMethod, Subroutine, Subroutines};
use super::writer::ShaderWriter;
use super::{DecompileFail, PROGRAM_END, ProgramCode, RegGetter, SwizzleData};

pub(super) struct GlslGenerator<'a, 'b> {
    pub subroutines: &'a Subroutines,
    pub program_code: &'a ProgramCode,
    pub swizzle_data: &'a SwizzleData,
    pub main_offset: u32,
    pub inputreg_getter: RegGetter<'b>,
    pub outputreg_getter: RegGetter<'b>,
    pub sanitize_mul: bool,

    pub shader: ShaderWriter,
}

impl<'a, 'b> GlslGenerator<'a, 'b> {
    pub fn new(
        subroutines: &'a Subroutines,
        program_code: &'a ProgramCode,
        swizzle_data: &'a SwizzleData,
        main_offset: u32,
        inputreg_getter: RegGetter<'b>,
        outputreg_getter: RegGetter<'b>,
        sanitize_mul: bool,
    ) -> Self {
        Self {
            subroutines,
            program_code,
            swizzle_data,
            main_offset,
            inputreg_getter,
            outputreg_getter,
            sanitize_mul,
            shader: ShaderWriter::default(),
        }
    }

    /// Generates the code, which the constructor of the core does, and returns it.
    pub fn move_shader_code(mut self) -> Result<String, DecompileFail> {
        self.generate()?;
        Ok(self.shader.move_result())
    }

    /// Gets the Subroutine object corresponding to the specified address.
    pub fn get_subroutine(&self, begin: u32, end: u32) -> &'a Subroutine {
        self.subroutines
            .get(&(begin, end))
            .expect("Assertion Failed!")
    }

    /// Generates condition evaluation code for the flow control instruction.
    pub fn evaluate_condition(flow_control: FlowControl) -> String {
        let result_x = if flow_control.refx() {
            "conditional_code.x"
        } else {
            "!conditional_code.x"
        };
        let result_y = if flow_control.refy() {
            "conditional_code.y"
        } else {
            "!conditional_code.y"
        };

        match flow_control.op() {
            FlowControlOp::JUST_X => result_x.to_string(),
            FlowControlOp::JUST_Y => result_y.to_string(),
            FlowControlOp::OR | FlowControlOp::AND => {
                let and_or = if flow_control.op() == FlowControlOp::OR {
                    "any"
                } else {
                    "all"
                };
                let bvec = if flow_control.refx() && flow_control.refy() {
                    "conditional_code".to_string()
                } else if !flow_control.refx() && !flow_control.refy() {
                    "not(conditional_code)".to_string()
                } else {
                    format!("bvec2({}, {})", result_x, result_y)
                };
                format!("{}({})", and_or, bvec)
            }
            _ => unreachable!(),
        }
    }

    /// Generates code representing a source register.
    pub fn get_source_register(
        &mut self,
        source_reg: SourceRegister,
        address_register_index: u32,
    ) -> String {
        let index = source_reg.get_index() as u32;

        match source_reg.get_register_type() {
            RegisterType::Input => (self.inputreg_getter)(index),
            RegisterType::Temporary => format!("reg_tmp{}", index),
            RegisterType::FloatUniform => {
                if address_register_index != 0 {
                    return format!(
                        "get_offset_register({}, address_registers.{})",
                        index,
                        ['x', 'y', 'z'][address_register_index as usize - 1]
                    );
                }
                format!("uniforms.f[{}]", index)
            }
            _ => unreachable!(),
        }
    }

    /// Generates code representing a destination register.
    pub fn get_dest_register(&mut self, dest_reg: DestRegister) -> String {
        let index = dest_reg.get_index() as u32;

        match dest_reg.get_register_type() {
            RegisterType::Output => (self.outputreg_getter)(index),
            RegisterType::Temporary => format!("reg_tmp{}", index),
            _ => unreachable!(),
        }
    }

    /// Generates code representing a bool uniform
    pub fn get_uniform_bool(index: u32) -> String {
        format!("uniforms.b[{}]", index)
    }

    /**
     * Adds code that calls a subroutine.
     * @param subroutine the subroutine to call.
     */
    pub fn call_subroutine(&mut self, subroutine: &Subroutine) {
        if subroutine.exit_method == ExitMethod::AlwaysEnd {
            self.shader
                .add_line(format!("{}();", subroutine.get_name()));
            self.shader.add_line("return true;");
        } else if subroutine.exit_method == ExitMethod::Conditional {
            self.shader.add_line(format!(
                "if ({}()) {{ return true; }}",
                subroutine.get_name()
            ));
        } else {
            self.shader
                .add_line(format!("{}();", subroutine.get_name()));
        }
    }

    /**
     * Writes code that does an assignment operation.
     * @param swizzle the swizzle data of the current instruction.
     * @param reg the destination register code.
     * @param value the code representing the value to assign.
     * @param dest_num_components number of components of the destination register.
     * @param value_num_components number of components of the value to assign.
     */
    pub fn set_dest(
        &mut self,
        swizzle: &SwizzlePattern,
        reg: &str,
        value: &str,
        dest_num_components: u32,
        value_num_components: u32,
    ) {
        let mut dest_mask_num_components = 0;
        let mut dest_mask_swizzle = String::from(".");

        for i in 0..dest_num_components {
            if swizzle.dest_component_enabled(i) {
                dest_mask_swizzle.push(['x', 'y', 'z', 'w'][i as usize]);
                dest_mask_num_components += 1;
            }
        }

        if reg.is_empty() || dest_mask_num_components == 0 {
            return;
        }
        debug_assert!(value_num_components >= dest_num_components || value_num_components == 1);

        let dest = format!(
            "{}{}",
            reg,
            if dest_num_components != 1 {
                dest_mask_swizzle.as_str()
            } else {
                ""
            }
        );

        let mut src = value.to_string();
        if value_num_components == 1 {
            if dest_mask_num_components != 1 {
                src = format!("vec{}({})", dest_mask_num_components, value);
            }
        } else if value_num_components != dest_mask_num_components {
            src = format!("({}){}", value, dest_mask_swizzle);
        }

        self.shader.add_line(format!("{} = {};", dest, src));
    }

    /// Logs an instruction that the decompiler does not handle.
    pub fn unhandled(&self, kind: &str, instr: Instruction) {
        let opcode = instr.opcode();
        log::error!(
            target: "HW_GPU",
            "Unhandled {}instruction: 0x{:02x} ({}): 0x{:08x}",
            kind,
            opcode.effective_op_code().0,
            opcode.get_info().name,
            instr.hex
        );
    }

    /**
     * Compiles a range of instructions from PICA to GLSL.
     * @param begin the offset of the starting instruction.
     * @param end the offset where the compilation should stop (exclusive).
     * @return the offset of the next instruction to compile. PROGRAM_END if the program terminates.
     */
    fn compile_range(&mut self, begin: u32, end: u32) -> Result<u32, DecompileFail> {
        let mut program_counter = begin;
        while program_counter < if begin > end { PROGRAM_END } else { end } {
            program_counter = self.compile_instr(program_counter)?;
        }
        Ok(program_counter)
    }

    fn generate(&mut self) -> Result<(), DecompileFail> {
        if self.sanitize_mul {
            self.shader
                .add_line("vec4 sanitize_mul(vec4 lhs, vec4 rhs) {");
            self.shader.scope += 1;
            self.shader.add_line("vec4 product = lhs * rhs;");
            self.shader.add_line(
                "return mix(product, mix(mix(vec4(0.0), product, isnan(rhs)), product, \
                 isnan(lhs)), isnan(product));",
            );
            self.shader.scope -= 1;
            self.shader.add_line("}\n");
        }

        self.shader
            .add_line("vec4 get_offset_register(int base_index, int offset) {");
        self.shader.scope += 1;
        self.shader
            .add_line("int fixed_offset = offset >= -128 && offset <= 127 ? offset : 0;");
        self.shader
            .add_line("uint index = uint((base_index + fixed_offset) & 0x7F);");
        self.shader
            .add_line("return index < 96u ? uniforms.f[index] : vec4(1.0);");
        self.shader.scope -= 1;
        self.shader.add_line("}\n");

        // Add declarations for registers
        self.shader
            .add_line("bvec2 conditional_code = bvec2(false);");
        self.shader.add_line("ivec3 address_registers = ivec3(0);");
        for i in 0..16 {
            self.shader
                .add_line(format!("vec4 reg_tmp{} = vec4(0.0, 0.0, 0.0, 1.0);", i));
        }
        self.shader.add_new_line();

        // Add declarations for all subroutines
        for subroutine in self.subroutines.values() {
            self.shader
                .add_line(format!("bool {}();", subroutine.get_name()));
        }
        self.shader.add_new_line();

        // Add the main entry point
        self.shader.add_line("bool exec_shader() {");
        self.shader.scope += 1;
        let main = self.get_subroutine(self.main_offset, PROGRAM_END);
        self.call_subroutine(main);
        self.shader.scope -= 1;
        self.shader.add_line("}\n");

        // Add definitions for all subroutines
        for subroutine in self.subroutines.values() {
            self.generate_subroutine(subroutine)?;
        }
        Ok(())
    }

    fn generate_subroutine(&mut self, subroutine: &Subroutine) -> Result<(), DecompileFail> {
        let mut labels = subroutine.labels.clone();

        self.shader
            .add_line(format!("bool {}() {{", subroutine.get_name()));
        self.shader.scope += 1;

        if labels.is_empty() {
            if self.compile_range(subroutine.begin, subroutine.end)? != PROGRAM_END {
                self.shader.add_line("return false;");
            }
        } else {
            labels.insert(subroutine.begin);
            self.shader
                .add_line(format!("uint jmp_to = {}u;", subroutine.begin));
            self.shader.add_line("while (true) {");
            self.shader.scope += 1;

            self.shader.add_line("switch (jmp_to) {");

            let mut label = labels.first().copied();
            while let Some(current) = label {
                self.shader.add_line(format!("case {}u: {{", current));
                self.shader.scope += 1;

                let next_label = labels
                    .range(current + 1..)
                    .next()
                    .copied()
                    .unwrap_or(subroutine.end);

                let compile_end = self.compile_range(current, next_label)?;
                if compile_end > next_label && compile_end != PROGRAM_END {
                    // This happens only when there is a label inside a IF/LOOP block
                    self.shader
                        .add_line(format!("{{ jmp_to = {}u; break; }}", compile_end));
                    labels.insert(compile_end);
                }

                self.shader.scope -= 1;
                self.shader.add_line("}");

                label = labels.range(current + 1..).next().copied();
            }

            self.shader.add_line("default: return false;");
            self.shader.add_line("}");

            self.shader.scope -= 1;
            self.shader.add_line("}");

            self.shader.add_line("return false;");
        }

        self.shader.scope -= 1;
        self.shader.add_line("}\n");

        debug_assert!(self.shader.scope == 0);
        Ok(())
    }
}
