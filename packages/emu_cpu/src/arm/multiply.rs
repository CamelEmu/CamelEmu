use crate::cpu::Cpu;
use crate::instruction::{Step, bit, bits};
use crate::state::psr;

/// The registers of a multiply: d is 19:16, a is 15:12, m is 11:8 and n is 3:0.
fn operands(cpu: &mut Cpu, raw: u32) -> (u32, u32) {
    (cpu.reg(bits(raw, 3, 0)), cpu.reg(bits(raw, 11, 8)))
}

/// Writes a 64-bit result to RdHi in the bits 19:16 and RdLo in the bits 15:12.
fn write_long(cpu: &mut Cpu, raw: u32, value: u64) {
    cpu.set_reg(bits(raw, 15, 12), value as u32);
    cpu.set_reg(bits(raw, 19, 16), (value >> 32) as u32);
}

/// Reads the 64-bit accumulator from RdHi and RdLo.
fn accumulator_long(cpu: &mut Cpu, raw: u32) -> u64 {
    let low = cpu.reg(bits(raw, 15, 12)) as u64;
    let high = cpu.reg(bits(raw, 19, 16)) as u64;
    low | (high << 32)
}

fn set_q(cpu: &mut Cpu) {
    cpu.state().cpsr |= psr::Q;
}

/// MUL: Rn * Rm.
pub fn mul(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let value = n.wrapping_mul(m);
    if bit(raw, 20) {
        cpu.state().set_nz(value);
    }
    cpu.set_reg(bits(raw, 19, 16), value);
    Step::Continue
}

/// MLA: Rn * Rm + Ra.
pub fn mla(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let value = n.wrapping_mul(m).wrapping_add(cpu.reg(bits(raw, 15, 12)));
    if bit(raw, 20) {
        cpu.state().set_nz(value);
    }
    cpu.set_reg(bits(raw, 19, 16), value);
    Step::Continue
}

/// MLS: Ra - Rn * Rm.
pub fn mls(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let value = cpu.reg(bits(raw, 15, 12)).wrapping_sub(n.wrapping_mul(m));
    cpu.set_reg(bits(raw, 19, 16), value);
    Step::Continue
}

/// UMULL, UMLAL, SMULL and SMLAL, selected by the bits 22:21.
pub fn multiply_long(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let signed = bit(raw, 22);
    let product = if signed {
        (n as i32 as i64).wrapping_mul(m as i32 as i64) as u64
    } else {
        (n as u64) * (m as u64)
    };
    let value = if bit(raw, 21) {
        product.wrapping_add(accumulator_long(cpu, raw))
    } else {
        product
    };
    if bit(raw, 20) {
        let state = cpu.state();
        state.set_flag(psr::N, value >> 63 != 0);
        state.set_flag(psr::Z, value == 0);
    }
    write_long(cpu, raw, value);
    Step::Continue
}

/// UMAAL: the 64-bit Rn * Rm + RdLo + RdHi.
pub fn umaal(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let low = cpu.reg(bits(raw, 15, 12)) as u64;
    let high = cpu.reg(bits(raw, 19, 16)) as u64;
    write_long(cpu, raw, (n as u64) * (m as u64) + low + high);
    Step::Continue
}

/// The signed upper or lower halfword of `value`.
fn half(value: u32, top: bool) -> i64 {
    (if top { value >> 16 } else { value }) as u16 as i16 as i64
}

/// SMLA<x><y>, where the bit 5 selects the half of Rn and the bit 6 the half of Rm.
pub fn smlaxy(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let product = half(n, bit(raw, 5)) * half(m, bit(raw, 6));
    let value = product + cpu.reg(bits(raw, 15, 12)) as i32 as i64;
    if value != value as i32 as i64 {
        set_q(cpu);
    }
    cpu.set_reg(bits(raw, 19, 16), value as u32);
    Step::Continue
}

/// SMUL<x><y>: SMLA<x><y> without the accumulator.
pub fn smulxy(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let value = half(n, bit(raw, 5)) * half(m, bit(raw, 6));
    cpu.set_reg(bits(raw, 19, 16), value as u32);
    Step::Continue
}

/// SMLAL<x><y>: SMLA<x><y> with a 64-bit accumulator.
pub fn smlalxy(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let product = half(n, bit(raw, 5)) * half(m, bit(raw, 6));
    let value = accumulator_long(cpu, raw).wrapping_add(product as u64);
    write_long(cpu, raw, value);
    Step::Continue
}

/// SMLAW<y> and SMULW<y>, where the bit 6 selects the half of Rm and the bit 5 tells SMULW<y>.
pub fn smlawy(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let product = ((n as i32 as i64) * half(m, bit(raw, 6))) >> 16;
    if bit(raw, 5) {
        cpu.set_reg(bits(raw, 19, 16), product as u32);
        return Step::Continue;
    }
    let value = product + cpu.reg(bits(raw, 15, 12)) as i32 as i64;
    if value != value as i32 as i64 {
        set_q(cpu);
    }
    cpu.set_reg(bits(raw, 19, 16), value as u32);
    Step::Continue
}

/// SMMUL, SMMLA and SMMLS, where the bit 5 rounds, the bits 7:6 tell SMMLS and the register 15
/// in the bits 15:12 tells SMMUL.
pub fn smmul(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let product = (n as i32 as i64) * (m as i32 as i64);
    let a = bits(raw, 15, 12);
    let accumulator = if a == 15 && !bit(raw, 6) {
        0
    } else {
        (cpu.reg(a) as i32 as i64) << 32
    };
    let mut value = if bit(raw, 6) {
        accumulator.wrapping_sub(product)
    } else {
        accumulator.wrapping_add(product)
    };
    if bit(raw, 5) {
        value = value.wrapping_add(0x8000_0000);
    }
    cpu.set_reg(bits(raw, 19, 16), (value >> 32) as u32);
    Step::Continue
}

/// The two products of a dual multiply, where the bit 5 swaps the halves of Rm.
fn dual_products(cpu: &mut Cpu, raw: u32) -> (i64, i64) {
    let (n, mut m) = operands(cpu, raw);
    if bit(raw, 5) {
        m = m.rotate_right(16);
    }
    (half(n, false) * half(m, false), half(n, true) * half(m, true))
}

/// SMLAD, SMLSD, SMUAD and SMUSD, where the bit 6 subtracts and the register 15 in the bits
/// 15:12 means no accumulator.
pub fn smlad(cpu: &mut Cpu, raw: u32) -> Step {
    let (low, high) = dual_products(cpu, raw);
    let a = bits(raw, 15, 12);
    let accumulator = if a == 15 { 0 } else { cpu.reg(a) as i32 as i64 };
    let value = if bit(raw, 6) { low - high } else { low + high } + accumulator;
    let smusd = a == 15 && bit(raw, 6);
    if value != value as i32 as i64 && !smusd {
        set_q(cpu);
    }
    cpu.set_reg(bits(raw, 19, 16), value as u32);
    Step::Continue
}

/// SMLALD and SMLSLD, where the bit 6 subtracts.
pub fn smlald(cpu: &mut Cpu, raw: u32) -> Step {
    let (low, high) = dual_products(cpu, raw);
    let product = if bit(raw, 6) { low - high } else { low + high };
    let value = accumulator_long(cpu, raw).wrapping_add(product as u64);
    write_long(cpu, raw, value);
    Step::Continue
}

/// SDIV: the signed Rn / Rm, which is 0 when Rm is 0.
pub fn sdiv(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    let value = if m == 0 {
        0
    } else {
        (n as i32).wrapping_div(m as i32) as u32
    };
    cpu.set_reg(bits(raw, 19, 16), value);
    Step::Continue
}

/// UDIV: the unsigned Rn / Rm, which is 0 when Rm is 0.
pub fn udiv(cpu: &mut Cpu, raw: u32) -> Step {
    let (n, m) = operands(cpu, raw);
    cpu.set_reg(bits(raw, 19, 16), n.checked_div(m).unwrap_or(0));
    Step::Continue
}
