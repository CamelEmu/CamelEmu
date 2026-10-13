use crate::cpu::Cpu;
use crate::instruction::{Step, bit, bits};
use crate::state::psr;

/// The signed 24-bit word offset of B, BL and BLX, in bytes.
fn offset24(raw: u32) -> u32 {
    (((raw << 8) as i32) >> 6) as u32
}

/// B: branches relative to the PC.
pub fn b(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(15).wrapping_add(offset24(raw));
    cpu.branch_write_pc(target)
}

/// BL: branches relative to the PC and keeps the return address in LR.
pub fn bl(cpu: &mut Cpu, raw: u32) -> Step {
    let return_address = cpu.pc().wrapping_add(4);
    cpu.set_reg(14, return_address);
    b(cpu, raw)
}

/// BLX with an immediate: calls Thumb code relative to the PC.
pub fn blx_imm(cpu: &mut Cpu, raw: u32) -> Step {
    let return_address = cpu.pc().wrapping_add(4);
    let target = cpu
        .reg(15)
        .wrapping_add(offset24(raw))
        .wrapping_add((bit(raw, 24) as u32) << 1);
    cpu.set_reg(14, return_address);
    cpu.state().cpsr |= psr::T;
    cpu.state().regs[15] = target;
    Step::Branch
}

/// BLX with a register: calls the address in Rm, whose bit 0 selects the instruction set.
pub fn blx_reg(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(bits(raw, 3, 0));
    let return_address = cpu.pc().wrapping_add(4);
    cpu.set_reg(14, return_address);
    cpu.bx_write_pc(target)
}

/// BX and BXJ, as the core has no Jazelle state.
pub fn bx(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(bits(raw, 3, 0));
    cpu.bx_write_pc(target)
}
