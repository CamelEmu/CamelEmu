//! Translates a decoded block into AArch64 code.
//!
//! The generated function takes the CPU in x0, the registers in x1 and the remaining ticks in x2,
//! and returns a `Step`. While it runs, x19 holds the CPU, x21 the registers, x22 the remaining
//! ticks and x23 the entries of the page table. The guest registers stay in memory between the
//! instructions, so the calls into Rust need no spilling.

use std::mem::offset_of;

use super::assembler::*;
use crate::arm::alu::{Shift, decode_imm_shift, expand_imm_c, opcode};
use crate::cpu::Cpu;
use crate::exec::{self, Block, Decoded};
use crate::instruction::{Kind, Step, bit, bits};
use crate::state::{CpuState, psr};

const CPSR: u32 = offset_of!(CpuState, cpsr) as u32;

fn reg_offset(register: u32) -> u32 {
    offset_of!(CpuState, regs) as u32 + register * 4
}

extern "C" fn jit_execute(cpu: *mut Cpu, decoded: *const Decoded) -> u32 {
    let (cpu, decoded) = unsafe { (&mut *cpu, &*decoded) };
    exec::execute(cpu, decoded) as u32
}

extern "C" fn jit_read8(cpu: *mut Cpu, address: u32) -> u32 {
    unsafe { &mut *cpu }.read8(address) as u32
}

extern "C" fn jit_read32(cpu: *mut Cpu, address: u32) -> u32 {
    unsafe { &mut *cpu }.read32(address)
}

extern "C" fn jit_write8(cpu: *mut Cpu, address: u32, value: u32) {
    unsafe { &mut *cpu }.write8(address, value as u8);
}

extern "C" fn jit_write32(cpu: *mut Cpu, address: u32, value: u32) {
    unsafe { &mut *cpu }.write32(address, value);
}

/// How the shifter carry of an operand is found.
enum ShifterCarry {
    /// The carry flag stays as it is.
    Unchanged,
    /// The carry is a constant.
    Constant(bool),
    /// The carry is in w14.
    Computed,
}

struct Emitter<'a> {
    asm: Assembler,
    block: &'a Block,
    /// The cycles of the native instructions that have not been taken from the ticks yet.
    pending_cycles: u32,
    epilogue: Label,
}

impl Emitter<'_> {
    /// Takes the pending cycles from the ticks, and keeps counting them for the code that follows
    /// unless `reset`.
    fn charge(&mut self, reset: bool) {
        let cycles = self.pending_cycles;
        if cycles == 0 {
            return;
        }
        self.asm.ldr_x(X9, X22, 0);
        if cycles < 4096 {
            self.asm.sub_imm_x(X9, X9, cycles);
        } else {
            self.asm.mov_imm32(X10, cycles);
            self.asm.sub_x(X9, X9, X10);
        }
        self.asm.str_x(X9, X22, 0);
        if reset {
            self.pending_cycles = 0;
        }
    }

    fn call(&mut self, function: usize) {
        self.asm.ldr_literal(X16, function as u64);
        self.asm.blr(X16);
    }

    /// Returns `step` with the PC already written.
    fn leave(&mut self, step: Step) {
        self.charge(false);
        self.asm.mov_imm32(X0, step as u32);
        self.asm.b(self.epilogue);
    }

    /// Loads a guest register, where the PC reads as a constant.
    fn load_reg(&mut self, rd: Reg, register: u32, decoded: &Decoded) {
        if register == 15 {
            self.asm.mov_imm32(rd, decoded.address.wrapping_add(8));
        } else {
            self.asm.ldr_w(rd, X21, reg_offset(register));
        }
    }

    fn store_reg(&mut self, rs: Reg, register: u32) {
        self.asm.str_w(rs, X21, reg_offset(register));
    }

    /// Skips the code that follows when the condition fails, and returns the label to bind after it.
    fn check_condition(&mut self, decoded: &Decoded) -> Option<Label> {
        if decoded.condition >= 0b1110 {
            return None;
        }
        let skip = self.asm.new_label();
        self.asm.ldr_w(X9, X21, CPSR);
        self.asm.msr_nzcv(X9);
        self.asm.b_cond(decoded.condition ^ 1, skip);
        Some(skip)
    }

    fn end_condition(&mut self, skip: Option<Label>) {
        if let Some(skip) = skip {
            self.asm.bind(skip);
        }
    }

    /// Calls the execution of the table for an instruction that has no native translation.
    fn fallback(&mut self, decoded: &Decoded) {
        self.charge(true);
        self.asm.mov_x(X0, X19);
        self.asm.ldr_literal(X1, decoded as *const Decoded as u64);
        self.call(jit_execute as *const () as usize);
        self.asm.cbnz_w(X0, self.epilogue);
    }

    /// Computes the shifted register operand of `raw` into w11, with Rm in w8.
    fn shifted_operand(&mut self, raw: u32, decoded: &Decoded, need_carry: bool) -> ShifterCarry {
        self.load_reg(X8, bits(raw, 3, 0), decoded);
        let (kind, amount) = decode_imm_shift(bits(raw, 6, 5), bits(raw, 11, 7));
        let carry = if !need_carry || (kind == Shift::Lsl && amount == 0) {
            ShifterCarry::Unchanged
        } else {
            let bit = match kind {
                Shift::Lsl => 32 - amount,
                Shift::Rrx => 0,
                _ => amount - 1,
            };
            self.asm.ubfx_w(X14, X8, bit, 1);
            ShifterCarry::Computed
        };
        match (kind, amount) {
            (Shift::Lsl, 0) => self.asm.mov_w(X11, X8),
            (Shift::Lsl, amount) => self.asm.lsl_imm_w(X11, X8, amount),
            (Shift::Lsr, 32) => self.asm.mov_w(X11, ZR),
            (Shift::Lsr, amount) => self.asm.lsr_imm_w(X11, X8, amount),
            (Shift::Asr, 32) => self.asm.asr_imm_w(X11, X8, 31),
            (Shift::Asr, amount) => self.asm.asr_imm_w(X11, X8, amount),
            (Shift::Ror, amount) => self.asm.ror_imm_w(X11, X8, amount),
            (Shift::Rrx, _) => {
                self.asm.ldr_w(X13, X21, CPSR);
                self.asm.ubfx_w(X13, X13, 29, 1);
                self.asm.extr_w(X11, X13, X8, 1);
            }
        }
        carry
    }

    /// The data-processing instructions with an immediate or a register shifted by an immediate.
    fn data_processing(&mut self, decoded: &Decoded) -> bool {
        let raw = decoded.raw;
        let register_shift = !bit(raw, 25) && bit(raw, 4);
        if register_shift {
            return false;
        }
        let op = bits(raw, 24, 21);
        let set_flags = bit(raw, 20);
        let d = bits(raw, 15, 12);
        let n = bits(raw, 19, 16);
        let test = (opcode::TST..=opcode::CMN).contains(&op);
        if d == 15 && !test {
            return false;
        }
        let logical = matches!(
            op,
            opcode::AND | opcode::EOR | opcode::TST | opcode::TEQ | opcode::ORR | opcode::MOV | opcode::BIC | opcode::MVN
        );

        self.pending_cycles += decoded.cycles;
        let skip = self.check_condition(decoded);

        let carry = if bit(raw, 25) {
            let imm12 = bits(raw, 11, 0);
            let (value, carry) = expand_imm_c(imm12, false);
            self.asm.mov_imm32(X11, value);
            if bits(imm12, 11, 8) == 0 {
                ShifterCarry::Unchanged
            } else {
                ShifterCarry::Constant(carry)
            }
        } else {
            self.shifted_operand(raw, decoded, set_flags && logical)
        };
        if !matches!(op, opcode::MOV | opcode::MVN) {
            self.load_reg(X12, n, decoded);
        }
        if matches!(op, opcode::ADC | opcode::SBC | opcode::RSC) {
            self.asm.ldr_w(X13, X21, CPSR);
            self.asm.msr_nzcv(X13);
        }

        let s = set_flags;
        match op {
            opcode::AND | opcode::TST => self.asm.and_w(X0, X12, X11),
            opcode::EOR | opcode::TEQ => self.asm.eor_w(X0, X12, X11),
            opcode::ORR => self.asm.orr_w(X0, X12, X11),
            opcode::BIC => self.asm.bic_w(X0, X12, X11),
            opcode::MOV => self.asm.mov_w(X0, X11),
            opcode::MVN => self.asm.orn_w(X0, ZR, X11),
            opcode::SUB | opcode::CMP if s => self.asm.subs_w(X0, X12, X11),
            opcode::SUB => self.asm.sub_w(X0, X12, X11),
            opcode::RSB if s => self.asm.subs_w(X0, X11, X12),
            opcode::RSB => self.asm.sub_w(X0, X11, X12),
            opcode::ADD | opcode::CMN if s => self.asm.adds_w(X0, X12, X11),
            opcode::ADD => self.asm.add_w(X0, X12, X11),
            opcode::ADC if s => self.asm.adcs_w(X0, X12, X11),
            opcode::ADC => self.asm.adc_w(X0, X12, X11),
            opcode::SBC if s => self.asm.sbcs_w(X0, X12, X11),
            opcode::SBC => self.asm.sbc_w(X0, X12, X11),
            _ if s => self.asm.sbcs_w(X0, X11, X12),
            _ => self.asm.sbc_w(X0, X11, X12),
        }

        if s {
            if logical {
                // (N << 1) | Z, then the carry below it when it changes.
                self.asm.lsr_imm_w(X9, X0, 31);
                self.asm.cmp_imm_w(X0, 0);
                self.asm.cset_w(X13, COND_EQ);
                self.asm.orr_w_shifted(X9, X13, X9, ShiftKind::Lsl, 1);
                self.asm.ldr_w(X10, X21, CPSR);
                match carry {
                    ShifterCarry::Unchanged => self.asm.bfi_w(X10, X9, 30, 2),
                    ShifterCarry::Constant(carry) => {
                        self.asm.mov_imm32(X14, carry as u32);
                        self.asm.orr_w_shifted(X9, X14, X9, ShiftKind::Lsl, 1);
                        self.asm.bfi_w(X10, X9, 29, 3);
                    }
                    ShifterCarry::Computed => {
                        self.asm.orr_w_shifted(X9, X14, X9, ShiftKind::Lsl, 1);
                        self.asm.bfi_w(X10, X9, 29, 3);
                    }
                }
            } else {
                self.asm.mrs_nzcv(X9);
                self.asm.lsr_imm_w(X9, X9, 28);
                self.asm.ldr_w(X10, X21, CPSR);
                self.asm.bfi_w(X10, X9, 28, 4);
            }
            self.asm.str_w(X10, X21, CPSR);
        }
        if !test {
            self.store_reg(X0, d);
        }
        self.end_condition(skip);
        true
    }

    /// Reads or writes the guest memory at w24 through the page table, and through Rust for the
    /// pages without a pointer and in big-endian state. A load leaves the value in w0, and a
    /// store takes it from w26.
    fn memory_access(&mut self, load: bool, byte: bool) {
        let slow = self.asm.new_label();
        let done = self.asm.new_label();
        self.asm.ldr_w(X9, X21, CPSR);
        self.asm.tbnz_w(X9, 9, slow);
        self.asm.lsr_imm_w(X9, X24, 12);
        self.asm.ldr_x_index(X9, X23, X9);
        self.asm.cbz_x(X9, slow);
        self.asm.ubfx_w(X10, X24, 0, 12);
        match (load, byte) {
            (true, true) => self.asm.ldrb_reg(X0, X9, X10),
            (true, false) => self.asm.ldr_w_reg(X0, X9, X10),
            (false, true) => self.asm.strb_reg(X26, X9, X10),
            (false, false) => self.asm.str_w_reg(X26, X9, X10),
        }
        self.asm.b(done);
        self.asm.bind(slow);
        self.asm.mov_x(X0, X19);
        self.asm.mov_w(X1, X24);
        let function = match (load, byte) {
            (true, true) => jit_read8 as *const () as usize,
            (true, false) => jit_read32 as *const () as usize,
            (false, true) => {
                self.asm.mov_w(X2, X26);
                jit_write8 as *const () as usize
            }
            (false, false) => {
                self.asm.mov_w(X2, X26);
                jit_write32 as *const () as usize
            }
        };
        self.call(function);
        self.asm.bind(done);
    }

    /// LDR, LDRB, STR and STRB with an immediate offset or a register shifted by an immediate.
    fn load_store(&mut self, decoded: &Decoded) -> bool {
        let raw = decoded.raw;
        let register_offset = bit(raw, 25);
        let (pre, up, byte, write_back, load) =
            (bit(raw, 24), bit(raw, 23), bit(raw, 22), bit(raw, 21), bit(raw, 20));
        let n = bits(raw, 19, 16);
        let t = bits(raw, 15, 12);
        let writes_base = !pre || write_back;
        if (load && t == 15) || (n == 15 && writes_base) {
            return false;
        }
        if register_offset && (bits(raw, 3, 0) == 15 || decode_imm_shift(bits(raw, 6, 5), bits(raw, 11, 7)).0 == Shift::Rrx) {
            return false;
        }

        self.pending_cycles += decoded.cycles;
        let skip = self.check_condition(decoded);

        if n == 15 {
            self.asm.mov_imm32(X12, decoded.address.wrapping_add(8) & !3);
        } else {
            self.asm.ldr_w(X12, X21, reg_offset(n));
        }
        if register_offset {
            self.shifted_operand(raw, decoded, false);
            if up {
                self.asm.add_w(X25, X12, X11);
            } else {
                self.asm.sub_w(X25, X12, X11);
            }
        } else if up {
            self.asm.add_imm_w(X25, X12, bits(raw, 11, 0));
        } else {
            self.asm.sub_imm_w(X25, X12, bits(raw, 11, 0));
        }
        self.asm.mov_w(X24, if pre { X25 } else { X12 });

        if load {
            self.memory_access(true, byte);
            if writes_base {
                self.store_reg(X25, n);
            }
            self.store_reg(X0, t);
        } else {
            self.load_reg(X26, t, decoded);
            self.memory_access(false, byte);
            if writes_base {
                self.store_reg(X25, n);
            }
        }
        self.end_condition(skip);
        true
    }

    /// B and BL, which write the PC and leave the block when they are taken.
    fn branch(&mut self, decoded: &Decoded, link: bool) -> bool {
        let raw = decoded.raw;
        let offset = (((raw << 8) as i32) >> 6) as u32;
        let target = decoded.address.wrapping_add(8).wrapping_add(offset);

        self.pending_cycles += decoded.cycles;
        let skip = self.check_condition(decoded);
        if link {
            self.asm.mov_imm32(X9, decoded.address.wrapping_add(4));
            self.store_reg(X9, 14);
        }
        self.asm.mov_imm32(X9, target);
        self.store_reg(X9, 15);
        self.leave(Step::Branch);
        self.end_condition(skip);
        true
    }

    fn native(&mut self, decoded: &Decoded) -> bool {
        if self.block.location.thumb {
            return false;
        }
        let raw = decoded.raw;
        match decoded.instruction.kind {
            Kind::DataProcessing => self.data_processing(decoded),
            Kind::LoadStore if bits(raw, 27, 26) == 0b01 && !(bit(raw, 25) && bit(raw, 4)) => {
                self.load_store(decoded)
            }
            Kind::Branch if bits(raw, 27, 25) == 0b101 && raw >> 28 != 0b1111 => {
                self.branch(decoded, bit(raw, 24))
            }
            _ => false,
        }
    }
}

/// Translates the block, reading the page table at `page_table_entries`.
pub fn emit(block: &Block, page_table_entries: u64) -> Option<Vec<u32>> {
    let mut asm = Assembler::new();
    let epilogue = asm.new_label();

    asm.stp_x_pre(X29, X30, SP, -16);
    asm.stp_x_pre(X19, X20, SP, -16);
    asm.stp_x_pre(X21, X22, SP, -16);
    asm.stp_x_pre(X23, X24, SP, -16);
    asm.stp_x_pre(X25, X26, SP, -16);
    asm.mov_x(X19, X0);
    asm.mov_x(X21, X1);
    asm.mov_x(X22, X2);
    asm.ldr_literal(X23, page_table_entries);

    let mut emitter = Emitter {
        asm,
        block,
        pending_cycles: 0,
        epilogue,
    };
    for decoded in &block.instructions {
        if !emitter.native(decoded) {
            emitter.fallback(decoded);
        }
    }
    let end_pc = block.end;
    emitter.asm.mov_imm32(X9, end_pc);
    emitter.store_reg(X9, 15);
    emitter.charge(true);
    emitter.asm.mov_imm32(X0, Step::Continue as u32);

    let mut asm = emitter.asm;
    asm.bind(epilogue);
    asm.ldp_x_post(X25, X26, SP, 16);
    asm.ldp_x_post(X23, X24, SP, 16);
    asm.ldp_x_post(X21, X22, SP, 16);
    asm.ldp_x_post(X19, X20, SP, 16);
    asm.ldp_x_post(X29, X30, SP, 16);
    asm.ret();
    asm.finish()
}

// The flags are merged into CPSR by their positions.
const _: () = assert!(psr::N == 1 << 31 && psr::C == 1 << 29 && psr::V == 1 << 28 && psr::E == 1 << 9);
