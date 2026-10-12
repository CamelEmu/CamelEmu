use super::alu::{self, Shift, decode_imm_shift, expand_imm_c, shift, shift_c};
use crate::cpu::Cpu;
use crate::instruction::{Step, bit, bits};
use crate::state::psr;

pub fn dp_imm(cpu: &mut Cpu, raw: u32) -> Step {
    let carry_in = cpu.state().flag(psr::C);
    let (operand, carry) = expand_imm_c(bits(raw, 11, 0), carry_in);
    let n = cpu.reg(bits(raw, 19, 16));
    alu::data_processing(cpu, bits(raw, 24, 21), bit(raw, 20), bits(raw, 15, 12), n, operand, carry)
}

pub fn dp_reg(cpu: &mut Cpu, raw: u32) -> Step {
    let carry_in = cpu.state().flag(psr::C);
    let (kind, amount) = decode_imm_shift(bits(raw, 6, 5), bits(raw, 11, 7));
    let m = cpu.reg(bits(raw, 3, 0));
    let (operand, carry) = shift_c(m, kind, amount, carry_in);
    let n = cpu.reg(bits(raw, 19, 16));
    alu::data_processing(cpu, bits(raw, 24, 21), bit(raw, 20), bits(raw, 15, 12), n, operand, carry)
}

pub fn dp_rsr(cpu: &mut Cpu, raw: u32) -> Step {
    let carry_in = cpu.state().flag(psr::C);
    let amount = cpu.reg(bits(raw, 11, 8)) & 0xFF;
    let m = cpu.reg(bits(raw, 3, 0));
    let (operand, carry) = shift_c(m, Shift::from_type(bits(raw, 6, 5)), amount, carry_in);
    let n = cpu.reg(bits(raw, 19, 16));
    alu::data_processing(cpu, bits(raw, 24, 21), bit(raw, 20), bits(raw, 15, 12), n, operand, carry)
}

pub fn clz(cpu: &mut Cpu, raw: u32) -> Step {
    let m = cpu.reg(bits(raw, 3, 0));
    cpu.write_alu(bits(raw, 15, 12), m.leading_zeros())
}

fn imm16(raw: u32) -> u32 {
    (bits(raw, 19, 16) << 12) | bits(raw, 11, 0)
}

pub fn movw(cpu: &mut Cpu, raw: u32) -> Step {
    cpu.write_alu(bits(raw, 15, 12), imm16(raw))
}

pub fn movt(cpu: &mut Cpu, raw: u32) -> Step {
    let d = bits(raw, 15, 12);
    let value = (cpu.reg(d) & 0xFFFF) | (imm16(raw) << 16);
    cpu.write_alu(d, value)
}

fn field_mask(lsb: u32, msb: u32) -> u32 {
    if msb < lsb {
        return 0;
    }
    (u32::MAX >> (31 - (msb - lsb))) << lsb
}

pub fn bfc(cpu: &mut Cpu, raw: u32) -> Step {
    let d = bits(raw, 15, 12);
    let mask = field_mask(bits(raw, 11, 7), bits(raw, 20, 16));
    let value = cpu.reg(d) & !mask;
    cpu.write_alu(d, value)
}

pub fn bfi(cpu: &mut Cpu, raw: u32) -> Step {
    let d = bits(raw, 15, 12);
    let lsb = bits(raw, 11, 7);
    let mask = field_mask(lsb, bits(raw, 20, 16));
    let n = cpu.reg(bits(raw, 3, 0));
    let value = (cpu.reg(d) & !mask) | ((n << lsb) & mask);
    cpu.write_alu(d, value)
}

pub fn sbfx(cpu: &mut Cpu, raw: u32) -> Step {
    let lsb = bits(raw, 11, 7);
    let width = bits(raw, 20, 16) + 1;
    let n = cpu.reg(bits(raw, 3, 0));
    let value = if lsb + width > 32 {
        0
    } else {
        (((n << (32 - lsb - width)) as i32) >> (32 - width)) as u32
    };
    cpu.write_alu(bits(raw, 15, 12), value)
}

pub fn ubfx(cpu: &mut Cpu, raw: u32) -> Step {
    let lsb = bits(raw, 11, 7);
    let width = bits(raw, 20, 16) + 1;
    let n = cpu.reg(bits(raw, 3, 0));
    let value = if lsb + width > 32 {
        0
    } else {
        (n >> lsb) & (u32::MAX >> (32 - width))
    };
    cpu.write_alu(bits(raw, 15, 12), value)
}

pub fn sel(cpu: &mut Cpu, raw: u32) -> Step {
    let n = cpu.reg(bits(raw, 19, 16));
    let m = cpu.reg(bits(raw, 3, 0));
    let ge = cpu.state().ge();
    let mut value = 0;
    for byte in 0..4 {
        let mask = 0xFF << (byte * 8);
        value |= if ge & (1 << byte) != 0 { n & mask } else { m & mask };
    }
    cpu.write_alu(bits(raw, 15, 12), value)
}

fn sum_absolute_differences(n: u32, m: u32) -> u32 {
    (0..4)
        .map(|byte| {
            let a = (n >> (byte * 8)) & 0xFF;
            let b = (m >> (byte * 8)) & 0xFF;
            a.abs_diff(b)
        })
        .sum()
}

pub fn usad8(cpu: &mut Cpu, raw: u32) -> Step {
    let n = cpu.reg(bits(raw, 3, 0));
    let m = cpu.reg(bits(raw, 11, 8));
    cpu.write_alu(bits(raw, 19, 16), sum_absolute_differences(n, m))
}

pub fn usada8(cpu: &mut Cpu, raw: u32) -> Step {
    let n = cpu.reg(bits(raw, 3, 0));
    let m = cpu.reg(bits(raw, 11, 8));
    let a = cpu.reg(bits(raw, 15, 12));
    let value = a.wrapping_add(sum_absolute_differences(n, m));
    cpu.write_alu(bits(raw, 19, 16), value)
}

pub fn pkh(cpu: &mut Cpu, raw: u32) -> Step {
    let n = cpu.reg(bits(raw, 19, 16));
    let m = cpu.reg(bits(raw, 3, 0));
    let imm5 = bits(raw, 11, 7);
    let value = if bit(raw, 6) {
        let (kind, amount) = decode_imm_shift(2, imm5);
        (n & 0xFFFF_0000) | (shift(m, kind, amount, false) & 0xFFFF)
    } else {
        (n & 0xFFFF) | ((m << imm5) & 0xFFFF_0000)
    };
    cpu.write_alu(bits(raw, 15, 12), value)
}

pub fn rev(cpu: &mut Cpu, raw: u32) -> Step {
    let m = cpu.reg(bits(raw, 3, 0));
    cpu.write_alu(bits(raw, 15, 12), m.swap_bytes())
}

pub fn rev16(cpu: &mut Cpu, raw: u32) -> Step {
    let m = cpu.reg(bits(raw, 3, 0));
    let value = ((m & 0x00FF_00FF) << 8) | ((m & 0xFF00_FF00) >> 8);
    cpu.write_alu(bits(raw, 15, 12), value)
}

pub fn revsh(cpu: &mut Cpu, raw: u32) -> Step {
    let m = cpu.reg(bits(raw, 3, 0));
    let value = (m as u16).swap_bytes() as i16 as i32 as u32;
    cpu.write_alu(bits(raw, 15, 12), value)
}

pub fn rbit(cpu: &mut Cpu, raw: u32) -> Step {
    let m = cpu.reg(bits(raw, 3, 0));
    cpu.write_alu(bits(raw, 15, 12), m.reverse_bits())
}

/// SXTB, SXTH, SXTB16, UXTB, UXTH, UXTB16 and the variants that add, selected by the bits 22:20.
/// The register 15 in the bits 19:16 means no addition.
pub fn extend(cpu: &mut Cpu, raw: u32) -> Step {
    let rotated = cpu.reg(bits(raw, 3, 0)).rotate_right(bits(raw, 11, 10) * 8);
    let n = bits(raw, 19, 16);
    let add = if n == 15 { 0 } else { cpu.reg(n) };
    let value = match bits(raw, 22, 20) {
        0b000 => add_halves(add, sign_extend_bytes16(rotated)),
        0b010 => add.wrapping_add(rotated as u8 as i8 as i32 as u32),
        0b011 => add.wrapping_add(rotated as u16 as i16 as i32 as u32),
        0b100 => add_halves(add, rotated & 0x00FF_00FF),
        0b110 => add.wrapping_add(rotated & 0xFF),
        _ => add.wrapping_add(rotated & 0xFFFF),
    };
    cpu.write_alu(bits(raw, 15, 12), value)
}

fn sign_extend_bytes16(value: u32) -> u32 {
    let low = value as u8 as i8 as i16 as u16 as u32;
    let high = (value >> 16) as u8 as i8 as i16 as u16 as u32;
    low | (high << 16)
}

fn add_halves(a: u32, b: u32) -> u32 {
    let low = (a as u16).wrapping_add(b as u16) as u32;
    let high = ((a >> 16) as u16).wrapping_add((b >> 16) as u16) as u32;
    low | (high << 16)
}

fn saturate_operand(cpu: &mut Cpu, raw: u32) -> i64 {
    let n = cpu.reg(bits(raw, 3, 0));
    let (kind, amount) = decode_imm_shift(if bit(raw, 6) { 2 } else { 0 }, bits(raw, 11, 7));
    shift(n, kind, amount, false) as i32 as i64
}

pub fn ssat(cpu: &mut Cpu, raw: u32) -> Step {
    let operand = saturate_operand(cpu, raw);
    let (value, saturated) = alu::signed_saturate(operand, bits(raw, 20, 16) + 1);
    if saturated {
        cpu.state().cpsr |= psr::Q;
    }
    cpu.write_alu(bits(raw, 15, 12), value)
}

pub fn usat(cpu: &mut Cpu, raw: u32) -> Step {
    let operand = saturate_operand(cpu, raw);
    let (value, saturated) = alu::unsigned_saturate(operand, bits(raw, 20, 16));
    if saturated {
        cpu.state().cpsr |= psr::Q;
    }
    cpu.write_alu(bits(raw, 15, 12), value)
}

fn saturate16(cpu: &mut Cpu, raw: u32, saturate: impl Fn(i64) -> (u32, bool)) -> Step {
    let n = cpu.reg(bits(raw, 3, 0));
    let (low, low_saturated) = saturate(n as u16 as i16 as i64);
    let (high, high_saturated) = saturate((n >> 16) as u16 as i16 as i64);
    if low_saturated || high_saturated {
        cpu.state().cpsr |= psr::Q;
    }
    cpu.write_alu(bits(raw, 15, 12), (low & 0xFFFF) | (high << 16))
}

pub fn ssat16(cpu: &mut Cpu, raw: u32) -> Step {
    let width = bits(raw, 19, 16) + 1;
    saturate16(cpu, raw, |value| alu::signed_saturate(value, width))
}

pub fn usat16(cpu: &mut Cpu, raw: u32) -> Step {
    let width = bits(raw, 19, 16);
    saturate16(cpu, raw, |value| alu::unsigned_saturate(value, width))
}

/// QADD, QSUB, QDADD and QDSUB, selected by the bits 22:21.
pub fn saturating_add_subtract(cpu: &mut Cpu, raw: u32) -> Step {
    let m = cpu.reg(bits(raw, 3, 0)) as i32 as i64;
    let n = cpu.reg(bits(raw, 19, 16)) as i32 as i64;
    let mut saturated = false;
    let mut doubled = |value: i64| {
        let (value, sat) = alu::signed_saturate(value * 2, 32);
        saturated |= sat;
        value as i32 as i64
    };
    let result = match bits(raw, 22, 21) {
        0b00 => m + n,
        0b01 => m - n,
        0b10 => m + doubled(n),
        _ => m - doubled(n),
    };
    let (value, sat) = alu::signed_saturate(result, 32);
    if saturated || sat {
        cpu.state().cpsr |= psr::Q;
    }
    cpu.write_alu(bits(raw, 15, 12), value)
}
