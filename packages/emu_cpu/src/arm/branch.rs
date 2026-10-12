use crate::cpu::Cpu;
use crate::instruction::{Step, bit, bits};
use crate::state::psr;

fn offset24(raw: u32) -> u32 {
    (((raw << 8) as i32) >> 6) as u32
}

pub fn b(cpu: &mut Cpu, raw: u32) -> Step {
    let target = cpu.reg(15).wrapping_add(offset24(raw));
    cpu.branch_write_pc(target)
}

pub fn bl(cpu: &mut Cpu, raw: u32) -> Step {
    let return_address = cpu.pc().wrapping_add(4);
    cpu.set_reg(14, return_address);
    b(cpu, raw)
}

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
