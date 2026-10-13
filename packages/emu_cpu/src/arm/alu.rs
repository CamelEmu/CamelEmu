use crate::cpu::Cpu;
use crate::instruction::bits;
use crate::state::psr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shift {
    Lsl,
    Lsr,
    Asr,
    Ror,
    Rrx,
}

impl Shift {
    pub fn from_type(shift_type: u32) -> Self {
        match shift_type & 3 {
            0 => Shift::Lsl,
            1 => Shift::Lsr,
            2 => Shift::Asr,
            _ => Shift::Ror,
        }
    }
}

/// Returns the result and the carry out of `x + y + carry`, and whether it overflowed.
pub fn add_with_carry(x: u32, y: u32, carry: bool) -> (u32, bool, bool) {
    let sum = x as u64 + y as u64 + carry as u64;
    let result = sum as u32;
    let overflow = ((x ^ result) & (y ^ result)) >> 31 != 0;
    (result, sum > u32::MAX as u64, overflow)
}

/// Expands the modified immediate of an ARM data-processing instruction.
pub fn expand_imm_c(imm12: u32, carry_in: bool) -> (u32, bool) {
    let rotation = bits(imm12, 11, 8) * 2;
    let value = (imm12 & 0xFF).rotate_right(rotation);
    let carry = if rotation == 0 { carry_in } else { value >> 31 != 0 };
    (value, carry)
}

/// Decodes the shift of an immediate shift, in which 0 means 32 or RRX.
pub fn decode_imm_shift(shift_type: u32, imm5: u32) -> (Shift, u32) {
    match Shift::from_type(shift_type) {
        Shift::Lsl => (Shift::Lsl, imm5),
        Shift::Lsr => (Shift::Lsr, if imm5 == 0 { 32 } else { imm5 }),
        Shift::Asr => (Shift::Asr, if imm5 == 0 { 32 } else { imm5 }),
        _ if imm5 == 0 => (Shift::Rrx, 1),
        _ => (Shift::Ror, imm5),
    }
}

/// Shifts `value` by `amount`, which may be any value of the bottom byte of a register.
pub fn shift_c(value: u32, shift: Shift, amount: u32, carry_in: bool) -> (u32, bool) {
    if amount == 0 && shift != Shift::Rrx {
        return (value, carry_in);
    }
    match shift {
        Shift::Lsl => match amount {
            1..=31 => (value << amount, (value >> (32 - amount)) & 1 != 0),
            32 => (0, value & 1 != 0),
            _ => (0, false),
        },
        Shift::Lsr => match amount {
            1..=31 => (value >> amount, (value >> (amount - 1)) & 1 != 0),
            32 => (0, value >> 31 != 0),
            _ => (0, false),
        },
        Shift::Asr => match amount {
            1..=31 => (
                ((value as i32) >> amount) as u32,
                (value >> (amount - 1)) & 1 != 0,
            ),
            _ => (((value as i32) >> 31) as u32, value >> 31 != 0),
        },
        Shift::Ror => {
            let result = value.rotate_right(amount % 32);
            (result, result >> 31 != 0)
        }
        Shift::Rrx => (((carry_in as u32) << 31) | (value >> 1), value & 1 != 0),
    }
}

pub fn shift(value: u32, shift: Shift, amount: u32, carry_in: bool) -> u32 {
    shift_c(value, shift, amount, carry_in).0
}

/// The opcodes of the bits 24:21 of a data-processing instruction.
pub mod opcode {
    pub const AND: u32 = 0;
    pub const EOR: u32 = 1;
    pub const SUB: u32 = 2;
    pub const RSB: u32 = 3;
    pub const ADD: u32 = 4;
    pub const ADC: u32 = 5;
    pub const SBC: u32 = 6;
    pub const RSC: u32 = 7;
    pub const TST: u32 = 8;
    pub const TEQ: u32 = 9;
    pub const CMP: u32 = 10;
    pub const CMN: u32 = 11;
    pub const ORR: u32 = 12;
    pub const MOV: u32 = 13;
    pub const BIC: u32 = 14;
    pub const MVN: u32 = 15;
}

/// Runs a data-processing operation and writes its result and flags.
pub fn data_processing(
    cpu: &mut Cpu,
    op: u32,
    set_flags: bool,
    d: u32,
    n: u32,
    operand: u32,
    shifter_carry: bool,
) -> crate::instruction::Step {
    use opcode::*;
    let carry_in = cpu.state().flag(psr::C);
    let (result, arithmetic) = match op {
        AND | TST => (n & operand, None),
        EOR | TEQ => (n ^ operand, None),
        ORR => (n | operand, None),
        MOV => (operand, None),
        BIC => (n & !operand, None),
        MVN => (!operand, None),
        SUB | CMP => split(add_with_carry(n, !operand, true)),
        RSB => split(add_with_carry(operand, !n, true)),
        ADD | CMN => split(add_with_carry(n, operand, false)),
        ADC => split(add_with_carry(n, operand, carry_in)),
        SBC => split(add_with_carry(n, !operand, carry_in)),
        RSC => split(add_with_carry(operand, !n, carry_in)),
        _ => unreachable!("the opcode has four bits"),
    };
    let writes = !(TST..=CMN).contains(&op);
    if writes && d == 15 {
        return cpu.alu_write_pc(result);
    }
    if set_flags {
        let state = cpu.state();
        match arithmetic {
            Some((carry, overflow)) => state.set_nzcv(result, carry, overflow),
            None => {
                state.set_nz(result);
                state.set_flag(psr::C, shifter_carry);
            }
        }
    }
    if writes {
        cpu.set_reg(d, result);
    }
    crate::instruction::Step::Continue
}

fn split((result, carry, overflow): (u32, bool, bool)) -> (u32, Option<(bool, bool)>) {
    (result, Some((carry, overflow)))
}

/// Saturates a signed value to `bits` bits and returns whether it saturated.
pub fn signed_saturate(value: i64, bits: u32) -> (u32, bool) {
    let max = (1i64 << (bits - 1)) - 1;
    let min = -(1i64 << (bits - 1));
    if value > max {
        (max as u32, true)
    } else if value < min {
        (min as u32, true)
    } else {
        (value as u32, false)
    }
}

/// Saturates a signed value to an unsigned value of `bits` bits.
pub fn unsigned_saturate(value: i64, bits: u32) -> (u32, bool) {
    let max = if bits >= 32 { u32::MAX as i64 } else { (1i64 << bits) - 1 };
    if value > max {
        (max as u32, true)
    } else if value < 0 {
        (0, true)
    } else {
        (value as u32, false)
    }
}
