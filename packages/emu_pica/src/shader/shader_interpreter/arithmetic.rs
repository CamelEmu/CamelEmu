use emu_pica_types::{F24, Vec4};

use super::Interpreter;
use crate::nihstro::shader_bytecode::{
    CompareOp, DestRegister, Instruction, OpCodeId, OpCodeInfo, RegisterType, Selector,
    SourceRegister, SwizzlePattern,
};
use crate::shader_unit::ShaderUnit;
use crate::vector::component_mut;

// Constants for handling invalid inputs
const DUMMY_VEC4_FLOAT24_ZEROS: [F24; 4] = [F24::zero(); 4];
const DUMMY_VEC4_FLOAT24_ONES: [F24; 4] = [F24::one(); 4];

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

impl Interpreter<'_> {
    fn lookup_source_register(
        &self,
        source_reg: SourceRegister,
        address_register_index: u32,
    ) -> [F24; 4] {
        let mut index = source_reg.get_index();
        match source_reg.get_register_type() {
            RegisterType::Input => to_array(&self.state.input[index as usize]),

            RegisterType::Temporary => to_array(&self.state.temporary[index as usize]),

            RegisterType::FloatUniform => {
                if address_register_index != 0 {
                    let mut offset =
                        self.state.address_registers[address_register_index as usize - 1];
                    if offset < i32::from(i8::MIN) || offset > i32::from(i8::MAX) {
                        offset = 0;
                    }
                    index = (index + offset) & 0x7F;
                    // If the index is above 96, the result is all one.
                    if index >= 96 {
                        return DUMMY_VEC4_FLOAT24_ONES;
                    }
                }
                to_array(&self.setup.uniforms.f[index as usize])
            }

            _ => DUMMY_VEC4_FLOAT24_ZEROS,
        }
    }

    pub(super) fn run_arithmetic(&mut self, instr: Instruction, swizzle: SwizzlePattern) {
        let opcode = instr.opcode();
        let is_inverted = 0 != (opcode.get_info().subtype & OpCodeInfo::SRC_INVERSED);

        let common = instr.common();
        let src1_ = self.lookup_source_register(
            common.get_src1(is_inverted),
            u32::from(!is_inverted) * common.address_register_index(),
        );
        let src2_ = self.lookup_source_register(
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

        let state = &mut *self.state;
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
                    *component_mut(dest, i) = F24::from_float32(src1[i].to_float32().floor());
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
                    *component_mut(dest, i) = if src1[i] > src2[i] { src1[i] } else { src2[i] };
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
                    *component_mut(dest, i) = if src1[i] < src2[i] { src1[i] } else { src2[i] };
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
                    state.address_registers[i as usize] = src1[i as usize].to_float32() as i32;
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

    pub(super) fn run_multiply_add(&mut self, instr: Instruction) {
        let opcode = instr.opcode();
        if opcode.effective_op_code() == OpCodeId::MAD
            || opcode.effective_op_code() == OpCodeId::MADI
        {
            let mad = instr.mad();
            let mad_swizzle = SwizzlePattern {
                hex: self.setup.swizzle_data[mad.operand_desc_id() as usize],
            };

            let is_inverted = opcode.effective_op_code() == OpCodeId::MADI;

            let src1_ = self.lookup_source_register(mad.get_src1(is_inverted), 0);
            let src2_ = self.lookup_source_register(
                mad.get_src2(is_inverted),
                u32::from(!is_inverted) * mad.address_register_index(),
            );
            let src3_ = self.lookup_source_register(
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
            let dest = dest_register(self.state, mad.dest(), &mut dummy);

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
}
