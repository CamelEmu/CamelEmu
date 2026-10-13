use super::alu::{decode_imm_shift, shift};
use crate::cpu::Cpu;
use crate::instruction::{Step, bit, bits};
use crate::state::psr;

/// The address of a single load or store and the value of the base register after it.
fn addresses(cpu: &mut Cpu, raw: u32, offset: u32) -> (u32, u32) {
    let n = bits(raw, 19, 16);
    let base = if n == 15 { cpu.reg(15) & !3 } else { cpu.reg(n) };
    let offset_address = if bit(raw, 23) {
        base.wrapping_add(offset)
    } else {
        base.wrapping_sub(offset)
    };
    let address = if bit(raw, 24) { offset_address } else { base };
    (address, offset_address)
}

/// Writes the base register back for the post-indexed forms and the pre-indexed forms with W.
fn write_back(cpu: &mut Cpu, raw: u32, offset_address: u32) {
    let n = bits(raw, 19, 16);
    if (!bit(raw, 24) || bit(raw, 21)) && n != 15 {
        cpu.set_reg(n, offset_address);
    }
}

/// LDR, LDRB, STR and STRB with the offset already computed.
fn single(cpu: &mut Cpu, raw: u32, offset: u32) -> Step {
    let (address, offset_address) = addresses(cpu, raw, offset);
    let t = bits(raw, 15, 12);
    let byte = bit(raw, 22);
    if bit(raw, 20) {
        let value = if byte {
            cpu.read8(address) as u32
        } else {
            cpu.read32(address)
        };
        write_back(cpu, raw, offset_address);
        cpu.write_load(t, value)
    } else {
        let value = cpu.reg(t);
        if byte {
            cpu.write8(address, value as u8);
        } else {
            cpu.write32(address, value);
        }
        write_back(cpu, raw, offset_address);
        Step::Continue
    }
}

/// LDR, LDRB, STR and STRB with an immediate offset, including the literal and the T forms.
pub fn single_imm(cpu: &mut Cpu, raw: u32) -> Step {
    single(cpu, raw, bits(raw, 11, 0))
}

/// LDR, LDRB, STR and STRB with a shifted register offset, including the T forms.
pub fn single_reg(cpu: &mut Cpu, raw: u32) -> Step {
    let carry = cpu.state().flag(psr::C);
    let (kind, amount) = decode_imm_shift(bits(raw, 6, 5), bits(raw, 11, 7));
    let offset = shift(cpu.reg(bits(raw, 3, 0)), kind, amount, carry);
    single(cpu, raw, offset)
}

/// LDRH, LDRSB, LDRSH, LDRD, STRH and STRD. The bit 22 selects the immediate offset, and the
/// bits 6:5 with the bit 20 the operation.
pub fn extra(cpu: &mut Cpu, raw: u32) -> Step {
    let offset = if bit(raw, 22) {
        (bits(raw, 11, 8) << 4) | bits(raw, 3, 0)
    } else {
        cpu.reg(bits(raw, 3, 0))
    };
    let (address, offset_address) = addresses(cpu, raw, offset);
    let t = bits(raw, 15, 12);
    match (bit(raw, 20), bits(raw, 6, 5)) {
        (true, op) => {
            let value = match op {
                0b01 => cpu.read16(address) as u32,
                0b10 => cpu.read8(address) as i8 as i32 as u32,
                _ => cpu.read16(address) as i16 as i32 as u32,
            };
            write_back(cpu, raw, offset_address);
            cpu.write_load(t, value)
        }
        (false, 0b01) => {
            let value = cpu.reg(t);
            cpu.write16(address, value as u16);
            write_back(cpu, raw, offset_address);
            Step::Continue
        }
        (false, 0b10) => {
            let low = cpu.read32(address);
            let high = cpu.read32(address.wrapping_add(4));
            write_back(cpu, raw, offset_address);
            cpu.write_load(t, low);
            cpu.write_load((t + 1) & 15, high)
        }
        _ => {
            let low = cpu.reg(t);
            let high = cpu.reg((t + 1) & 15);
            cpu.write32(address, low);
            cpu.write32(address.wrapping_add(4), high);
            write_back(cpu, raw, offset_address);
            Step::Continue
        }
    }
}

/// LDM and STM in every addressing mode, selected by the bits 24:23. The forms with the bit 22,
/// which touch the registers of the user mode or return from an exception, behave as the plain
/// forms in the user mode.
pub fn multiple(cpu: &mut Cpu, raw: u32) -> Step {
    let n = bits(raw, 19, 16);
    let list = bits(raw, 15, 0);
    let count = list.count_ones();
    let base = cpu.reg(n);
    let size = count * 4;
    let mut address = match bits(raw, 24, 23) {
        0b01 => base,
        0b11 => base.wrapping_add(4),
        0b00 => base.wrapping_sub(size).wrapping_add(4),
        _ => base.wrapping_sub(size),
    };
    let new_base = if bit(raw, 23) {
        base.wrapping_add(size)
    } else {
        base.wrapping_sub(size)
    };
    let write_back = bit(raw, 21);

    if bit(raw, 20) {
        if write_back {
            cpu.set_reg(n, new_base);
        }
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
    } else {
        for register in 0..16 {
            if list & (1 << register) == 0 {
                continue;
            }
            let value = cpu.reg(register);
            cpu.write32(address, value);
            address = address.wrapping_add(4);
        }
        if write_back {
            cpu.set_reg(n, new_base);
        }
        Step::Continue
    }
}

/// The cycles of LDM and STM, which grow with the registers in the list.
pub fn multiple_cycles(raw: u32) -> u32 {
    1 + (raw & 0xFFFF).count_ones() / 2
}

/// The cycles of a load or store with a register offset, which are lower for an offset that is
/// added without a shift or with a shift of 2.
pub fn single_reg_cycles(raw: u32) -> u32 {
    let up = bit(raw, 23);
    let shift_type = bits(raw, 6, 5);
    let imm5 = bits(raw, 11, 7);
    if up && shift_type == 0 && (imm5 == 0 || imm5 == 2) { 2 } else { 4 }
}

/// The cycles of LDRH, LDRSB, LDRSH, LDRD, STRH and STRD with a register offset, which are lower
/// for an offset that is added.
pub fn extra_reg_cycles(raw: u32) -> u32 {
    if bit(raw, 23) { 2 } else { 4 }
}

/// SWP and SWPB, selected by the bit 22.
pub fn swap(cpu: &mut Cpu, raw: u32) -> Step {
    let address = cpu.reg(bits(raw, 19, 16));
    let value = cpu.reg(bits(raw, 3, 0));
    let old = if bit(raw, 22) {
        let old = cpu.read8(address) as u32;
        cpu.write8(address, value as u8);
        old
    } else {
        let old = cpu.read32(address);
        cpu.write32(address, value);
        old
    };
    cpu.write_load(bits(raw, 15, 12), old)
}

/// The size of an exclusive access from the bits 22:21.
fn exclusive_size(raw: u32) -> u32 {
    match bits(raw, 22, 21) {
        0b00 => 4,
        0b01 => 8,
        0b10 => 1,
        _ => 2,
    }
}

/// LDREX, LDREXB, LDREXH and LDREXD.
pub fn load_exclusive(cpu: &mut Cpu, raw: u32) -> Step {
    let address = cpu.reg(bits(raw, 19, 16));
    let t = bits(raw, 15, 12);
    let size = exclusive_size(raw);
    let value = cpu.exclusive_read(address, size);
    if size == 8 {
        let (low, high) = if cpu.state().big_endian() {
            ((value >> 32) as u32, value as u32)
        } else {
            (value as u32, (value >> 32) as u32)
        };
        cpu.set_reg(t, low);
        return cpu.write_load((t + 1) & 15, high);
    }
    cpu.write_load(t, value as u32)
}

/// STREX, STREXB, STREXH and STREXD. The register in the bits 15:12 gets 0 when the store
/// succeeds and 1 when it fails.
pub fn store_exclusive(cpu: &mut Cpu, raw: u32) -> Step {
    let address = cpu.reg(bits(raw, 19, 16));
    let t = bits(raw, 3, 0);
    let size = exclusive_size(raw);
    let value = if size == 8 {
        let low = cpu.reg(t) as u64;
        let high = cpu.reg((t + 1) & 15) as u64;
        if cpu.state().big_endian() {
            (low << 32) | high
        } else {
            low | (high << 32)
        }
    } else {
        cpu.reg(t) as u64
    };
    let stored = cpu.exclusive_write(address, size, value);
    cpu.set_reg(bits(raw, 15, 12), !stored as u32);
    Step::Continue
}
