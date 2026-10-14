//! The instructions of the Thumb state. The ARM11 has the 16-bit instructions of ARMv6 and the
//! 32-bit BL and BLX.

use crate::arm::alu::{Shift, add_with_carry, decode_imm_shift, shift_c};
use crate::arm::system::{bkpt, nop, supervisor_call, undefined};
use crate::cpu::Cpu;
use crate::instruction::{Instruction, Step, bit, bits, inst};
use crate::state::psr;

fn set_nzcv(cpu: &mut Cpu, (result, carry, overflow): (u32, bool, bool)) -> u32 {
    cpu.state().set_nzcv(result, carry, overflow);
    result
}

fn set_nzc(cpu: &mut Cpu, result: u32, carry: bool) -> u32 {
    let state = cpu.state();
    state.set_nz(result);
    state.set_flag(psr::C, carry);
    result
}

/// LSL, LSR and ASR with an immediate, selected by the bits 12:11.
pub fn shift_imm(cpu: &mut Cpu, raw: u32) -> Step {
    let (kind, amount) = decode_imm_shift(bits(raw, 12, 11), bits(raw, 10, 6));
    let carry = cpu.state().flag(psr::C);
    let (result, carry) = shift_c(cpu.reg(bits(raw, 5, 3)), kind, amount, carry);
    let result = set_nzc(cpu, result, carry);
    cpu.set_reg(bits(raw, 2, 0), result);
    Step::Continue
}

/// ADD and SUB with three low registers or with a 3-bit immediate. The bit 10 selects the
/// immediate and the bit 9 subtracts.
pub fn add_sub_3(cpu: &mut Cpu, raw: u32) -> Step {
    let n = cpu.reg(bits(raw, 5, 3));
    let operand = if bit(raw, 10) {
        bits(raw, 8, 6)
    } else {
        cpu.reg(bits(raw, 8, 6))
    };
    let result = if bit(raw, 9) {
        add_with_carry(n, !operand, true)
    } else {
        add_with_carry(n, operand, false)
    };
    let result = set_nzcv(cpu, result);
    cpu.set_reg(bits(raw, 2, 0), result);
    Step::Continue
}

/// MOV, CMP, ADD and SUB with an 8-bit immediate, selected by the bits 12:11.
pub fn imm8(cpu: &mut Cpu, raw: u32) -> Step {
    let d = bits(raw, 10, 8);
    let imm = bits(raw, 7, 0);
    let n = cpu.reg(d);
    match bits(raw, 12, 11) {
        0b00 => {
            let carry = cpu.state().flag(psr::C);
            set_nzc(cpu, imm, carry);
            cpu.set_reg(d, imm);
        }
        0b01 => {
            set_nzcv(cpu, add_with_carry(n, !imm, true));
        }
        0b10 => {
            let result = set_nzcv(cpu, add_with_carry(n, imm, false));
            cpu.set_reg(d, result);
        }
        _ => {
            let result = set_nzcv(cpu, add_with_carry(n, !imm, true));
            cpu.set_reg(d, result);
        }
    }
    Step::Continue
}

/// The data-processing instructions on two low registers, selected by the bits 9:6.
pub fn alu(cpu: &mut Cpu, raw: u32) -> Step {
    let d = bits(raw, 2, 0);
    let a = cpu.reg(d);
    let m = cpu.reg(bits(raw, 5, 3));
    let carry = cpu.state().flag(psr::C);
    let shift_by = |cpu: &mut Cpu, kind: Shift| {
        let (result, carry) = shift_c(a, kind, m & 0xFF, carry);
        Some(set_nzc(cpu, result, carry))
    };
    let result = match bits(raw, 9, 6) {
        0b0000 => Some(set_nzc(cpu, a & m, carry)),
        0b0001 => Some(set_nzc(cpu, a ^ m, carry)),
        0b0010 => shift_by(cpu, Shift::Lsl),
        0b0011 => shift_by(cpu, Shift::Lsr),
        0b0100 => shift_by(cpu, Shift::Asr),
        0b0101 => Some(set_nzcv(cpu, add_with_carry(a, m, carry))),
        0b0110 => Some(set_nzcv(cpu, add_with_carry(a, !m, carry))),
        0b0111 => shift_by(cpu, Shift::Ror),
        0b1000 => {
            set_nzc(cpu, a & m, carry);
            None
        }
        0b1001 => Some(set_nzcv(cpu, add_with_carry(!m, 0, true))),
        0b1010 => {
            set_nzcv(cpu, add_with_carry(a, !m, true));
            None
        }
        0b1011 => {
            set_nzcv(cpu, add_with_carry(a, m, false));
            None
        }
        0b1100 => Some(set_nzc(cpu, a | m, carry)),
        0b1101 => {
            let result = a.wrapping_mul(m);
            cpu.state().set_nz(result);
            Some(result)
        }
        0b1110 => Some(set_nzc(cpu, a & !m, carry)),
        _ => Some(set_nzc(cpu, !m, carry)),
    };
    if let Some(result) = result {
        cpu.set_reg(d, result);
    }
    Step::Continue
}

/// The register of the bits 2:0 with the bit 7 on top, which reaches R8-R15.
fn high_register(raw: u32) -> u32 {
    ((bit(raw, 7) as u32) << 3) | bits(raw, 2, 0)
}

/// ADD with high registers, which does not set the flags.
pub fn add_high(cpu: &mut Cpu, raw: u32) -> Step {
    let d = high_register(raw);
    let value = cpu.reg(d).wrapping_add(cpu.reg(bits(raw, 6, 3)));
    cpu.write_alu(d, value)
}

/// CMP with high registers.
pub fn cmp_high(cpu: &mut Cpu, raw: u32) -> Step {
    let n = cpu.reg(high_register(raw));
    let m = cpu.reg(bits(raw, 6, 3));
    set_nzcv(cpu, add_with_carry(n, !m, true));
    Step::Continue
}

/// MOV with high registers, which does not set the flags.
pub fn mov_high(cpu: &mut Cpu, raw: u32) -> Step {
    let value = cpu.reg(bits(raw, 6, 3));
    cpu.write_alu(high_register(raw), value)
}

/// BX: branches to the address in Rm, whose bit 0 selects the instruction set.
pub fn bx(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(bits(raw, 6, 3));
    cpu.bx_write_pc(target)
}

/// BLX with a register: calls the address in Rm, whose bit 0 selects the instruction set.
pub fn blx_reg(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(bits(raw, 6, 3));
    let return_address = cpu.pc().wrapping_add(2) | 1;
    cpu.set_reg(14, return_address);
    cpu.bx_write_pc(target)
}

/// The PC aligned to a word, which the PC-relative loads and ADR use in Thumb state.
fn aligned_pc(cpu: &mut Cpu) -> u32 {
    cpu.reg(15) & !3
}

/// LDR relative to the PC.
pub fn ldr_literal(cpu: &mut Cpu, raw: u32) -> Step {
    let address = aligned_pc(cpu).wrapping_add(bits(raw, 7, 0) * 4);
    let value = cpu.read32(address);
    cpu.set_reg(bits(raw, 10, 8), value);
    Step::Continue
}

/// The loads and stores with a register offset, selected by the bits 11:9.
pub fn load_store_reg(cpu: &mut Cpu, raw: u32) -> Step {
    let address = cpu.reg(bits(raw, 5, 3)).wrapping_add(cpu.reg(bits(raw, 8, 6)));
    let t = bits(raw, 2, 0);
    let value = cpu.reg(t);
    let loaded = match bits(raw, 11, 9) {
        0b000 => {
            cpu.write32(address, value);
            None
        }
        0b001 => {
            cpu.write16(address, value as u16);
            None
        }
        0b010 => {
            cpu.write8(address, value as u8);
            None
        }
        0b011 => Some(cpu.read8(address) as i8 as i32 as u32),
        0b100 => Some(cpu.read32(address)),
        0b101 => Some(cpu.read16(address) as u32),
        0b110 => Some(cpu.read8(address) as u32),
        _ => Some(cpu.read16(address) as i16 as i32 as u32),
    };
    if let Some(loaded) = loaded {
        cpu.set_reg(t, loaded);
    }
    Step::Continue
}

/// LDR, STR, LDRB and STRB with a 5-bit immediate. The bit 12 selects a byte and the bit 11 loads.
pub fn load_store_imm(cpu: &mut Cpu, raw: u32) -> Step {
    let byte = bit(raw, 12);
    let offset = bits(raw, 10, 6) * if byte { 1 } else { 4 };
    let address = cpu.reg(bits(raw, 5, 3)).wrapping_add(offset);
    let t = bits(raw, 2, 0);
    match (bit(raw, 11), byte) {
        (true, true) => {
            let value = cpu.read8(address) as u32;
            cpu.set_reg(t, value);
        }
        (true, false) => {
            let value = cpu.read32(address);
            cpu.set_reg(t, value);
        }
        (false, true) => {
            let value = cpu.reg(t);
            cpu.write8(address, value as u8);
        }
        (false, false) => {
            let value = cpu.reg(t);
            cpu.write32(address, value);
        }
    }
    Step::Continue
}

/// LDRH and STRH with a 5-bit immediate, where the bit 11 loads.
pub fn load_store_half(cpu: &mut Cpu, raw: u32) -> Step {
    let address = cpu.reg(bits(raw, 5, 3)).wrapping_add(bits(raw, 10, 6) * 2);
    let t = bits(raw, 2, 0);
    if bit(raw, 11) {
        let value = cpu.read16(address) as u32;
        cpu.set_reg(t, value);
    } else {
        let value = cpu.reg(t);
        cpu.write16(address, value as u16);
    }
    Step::Continue
}

/// LDR and STR relative to SP, where the bit 11 loads.
pub fn load_store_sp(cpu: &mut Cpu, raw: u32) -> Step {
    let address = cpu.reg(13).wrapping_add(bits(raw, 7, 0) * 4);
    let t = bits(raw, 10, 8);
    if bit(raw, 11) {
        let value = cpu.read32(address);
        cpu.set_reg(t, value);
    } else {
        let value = cpu.reg(t);
        cpu.write32(address, value);
    }
    Step::Continue
}

/// ADR: an address relative to the PC.
pub fn adr(cpu: &mut Cpu, raw: u32) -> Step {
    let value = aligned_pc(cpu).wrapping_add(bits(raw, 7, 0) * 4);
    cpu.set_reg(bits(raw, 10, 8), value);
    Step::Continue
}

/// ADD of SP and an immediate into a low register.
pub fn add_sp_t1(cpu: &mut Cpu, raw: u32) -> Step {
    let value = cpu.reg(13).wrapping_add(bits(raw, 7, 0) * 4);
    cpu.set_reg(bits(raw, 10, 8), value);
    Step::Continue
}

/// ADD and SUB on SP with a 7-bit immediate, where the bit 7 subtracts.
pub fn adjust_sp(cpu: &mut Cpu, raw: u32) -> Step {
    let offset = bits(raw, 6, 0) * 4;
    let sp = cpu.reg(13);
    let value = if bit(raw, 7) {
        sp.wrapping_sub(offset)
    } else {
        sp.wrapping_add(offset)
    };
    cpu.set_reg(13, value);
    Step::Continue
}

/// SXTH, SXTB, UXTH and UXTB, selected by the bits 7:6.
pub fn extend(cpu: &mut Cpu, raw: u32) -> Step {
    let m = cpu.reg(bits(raw, 5, 3));
    let value = match bits(raw, 7, 6) {
        0b00 => m as u16 as i16 as i32 as u32,
        0b01 => m as u8 as i8 as i32 as u32,
        0b10 => m & 0xFFFF,
        _ => m & 0xFF,
    };
    cpu.set_reg(bits(raw, 2, 0), value);
    Step::Continue
}

/// REV, REV16 and REVSH, selected by the bits 7:6.
pub fn reverse(cpu: &mut Cpu, raw: u32) -> Step {
    let m = cpu.reg(bits(raw, 5, 3));
    let value = match bits(raw, 7, 6) {
        0b00 => m.swap_bytes(),
        0b01 => ((m & 0x00FF_00FF) << 8) | ((m & 0xFF00_FF00) >> 8),
        _ => (m as u16).swap_bytes() as i16 as i32 as u32,
    };
    cpu.set_reg(bits(raw, 2, 0), value);
    Step::Continue
}

/// SETEND in Thumb state, where the bit 3 selects big endian.
pub fn setend(cpu: &mut Cpu, raw: u32) -> Step {
    cpu.state().set_flag(psr::E, bit(raw, 3));
    Step::Continue
}

/// PUSH, where the bit 8 adds LR.
pub fn push(cpu: &mut Cpu, raw: u32) -> Step {
    let list = bits(raw, 7, 0) | ((bit(raw, 8) as u32) << 14);
    let sp = cpu.reg(13);
    let mut address = sp.wrapping_sub(list.count_ones() * 4);
    cpu.set_reg(13, address);
    for register in 0..15 {
        if list & (1 << register) != 0 {
            let value = cpu.reg(register);
            cpu.write32(address, value);
            address = address.wrapping_add(4);
        }
    }
    Step::Continue
}

/// POP, where the bit 8 adds PC.
pub fn pop(cpu: &mut Cpu, raw: u32) -> Step {
    let list = bits(raw, 7, 0) | ((bit(raw, 8) as u32) << 15);
    let mut address = cpu.reg(13);
    cpu.set_reg(13, address.wrapping_add(list.count_ones() * 4));
    let mut step = Step::Continue;
    for register in 0..16 {
        if list & (1 << register) == 0 {
            continue;
        }
        let value = cpu.read32(address);
        address = address.wrapping_add(4);
        if register == 15 {
            step = cpu.bx_write_pc(value);
        } else {
            cpu.set_reg(register, value);
        }
    }
    step
}

/// STMIA and LDMIA, where the bit 11 loads. A load does not write the base back if it loads it.
pub fn load_store_multiple(cpu: &mut Cpu, raw: u32) -> Step {
    let n = bits(raw, 10, 8);
    let list = bits(raw, 7, 0);
    let mut address = cpu.reg(n);
    let new_base = address.wrapping_add(list.count_ones() * 4);
    let load = bit(raw, 11);
    for register in 0..8 {
        if list & (1 << register) == 0 {
            continue;
        }
        if load {
            let value = cpu.read32(address);
            cpu.set_reg(register, value);
        } else {
            let value = cpu.reg(register);
            cpu.write32(address, value);
        }
        address = address.wrapping_add(4);
    }
    if !load || list & (1 << n) == 0 {
        cpu.set_reg(n, new_base);
    }
    Step::Continue
}

/// CBZ and CBNZ: branch forward when Rn is zero, or not zero with the bit 11.
pub fn cbz_cbnz(cpu: &mut Cpu, raw: u32) -> Step {
    let value = cpu.reg(bits(raw, 2, 0));
    if (value == 0) == bit(raw, 11) {
        return Step::Continue;
    }
    let offset = (((bit(raw, 9) as u32) << 5) | bits(raw, 7, 3)) << 1;
    let target = cpu.reg(15).wrapping_add(offset);
    cpu.branch_write_pc(target)
}

/// B with a condition, which checks it by itself as the block does not.
pub fn b_conditional(cpu: &mut Cpu, raw: u32) -> Step {
    if !cpu.state().condition_passed(bits(raw, 11, 8)) {
        return Step::Continue;
    }
    let offset = ((raw << 24) as i32 >> 23) as u32;
    let target = cpu.reg(15).wrapping_add(offset);
    cpu.branch_write_pc(target)
}

/// B: branches relative to the PC.
pub fn b(cpu: &mut Cpu, raw: u32) -> Step {
    let offset = ((raw << 21) as i32 >> 20) as u32;
    let target = cpu.reg(15).wrapping_add(offset);
    cpu.branch_write_pc(target)
}

/// SVC in Thumb state, with the 8-bit immediate.
pub fn svc(cpu: &mut Cpu, raw: u32) -> Step {
    supervisor_call(cpu, bits(raw, 7, 0))
}

/// The first half of BL and BLX when it runs on its own, which keeps the upper part of the
/// offset in LR.
pub fn bl_prefix(cpu: &mut Cpu, raw: u32) -> Step {
    let offset = ((raw << 21) as i32 >> 9) as u32;
    let value = cpu.reg(15).wrapping_add(offset);
    cpu.set_reg(14, value);
    Step::Continue
}

/// The second half of BL when it runs on its own.
pub fn bl_suffix(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(14).wrapping_add(bits(raw, 10, 0) << 1);
    let return_address = cpu.pc().wrapping_add(2) | 1;
    cpu.set_reg(14, return_address);
    cpu.branch_write_pc(target)
}

/// The second half of BLX when it runs on its own.
pub fn blx_suffix(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(14).wrapping_add(bits(raw, 10, 0) << 1) & !3;
    let return_address = cpu.pc().wrapping_add(2) | 1;
    cpu.set_reg(14, return_address);
    cpu.state().cpsr &= !psr::T;
    cpu.state().regs[15] = target;
    Step::Branch
}

/// The offset of a 32-bit BL or BLX, of which `raw` holds the first half in the upper bits.
fn long_branch_offset(raw: u32) -> u32 {
    let s = bit(raw, 26);
    let i1 = !(bit(raw, 13) ^ s);
    let i2 = !(bit(raw, 11) ^ s);
    let offset = ((s as u32) << 24)
        | ((i1 as u32) << 23)
        | ((i2 as u32) << 22)
        | (bits(raw, 25, 16) << 12)
        | (bits(raw, 10, 0) << 1);
    ((offset << 7) as i32 >> 7) as u32
}

/// BL: calls Thumb code relative to the PC.
pub fn bl(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(15).wrapping_add(long_branch_offset(raw));
    let return_address = cpu.pc().wrapping_add(4) | 1;
    cpu.set_reg(14, return_address);
    cpu.branch_write_pc(target)
}

/// BLX with an immediate: calls ARM code relative to the PC.
pub fn blx(cpu: &mut Cpu, raw: u32) -> Step {
    let target = (cpu.reg(15) & !3).wrapping_add(long_branch_offset(raw));
    let return_address = cpu.pc().wrapping_add(4) | 1;
    cpu.set_reg(14, return_address);
    cpu.state().cpsr &= !psr::T;
    cpu.state().regs[15] = target & !3;
    Step::Branch
}

#[rustfmt::skip]
pub static TABLE: &[Instruction] = &[
    // Shift (immediate) add, subtract, move and compare instructions
    inst!("LSL (imm)",               DataProcessing,      "00000vvvvvmmmddd", shift_imm, 1),
    inst!("LSR (imm)",               DataProcessing,      "00001vvvvvmmmddd", shift_imm, 1),
    inst!("ASR (imm)",               DataProcessing,      "00010vvvvvmmmddd", shift_imm, 1),
    inst!("ADD (reg, T1)",           DataProcessing,      "0001100mmmnnnddd", add_sub_3, 1),
    inst!("SUB (reg)",               DataProcessing,      "0001101mmmnnnddd", add_sub_3, 1),
    inst!("ADD (imm, T1)",           DataProcessing,      "0001110vvvnnnddd", add_sub_3, 1),
    inst!("SUB (imm, T1)",           DataProcessing,      "0001111vvvnnnddd", add_sub_3, 1),
    inst!("MOV (imm)",               DataProcessing,      "00100dddvvvvvvvv", imm8, 1),
    inst!("CMP (imm)",               DataProcessing,      "00101nnnvvvvvvvv", imm8, 1),
    inst!("ADD (imm, T2)",           DataProcessing,      "00110dddvvvvvvvv", imm8, 1),
    inst!("SUB (imm, T2)",           DataProcessing,      "00111dddvvvvvvvv", imm8, 1),

    // Data-processing instructions
    inst!("AND (reg)",               DataProcessing,      "0100000000mmmddd", alu, 1),
    inst!("EOR (reg)",               DataProcessing,      "0100000001mmmddd", alu, 1),
    inst!("LSL (reg)",               DataProcessing,      "0100000010mmmddd", alu, 1),
    inst!("LSR (reg)",               DataProcessing,      "0100000011mmmddd", alu, 1),
    inst!("ASR (reg)",               DataProcessing,      "0100000100mmmddd", alu, 1),
    inst!("ADC (reg)",               DataProcessing,      "0100000101mmmddd", alu, 1),
    inst!("SBC (reg)",               DataProcessing,      "0100000110mmmddd", alu, 1),
    inst!("ROR (reg)",               DataProcessing,      "0100000111sssddd", alu, 1),
    inst!("TST (reg)",               DataProcessing,      "0100001000mmmnnn", alu, 1),
    inst!("RSB (imm)",               DataProcessing,      "0100001001nnnddd", alu, 1),
    inst!("CMP (reg, T1)",           DataProcessing,      "0100001010mmmnnn", alu, 1),
    inst!("CMN (reg)",               DataProcessing,      "0100001011mmmnnn", alu, 1),
    inst!("ORR (reg)",               DataProcessing,      "0100001100mmmddd", alu, 1),
    inst!("MUL (reg)",               Multiply,            "0100001101nnnddd", alu, 1),
    inst!("BIC (reg)",               DataProcessing,      "0100001110mmmddd", alu, 1),
    inst!("MVN (reg)",               DataProcessing,      "0100001111mmmddd", alu, 1),

    // Special data instructions
    inst!("ADD (reg, T2)",           DataProcessing,      "01000100Dmmmmddd", add_high, 1),
    inst!("CMP (reg, T2)",           DataProcessing,      "01000101Nmmmmnnn", cmp_high, 1),
    inst!("MOV (reg)",               DataProcessing,      "01000110Dmmmmddd", mov_high, 1),

    // Store/Load single data item instructions
    inst!("LDR (literal)",           LoadStore,           "01001tttvvvvvvvv", ldr_literal, 1),
    inst!("STR (reg)",               LoadStore,           "0101000mmmnnnttt", load_store_reg, 1),
    inst!("STRH (reg)",              LoadStore,           "0101001mmmnnnttt", load_store_reg, 1),
    inst!("STRB (reg)",              LoadStore,           "0101010mmmnnnttt", load_store_reg, 1),
    inst!("LDRSB (reg)",             LoadStore,           "0101011mmmnnnttt", load_store_reg, 1),
    inst!("LDR (reg)",               LoadStore,           "0101100mmmnnnttt", load_store_reg, 1),
    inst!("LDRH (reg)",              LoadStore,           "0101101mmmnnnttt", load_store_reg, 1),
    inst!("LDRB (reg)",              LoadStore,           "0101110mmmnnnttt", load_store_reg, 1),
    inst!("LDRSH (reg)",             LoadStore,           "0101111mmmnnnttt", load_store_reg, 1),
    inst!("STR (imm, T1)",           LoadStore,           "01100vvvvvnnnttt", load_store_imm, 1),
    inst!("LDR (imm, T1)",           LoadStore,           "01101vvvvvnnnttt", load_store_imm, 1),
    inst!("STRB (imm)",              LoadStore,           "01110vvvvvnnnttt", load_store_imm, 1),
    inst!("LDRB (imm)",              LoadStore,           "01111vvvvvnnnttt", load_store_imm, 1),
    inst!("STRH (imm)",              LoadStore,           "10000vvvvvnnnttt", load_store_half, 1),
    inst!("LDRH (imm)",              LoadStore,           "10001vvvvvnnnttt", load_store_half, 1),
    inst!("STR (imm, T2)",           LoadStore,           "10010tttvvvvvvvv", load_store_sp, 1),
    inst!("LDR (imm, T2)",           LoadStore,           "10011tttvvvvvvvv", load_store_sp, 1),

    // Generate relative address instructions
    inst!("ADR",                     DataProcessing,      "10100dddvvvvvvvv", adr, 1),
    inst!("ADD (SP plus imm, T1)",   DataProcessing,      "10101dddvvvvvvvv", add_sp_t1, 1),
    inst!("ADD (SP plus imm, T2)",   DataProcessing,      "101100000vvvvvvv", adjust_sp, 1),
    inst!("SUB (SP minus imm)",      DataProcessing,      "101100001vvvvvvv", adjust_sp, 1),

    // Hint instructions
    inst!("SEV",                     Hint,                "1011111101000000", nop, 1),
    inst!("SEVL",                    Hint,                "1011111101010000", nop, 1),
    inst!("WFE",                     Hint,                "1011111100100000", nop, 1),
    inst!("WFI",                     Hint,                "1011111100110000", nop, 1),
    inst!("YIELD",                   Hint,                "1011111100010000", nop, 1),
    inst!("NOP",                     Hint,                "10111111----0000", nop, 1),

    // IT instruction, which the ARM11 does not have
    inst!("IT",                      Undefined,           "10111111iiiiiiii", undefined, 1),

    // Miscellaneous 16-bit instructions
    inst!("SXTH",                    Extension,           "1011001000mmmddd", extend, 1),
    inst!("SXTB",                    Extension,           "1011001001mmmddd", extend, 1),
    inst!("UXTH",                    Extension,           "1011001010mmmddd", extend, 1),
    inst!("UXTB",                    Extension,           "1011001011mmmddd", extend, 1),
    inst!("PUSH",                    LoadStoreMultiple,   "1011010Mxxxxxxxx", push, 1),
    inst!("POP",                     LoadStoreMultiple,   "1011110Pxxxxxxxx", pop, 1),
    inst!("SETEND",                  StatusRegister,      "101101100101x000", setend, 1),
    inst!("CPS",                     StatusRegister,      "10110110011m0aif", nop, 1),
    inst!("REV",                     Reversal,            "1011101000mmmddd", reverse, 1),
    inst!("REV16",                   Reversal,            "1011101001mmmddd", reverse, 1),
    inst!("REVSH",                   Reversal,            "1011101011mmmddd", reverse, 1),
    inst!("BKPT",                    ExceptionGenerating, "10111110xxxxxxxx", bkpt, 1),

    // Store/Load multiple registers
    inst!("STMIA",                   LoadStoreMultiple,   "11000nnnxxxxxxxx", load_store_multiple, 1),
    inst!("LDMIA",                   LoadStoreMultiple,   "11001nnnxxxxxxxx", load_store_multiple, 1),

    // Branch instructions
    inst!("BX",                      Branch,              "010001110mmmm000", bx, 1),
    inst!("BLX (reg)",               Branch,              "010001111mmmm000", blx_reg, 1),
    inst!("CBZ/CBNZ",                Branch,              "1011o0i1iiiiinnn", cbz_cbnz, 1),
    inst!("UDF",                     Undefined,           "11011110--------", undefined, 1),
    inst!("SVC",                     ExceptionGenerating, "11011111xxxxxxxx", svc, 1),
    inst!("B (T1)",                  Branch,              "1101ccccvvvvvvvv", b_conditional, 1),
    inst!("B (T2)",                  Branch,              "11100vvvvvvvvvvv", b, 1),
    inst!("BL (prefix)",             DataProcessing,      "11110vvvvvvvvvvv", bl_prefix, 1),
    inst!("BL (suffix)",             Branch,              "11111vvvvvvvvvvv", bl_suffix, 1),
    inst!("BLX (suffix)",            Branch,              "11101vvvvvvvvvvv", blx_suffix, 1),
];

/// The 32-bit instructions, with the first half in the upper bits.
#[rustfmt::skip]
pub static TABLE32: &[Instruction] = &[
    inst!("BL (imm)",                Branch,              "11110Svvvvvvvvvv11j1jvvvvvvvvvvv", bl, 1),
    inst!("BLX (imm)",               Branch,              "11110Svvvvvvvvvv11j0jvvvvvvvvvvv", blx, 1),
];
