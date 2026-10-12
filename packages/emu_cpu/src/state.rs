/// The registers of an ARM11 core. It is `EmuCpuState` of `emu_cpu.h`, which the core reads and
/// writes directly to save and load the context of a thread.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CpuState {
    pub regs: [u32; 16],
    pub cpsr: u32,
    /// S0-S31, which overlap D0-D15, followed by D16-D31.
    pub ext_regs: [u32; 64],
    pub fpscr: u32,
    pub fpexc: u32,
    pub cp15_thread_uprw: u32,
    pub cp15_thread_uro: u32,
}

impl Default for CpuState {
    fn default() -> Self {
        Self {
            regs: [0; 16],
            cpsr: psr::MODE_USER,
            ext_regs: [0; 64],
            fpscr: 0,
            fpexc: 0,
            cp15_thread_uprw: 0,
            cp15_thread_uro: 0,
        }
    }
}

pub mod psr {
    pub const N: u32 = 1 << 31;
    pub const Z: u32 = 1 << 30;
    pub const C: u32 = 1 << 29;
    pub const V: u32 = 1 << 28;
    pub const Q: u32 = 1 << 27;
    pub const NZCV: u32 = N | Z | C | V;
    pub const GE_SHIFT: u32 = 16;
    pub const GE: u32 = 0xF << GE_SHIFT;
    pub const E: u32 = 1 << 9;
    pub const T: u32 = 1 << 5;
    pub const MODE_USER: u32 = 0x10;
    /// The bits that MSR may write in user mode.
    pub const USER_WRITABLE: u32 = NZCV | Q | GE | E;
}

impl CpuState {
    pub fn thumb(&self) -> bool {
        self.cpsr & psr::T != 0
    }

    pub fn big_endian(&self) -> bool {
        self.cpsr & psr::E != 0
    }

    pub fn flag(&self, bit: u32) -> bool {
        self.cpsr & bit != 0
    }

    pub fn set_flag(&mut self, bit: u32, value: bool) {
        if value {
            self.cpsr |= bit;
        } else {
            self.cpsr &= !bit;
        }
    }

    pub fn set_nz(&mut self, result: u32) {
        self.set_flag(psr::N, result & 0x8000_0000 != 0);
        self.set_flag(psr::Z, result == 0);
    }

    pub fn set_nzcv(&mut self, result: u32, carry: bool, overflow: bool) {
        self.set_nz(result);
        self.set_flag(psr::C, carry);
        self.set_flag(psr::V, overflow);
    }

    pub fn ge(&self) -> u32 {
        (self.cpsr & psr::GE) >> psr::GE_SHIFT
    }

    pub fn set_ge(&mut self, ge: u32) {
        self.cpsr = (self.cpsr & !psr::GE) | ((ge & 0xF) << psr::GE_SHIFT);
    }

    /// Whether the condition of the bits 31:28 of an ARM instruction, or of a Thumb conditional
    /// branch, passes. The value 0b1111 passes as the instructions that use it are unconditional.
    pub fn condition_passed(&self, cond: u32) -> bool {
        let n = self.flag(psr::N);
        let z = self.flag(psr::Z);
        let c = self.flag(psr::C);
        let v = self.flag(psr::V);
        let result = match cond >> 1 {
            0b000 => z,
            0b001 => c,
            0b010 => n,
            0b011 => v,
            0b100 => c && !z,
            0b101 => n == v,
            0b110 => !z && n == v,
            _ => return true,
        };
        result != (cond & 1 != 0)
    }

    pub fn single(&self, index: usize) -> u32 {
        self.ext_regs[index]
    }

    pub fn set_single(&mut self, index: usize, value: u32) {
        self.ext_regs[index] = value;
    }

    pub fn double(&self, index: usize) -> u64 {
        let low = self.ext_regs[index * 2] as u64;
        let high = self.ext_regs[index * 2 + 1] as u64;
        low | (high << 32)
    }

    pub fn set_double(&mut self, index: usize, value: u64) {
        self.ext_regs[index * 2] = value as u32;
        self.ext_regs[index * 2 + 1] = (value >> 32) as u32;
    }
}
