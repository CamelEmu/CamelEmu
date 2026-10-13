use std::sync::atomic::{Ordering, fence};

use super::alu::expand_imm_c;
use crate::cpu::Cpu;
use crate::host::Exception;
use crate::instruction::{Step, bit, bits};
use crate::state::psr;

/// MRS: reads CPSR.
pub fn mrs(cpu: &mut Cpu, raw: u32) -> Step {
    let cpsr = cpu.state().cpsr;
    cpu.write_alu(bits(raw, 15, 12), cpsr)
}

/// Writes the bytes of CPSR that the bits 19:16 select, limited to the bits that the user mode
/// may change.
fn write_cpsr(cpu: &mut Cpu, raw: u32, value: u32) -> Step {
    let fields = bits(raw, 19, 16);
    let mut mask = 0;
    for field in 0..4 {
        if fields & (1 << field) != 0 {
            mask |= 0xFF << (field * 8);
        }
    }
    mask &= psr::USER_WRITABLE;
    let state = cpu.state();
    state.cpsr = (state.cpsr & !mask) | (value & mask);
    Step::Continue
}

/// MSR with an immediate: writes CPSR.
pub fn msr_imm(cpu: &mut Cpu, raw: u32) -> Step {
    let (value, _) = expand_imm_c(bits(raw, 11, 0), false);
    write_cpsr(cpu, raw, value)
}

/// MSR with a register: writes CPSR.
pub fn msr_reg(cpu: &mut Cpu, raw: u32) -> Step {
    let value = cpu.reg(bits(raw, 3, 0));
    write_cpsr(cpu, raw, value)
}

/// SETEND in ARM state, where the bit 9 selects big endian.
pub fn setend(cpu: &mut Cpu, raw: u32) -> Step {
    cpu.state().set_flag(psr::E, bit(raw, 9));
    Step::Continue
}

/// Instructions that do nothing in the user mode, such as CPS, the hints and the preloads.
pub fn nop(_cpu: &mut Cpu, _raw: u32) -> Step {
    Step::Continue
}

pub fn barrier(_cpu: &mut Cpu, _raw: u32) -> Step {
    fence(Ordering::SeqCst);
    Step::Continue
}

/// CLREX: drops the local reservation of the exclusive accesses.
pub fn clrex(cpu: &mut Cpu, _raw: u32) -> Step {
    cpu.clear_exclusive();
    Step::Continue
}

fn instruction_size(cpu: &mut Cpu) -> u32 {
    if cpu.state().thumb() { 2 } else { 4 }
}

/// Calls the supervisor with the PC at the next instruction, as the kernel may switch the thread.
pub fn supervisor_call(cpu: &mut Cpu, swi: u32) -> Step {
    let next = cpu.pc().wrapping_add(instruction_size(cpu));
    cpu.state().regs[15] = next;
    cpu.call_svc(swi);
    Step::Branch
}

/// SVC in ARM state, with the 24-bit immediate.
pub fn svc(cpu: &mut Cpu, raw: u32) -> Step {
    supervisor_call(cpu, bits(raw, 23, 0))
}

/// BKPT: reports a breakpoint to the core, which serves it to the GDB stub.
pub fn bkpt(cpu: &mut Cpu, _raw: u32) -> Step {
    let pc = cpu.pc();
    cpu.raise(Exception::Breakpoint, pc)
}

/// UDF, and every encoding that the CPU does not implement.
pub fn undefined(cpu: &mut Cpu, _raw: u32) -> Step {
    let pc = cpu.pc();
    cpu.raise(Exception::UndefinedInstruction, pc)
}

/// The coprocessor fields of MCR and MRC.
struct CoprocessorRegister {
    coprocessor: u32,
    opc1: u32,
    crn: u32,
    crm: u32,
    opc2: u32,
}

impl CoprocessorRegister {
    fn decode(raw: u32) -> Self {
        Self {
            coprocessor: bits(raw, 11, 8),
            opc1: bits(raw, 23, 21),
            crn: bits(raw, 19, 16),
            crm: bits(raw, 3, 0),
            opc2: bits(raw, 7, 5),
        }
    }

    fn is(&self, crn: u32, opc1: u32, crm: u32, opc2: u32) -> bool {
        self.coprocessor == 15
            && self.crn == crn
            && self.opc1 == opc1
            && self.crm == crm
            && self.opc2 == opc2
    }
}

/// MCR to the registers of CP15 that the user mode reaches: the barriers, the flush of the
/// prefetch buffer and the thread ID that the user may write.
pub fn mcr(cpu: &mut Cpu, raw: u32) -> Step {
    let register = CoprocessorRegister::decode(raw);
    let unconditional = bits(raw, 31, 28) == 0b1111;
    if unconditional {
        return undefined(cpu, raw);
    }
    if register.is(7, 0, 5, 4) || register.is(7, 0, 10, 4) || register.is(7, 0, 10, 5) {
        return barrier(cpu, raw);
    }
    if register.is(13, 0, 0, 2) {
        let value = cpu.reg(bits(raw, 15, 12));
        cpu.state().cp15_thread_uprw = value;
        return Step::Continue;
    }
    undefined(cpu, raw)
}

/// MRC from the thread IDs of CP15.
pub fn mrc(cpu: &mut Cpu, raw: u32) -> Step {
    let register = CoprocessorRegister::decode(raw);
    let unconditional = bits(raw, 31, 28) == 0b1111;
    let value = if unconditional {
        None
    } else if register.is(13, 0, 0, 2) {
        Some(cpu.state().cp15_thread_uprw)
    } else if register.is(13, 0, 0, 3) {
        Some(cpu.state().cp15_thread_uro)
    } else {
        None
    };
    let Some(value) = value else {
        return undefined(cpu, raw);
    };
    let t = bits(raw, 15, 12);
    if t == 15 {
        let state = cpu.state();
        state.cpsr = (state.cpsr & !psr::NZCV) | (value & psr::NZCV);
        return Step::Continue;
    }
    cpu.set_reg(t, value);
    Step::Continue
}
