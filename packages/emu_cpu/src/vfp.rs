//! The instructions of VFPv2. The host computes them in the floating-point environment of the
//! guest, which `fpenv` keeps in sync with FPSCR.

use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::arm::system::undefined;
use crate::cpu::Cpu;
use crate::fpenv;
use crate::instruction::{Instruction, Step, bit, bits, inst};
use crate::state::{CpuState, psr};

trait Float:
    Copy + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self>
{
    /// The index of a register from its four bits and its extra bit.
    fn index(field: u32, extra: bool) -> usize;
    fn read(state: &CpuState, index: usize) -> Self;
    fn write(state: &mut CpuState, index: usize, value: Self);
    fn abs(self) -> Self;
    fn sqrt(self) -> Self;
    fn zero() -> Self;
    fn compare(self, other: Self, signal_nans: bool) -> u32;
    fn to_i32(self, round_to_zero: bool) -> i32;
    fn to_u32(self, round_to_zero: bool) -> u32;
    fn from_i32(value: i32) -> Self;
    fn from_u32(value: u32) -> Self;
}

impl Float for f32 {
    fn index(field: u32, extra: bool) -> usize {
        ((field << 1) | extra as u32) as usize
    }
    fn read(state: &CpuState, index: usize) -> Self {
        f32::from_bits(state.single(index))
    }
    fn write(state: &mut CpuState, index: usize, value: Self) {
        state.set_single(index, value.to_bits());
    }
    fn abs(self) -> Self {
        f32::abs(self)
    }
    fn sqrt(self) -> Self {
        f32::sqrt(self)
    }
    fn zero() -> Self {
        0.0
    }
    fn compare(self, other: Self, signal_nans: bool) -> u32 {
        host::compare_f32(self, other, signal_nans)
    }
    fn to_i32(self, round_to_zero: bool) -> i32 {
        host::round_f32(self, round_to_zero) as i32
    }
    fn to_u32(self, round_to_zero: bool) -> u32 {
        host::round_f32(self, round_to_zero) as u32
    }
    fn from_i32(value: i32) -> Self {
        value as f32
    }
    fn from_u32(value: u32) -> Self {
        value as f32
    }
}

impl Float for f64 {
    fn index(field: u32, extra: bool) -> usize {
        (((extra as u32) << 4) | field) as usize
    }
    fn read(state: &CpuState, index: usize) -> Self {
        f64::from_bits(state.double(index))
    }
    fn write(state: &mut CpuState, index: usize, value: Self) {
        state.set_double(index, value.to_bits());
    }
    fn abs(self) -> Self {
        f64::abs(self)
    }
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    fn zero() -> Self {
        0.0
    }
    fn compare(self, other: Self, signal_nans: bool) -> u32 {
        host::compare_f64(self, other, signal_nans)
    }
    fn to_i32(self, round_to_zero: bool) -> i32 {
        host::round_f64(self, round_to_zero) as i32
    }
    fn to_u32(self, round_to_zero: bool) -> u32 {
        host::round_f64(self, round_to_zero) as u32
    }
    fn from_i32(value: i32) -> Self {
        value as f64
    }
    fn from_u32(value: u32) -> Self {
        value as f64
    }
}

/// The operations that need instructions of the host with exact exception semantics.
mod host {
    #[cfg(target_arch = "aarch64")]
    use std::arch::asm;

    /// Returns the NZCV flags of a comparison, which AArch64 sets as VFP does.
    #[cfg(target_arch = "aarch64")]
    pub fn compare_f32(a: f32, b: f32, signal_nans: bool) -> u32 {
        let nzcv: u64;
        unsafe {
            if signal_nans {
                asm!("fcmpe {a:s}, {b:s}", "mrs {f}, nzcv", a = in(vreg) a, b = in(vreg) b, f = out(reg) nzcv);
            } else {
                asm!("fcmp {a:s}, {b:s}", "mrs {f}, nzcv", a = in(vreg) a, b = in(vreg) b, f = out(reg) nzcv);
            }
        }
        nzcv as u32 & 0xF000_0000
    }

    #[cfg(target_arch = "aarch64")]
    pub fn compare_f64(a: f64, b: f64, signal_nans: bool) -> u32 {
        let nzcv: u64;
        unsafe {
            if signal_nans {
                asm!("fcmpe {a:d}, {b:d}", "mrs {f}, nzcv", a = in(vreg) a, b = in(vreg) b, f = out(reg) nzcv);
            } else {
                asm!("fcmp {a:d}, {b:d}", "mrs {f}, nzcv", a = in(vreg) a, b = in(vreg) b, f = out(reg) nzcv);
            }
        }
        nzcv as u32 & 0xF000_0000
    }

    /// Rounds to an integral value with the rounding mode of FPSCR, unless the conversion rounds
    /// to zero, which the conversion to an integer then does by itself.
    #[cfg(target_arch = "aarch64")]
    pub fn round_f32(value: f32, round_to_zero: bool) -> f32 {
        if round_to_zero {
            return value;
        }
        let result: f32;
        unsafe { asm!("frintx {r:s}, {v:s}", v = in(vreg) value, r = out(vreg) result) };
        result
    }

    #[cfg(target_arch = "aarch64")]
    pub fn round_f64(value: f64, round_to_zero: bool) -> f64 {
        if round_to_zero {
            return value;
        }
        let result: f64;
        unsafe { asm!("frintx {r:d}, {v:d}", v = in(vreg) value, r = out(vreg) result) };
        result
    }

    #[cfg(not(target_arch = "aarch64"))]
    fn compare<T: PartialOrd>(a: T, b: T) -> u32 {
        match a.partial_cmp(&b) {
            Some(std::cmp::Ordering::Equal) => 0x6000_0000,
            Some(std::cmp::Ordering::Less) => 0x8000_0000,
            Some(std::cmp::Ordering::Greater) => 0x2000_0000,
            None => 0x3000_0000,
        }
    }

    #[cfg(not(target_arch = "aarch64"))]
    pub fn compare_f32(a: f32, b: f32, _signal_nans: bool) -> u32 {
        compare(a, b)
    }

    #[cfg(not(target_arch = "aarch64"))]
    pub fn compare_f64(a: f64, b: f64, _signal_nans: bool) -> u32 {
        compare(a, b)
    }

    #[cfg(not(target_arch = "aarch64"))]
    pub fn round_f32(value: f32, round_to_zero: bool) -> f32 {
        if round_to_zero { value } else { value.round_ties_even() }
    }

    #[cfg(not(target_arch = "aarch64"))]
    pub fn round_f64(value: f64, round_to_zero: bool) -> f64 {
        if round_to_zero { value } else { value.round_ties_even() }
    }
}

fn vd<T: Float>(raw: u32) -> usize {
    T::index(bits(raw, 15, 12), bit(raw, 22))
}

fn vn<T: Float>(raw: u32) -> usize {
    T::index(bits(raw, 19, 16), bit(raw, 7))
}

fn vm<T: Float>(raw: u32) -> usize {
    T::index(bits(raw, 3, 0), bit(raw, 5))
}

/// Whether the instruction works on doubles, from the bit 8.
fn is_double(raw: u32) -> bool {
    bit(raw, 8)
}

macro_rules! by_precision {
    ($cpu:expr, $raw:expr, $function:ident) => {
        if is_double($raw) {
            $function::<f64>($cpu, $raw)
        } else {
            $function::<f32>($cpu, $raw)
        }
    };
}

fn three<T: Float>(cpu: &mut Cpu, raw: u32) -> Step {
    let state = cpu.state();
    let d = T::read(state, vd::<T>(raw));
    let n = T::read(state, vn::<T>(raw));
    let m = T::read(state, vm::<T>(raw));
    let result = match (bit(raw, 23), bits(raw, 21, 20), bit(raw, 6)) {
        (false, 0b00, false) => d + n * m,
        (false, 0b00, true) => d + -(n * m),
        (false, 0b01, false) => -d + n * m,
        (false, 0b01, true) => -d + -(n * m),
        (false, 0b10, false) => n * m,
        (false, 0b10, true) => -(n * m),
        (false, 0b11, false) => n + m,
        (false, 0b11, true) => n - m,
        (true, 0b00, false) => n / m,
        _ => return undefined(cpu, raw),
    };
    T::write(cpu.state(), vd::<T>(raw), result);
    Step::Continue
}

/// VMLA, VMLS, VNMLA, VNMLS, VMUL, VNMUL, VADD, VSUB and VDIV.
pub fn arithmetic(cpu: &mut Cpu, raw: u32) -> Step {
    by_precision!(cpu, raw, three)
}

fn unary<T: Float>(cpu: &mut Cpu, raw: u32) -> Step {
    let state = cpu.state();
    let m = T::read(state, vm::<T>(raw));
    let result = match (bits(raw, 19, 16), bit(raw, 7)) {
        (0b0000, false) => m,
        (0b0000, true) => m.abs(),
        (0b0001, false) => -m,
        _ => m.sqrt(),
    };
    T::write(state, vd::<T>(raw), result);
    Step::Continue
}

/// VMOV (reg), VABS, VNEG and VSQRT.
pub fn unary_operation(cpu: &mut Cpu, raw: u32) -> Step {
    by_precision!(cpu, raw, unary)
}

fn compare<T: Float>(cpu: &mut Cpu, raw: u32) -> Step {
    let state = cpu.state();
    let d = T::read(state, vd::<T>(raw));
    let m = if bit(raw, 16) {
        T::zero()
    } else {
        T::read(state, vm::<T>(raw))
    };
    let flags = d.compare(m, bit(raw, 7));
    state.fpscr = (state.fpscr & !psr::NZCV) | flags;
    Step::Continue
}

/// VCMP and VCMPE, against a register or against zero.
pub fn vcmp(cpu: &mut Cpu, raw: u32) -> Step {
    by_precision!(cpu, raw, compare)
}

/// VCVT between single and double precision, where the bit 8 tells the precision of the source.
pub fn vcvt_f_to_f(cpu: &mut Cpu, raw: u32) -> Step {
    let state = cpu.state();
    if is_double(raw) {
        let value = f64::read(state, vm::<f64>(raw)) as f32;
        f32::write(state, vd::<f32>(raw), value);
    } else {
        let value = f32::read(state, vm::<f32>(raw)) as f64;
        f64::write(state, vd::<f64>(raw), value);
    }
    Step::Continue
}

fn from_int<T: Float>(cpu: &mut Cpu, raw: u32) -> Step {
    let state = cpu.state();
    let source = state.single(vm::<f32>(raw));
    let value = if bit(raw, 7) {
        T::from_i32(source as i32)
    } else {
        T::from_u32(source)
    };
    T::write(state, vd::<T>(raw), value);
    Step::Continue
}

/// VCVT from a 32-bit integer, where the bit 7 tells a signed one.
pub fn vcvt_from_int(cpu: &mut Cpu, raw: u32) -> Step {
    by_precision!(cpu, raw, from_int)
}

fn to_int<T: Float>(cpu: &mut Cpu, raw: u32) -> Step {
    let state = cpu.state();
    let value = T::read(state, vm::<T>(raw));
    let round_to_zero = bit(raw, 7);
    let result = if bit(raw, 16) {
        value.to_i32(round_to_zero) as u32
    } else {
        value.to_u32(round_to_zero)
    };
    state.set_single(vd::<f32>(raw), result);
    Step::Continue
}

/// VCVT to a 32-bit integer, where the bit 16 tells a signed one and the bit 7 rounds to zero
/// instead of with the rounding mode of FPSCR.
pub fn vcvt_to_int(cpu: &mut Cpu, raw: u32) -> Step {
    by_precision!(cpu, raw, to_int)
}

/// VMOV between a core register and a single, where the bit 20 moves to the core register.
pub fn vmov_core_single(cpu: &mut Cpu, raw: u32) -> Step {
    let n = f32::index(bits(raw, 19, 16), bit(raw, 7));
    let t = bits(raw, 15, 12);
    if bit(raw, 20) {
        let value = cpu.state().single(n);
        return cpu.write_load(t, value);
    }
    let value = cpu.reg(t);
    cpu.state().set_single(n, value);
    Step::Continue
}

/// VMOV between two core registers and two singles, where the bit 20 moves to the core registers.
pub fn vmov_core_two_singles(cpu: &mut Cpu, raw: u32) -> Step {
    let m = vm::<f32>(raw);
    let t = bits(raw, 15, 12);
    let t2 = bits(raw, 19, 16);
    if m == 31 {
        return undefined(cpu, raw);
    }
    if bit(raw, 20) {
        let state = cpu.state();
        let (low, high) = (state.single(m), state.single(m + 1));
        cpu.set_reg(t, low);
        cpu.set_reg(t2, high);
    } else {
        let (low, high) = (cpu.reg(t), cpu.reg(t2));
        let state = cpu.state();
        state.set_single(m, low);
        state.set_single(m + 1, high);
    }
    Step::Continue
}

/// VMOV between two core registers and a double, where the bit 20 moves to the core registers.
pub fn vmov_core_double(cpu: &mut Cpu, raw: u32) -> Step {
    let m = vm::<f64>(raw);
    let t = bits(raw, 15, 12);
    let t2 = bits(raw, 19, 16);
    if bit(raw, 20) {
        let value = cpu.state().double(m);
        cpu.set_reg(t, value as u32);
        cpu.set_reg(t2, (value >> 32) as u32);
    } else {
        let value = cpu.reg(t) as u64 | ((cpu.reg(t2) as u64) << 32);
        cpu.state().set_double(m, value);
    }
    Step::Continue
}

/// VMOV between a core register and a half of a double, where the bit 21 selects the upper half
/// and the bit 20 moves to the core register.
pub fn vmov_core_scalar(cpu: &mut Cpu, raw: u32) -> Step {
    let n = f64::index(bits(raw, 19, 16), bit(raw, 7));
    let word = n * 2 + bit(raw, 21) as usize;
    let t = bits(raw, 15, 12);
    if bit(raw, 20) {
        let value = cpu.state().ext_regs[word];
        return cpu.write_load(t, value);
    }
    let value = cpu.reg(t);
    cpu.state().ext_regs[word] = value;
    Step::Continue
}

pub fn vmrs(cpu: &mut Cpu, raw: u32) -> Step {
    if cpu.in_guest_environment() {
        fpenv::collect_exceptions(&mut cpu.state().fpscr);
    }
    let fpscr = cpu.state().fpscr;
    let t = bits(raw, 15, 12);
    if t == 15 {
        let state = cpu.state();
        state.cpsr = (state.cpsr & !psr::NZCV) | (fpscr & psr::NZCV);
        return Step::Continue;
    }
    cpu.set_reg(t, fpscr);
    Step::Continue
}

pub fn vmsr(cpu: &mut Cpu, raw: u32) -> Step {
    let value = cpu.reg(bits(raw, 15, 12));
    if cpu.in_guest_environment() {
        fpenv::collect_exceptions(&mut cpu.state().fpscr);
        fpenv::apply_control(value);
    }
    cpu.state().fpscr = value;
    Step::Continue
}

fn load_double(cpu: &mut Cpu, address: u32) -> u64 {
    let first = cpu.read32(address) as u64;
    let second = cpu.read32(address.wrapping_add(4)) as u64;
    if cpu.state().big_endian() {
        (first << 32) | second
    } else {
        first | (second << 32)
    }
}

fn store_double(cpu: &mut Cpu, address: u32, value: u64) {
    let (first, second) = if cpu.state().big_endian() {
        ((value >> 32) as u32, value as u32)
    } else {
        (value as u32, (value >> 32) as u32)
    };
    cpu.write32(address, first);
    cpu.write32(address.wrapping_add(4), second);
}

fn transfer(cpu: &mut Cpu, raw: u32, address: u32, register: usize) {
    let load = bit(raw, 20);
    if is_double(raw) {
        if load {
            let value = load_double(cpu, address);
            cpu.state().set_double(register, value);
        } else {
            let value = cpu.state().double(register);
            store_double(cpu, address, value);
        }
    } else if load {
        let value = cpu.read32(address);
        cpu.state().set_single(register, value);
    } else {
        let value = cpu.state().single(register);
        cpu.write32(address, value);
    }
}

fn first_register(raw: u32) -> usize {
    if is_double(raw) { vd::<f64>(raw) } else { vd::<f32>(raw) }
}

/// VLDR and VSTR, where the bit 20 loads.
pub fn vldr_vstr(cpu: &mut Cpu, raw: u32) -> Step {
    let n = bits(raw, 19, 16);
    let base = if n == 15 { cpu.reg(15) & !3 } else { cpu.reg(n) };
    let offset = bits(raw, 7, 0) * 4;
    let address = if bit(raw, 23) {
        base.wrapping_add(offset)
    } else {
        base.wrapping_sub(offset)
    };
    transfer(cpu, raw, address, first_register(raw));
    Step::Continue
}

/// VLDM, VSTM, VPUSH and VPOP, where the bit 20 loads, the bit 24 decrements before and the bit
/// 21 writes the base back.
pub fn vldm_vstm(cpu: &mut Cpu, raw: u32) -> Step {
    let (before, up, write_back) = (bit(raw, 24), bit(raw, 23), bit(raw, 21));
    if before == up {
        return undefined(cpu, raw);
    }
    let n = bits(raw, 19, 16);
    let words = bits(raw, 7, 0);
    let base = cpu.reg(n);
    let size = words * 4;
    let mut address = if up { base } else { base.wrapping_sub(size) };
    let (count, step) = if is_double(raw) { (words / 2, 8) } else { (words, 4) };
    let first = first_register(raw);
    if write_back {
        let new_base = if up { base.wrapping_add(size) } else { base.wrapping_sub(size) };
        cpu.set_reg(n, new_base);
    }
    for index in 0..count as usize {
        transfer(cpu, raw, address, (first + index) % 32);
        address = address.wrapping_add(step);
    }
    Step::Continue
}

#[rustfmt::skip]
pub static TABLE: &[Instruction] = &[
    // Floating-point three-register data processing instructions
    inst!("VMLA",                    FloatingPoint,          "cccc11100D00nnnndddd101zN0M0mmmm", arithmetic, 1),
    inst!("VMLS",                    FloatingPoint,          "cccc11100D00nnnndddd101zN1M0mmmm", arithmetic, 1),
    inst!("VNMLS",                   FloatingPoint,          "cccc11100D01nnnndddd101zN0M0mmmm", arithmetic, 1),
    inst!("VNMLA",                   FloatingPoint,          "cccc11100D01nnnndddd101zN1M0mmmm", arithmetic, 1),
    inst!("VMUL",                    FloatingPoint,          "cccc11100D10nnnndddd101zN0M0mmmm", arithmetic, 1),
    inst!("VNMUL",                   FloatingPoint,          "cccc11100D10nnnndddd101zN1M0mmmm", arithmetic, 1),
    inst!("VADD",                    FloatingPoint,          "cccc11100D11nnnndddd101zN0M0mmmm", arithmetic, 1),
    inst!("VSUB",                    FloatingPoint,          "cccc11100D11nnnndddd101zN1M0mmmm", arithmetic, 1),
    inst!("VDIV",                    FloatingPoint,          "cccc11101D00nnnndddd101zN0M0mmmm", arithmetic, 1),
    inst!("VFNMS",                   Undefined,              "cccc11101D01nnnndddd101zN0M0mmmm", undefined, 1),
    inst!("VFNMA",                   Undefined,              "cccc11101D01nnnndddd101zN1M0mmmm", undefined, 1),
    inst!("VFMA",                    Undefined,              "cccc11101D10nnnndddd101zN0M0mmmm", undefined, 1),
    inst!("VFMS",                    Undefined,              "cccc11101D10nnnndddd101zN1M0mmmm", undefined, 1),
    inst!("VSEL",                    Undefined,              "111111100Dccnnnndddd101zN0M0mmmm", undefined, 1),
    inst!("VMAXNNM",                 Undefined,              "111111101D00nnnndddd101zN0M0mmmm", undefined, 1),
    inst!("VMINNM",                  Undefined,              "111111101D00nnnndddd101zN1M0mmmm", undefined, 1),

    // Other floating-point data-processing instructions
    inst!("VMOV (immediate)",        Undefined,              "cccc11101D11vvvvdddd101z0000vvvv", undefined, 1),
    inst!("VMOV (reg)",              FloatingPoint,          "cccc11101D110000dddd101z01M0mmmm", unary_operation, 1),
    inst!("VABS",                    FloatingPoint,          "cccc11101D110000dddd101z11M0mmmm", unary_operation, 1),
    inst!("VNEG",                    FloatingPoint,          "cccc11101D110001dddd101z01M0mmmm", unary_operation, 1),
    inst!("VSQRT",                   FloatingPoint,          "cccc11101D110001dddd101z11M0mmmm", unary_operation, 1),
    inst!("VCVTB",                   Undefined,              "cccc11101D11001odddd101z01M0mmmm", undefined, 1),
    inst!("VCVTT",                   Undefined,              "cccc11101D11001odddd101z11M0mmmm", undefined, 1),
    inst!("VCMP",                    FloatingPoint,          "cccc11101D110100dddd101zE1M0mmmm", vcmp, 1),
    inst!("VCMP (with zero)",        FloatingPoint,          "cccc11101D110101dddd101zE1000000", vcmp, 1),
    inst!("VRINTR",                  Undefined,              "cccc11101D110110dddd101z01M0mmmm", undefined, 1),
    inst!("VRINTZ",                  Undefined,              "cccc11101D110110dddd101z11M0mmmm", undefined, 1),
    inst!("VRINTX",                  Undefined,              "cccc11101D110111dddd101z01M0mmmm", undefined, 1),
    inst!("VCVT (f32<->f64)",        FloatingPoint,          "cccc11101D110111dddd101z11M0mmmm", vcvt_f_to_f, 1),
    inst!("VCVT (from int)",         FloatingPoint,          "cccc11101D111000dddd101zs1M0mmmm", vcvt_from_int, 1),
    inst!("VCVT (from fixed)",       Undefined,              "cccc11101D11101Udddd101zx1i0vvvv", undefined, 1),
    inst!("VCVT (to u32)",           FloatingPoint,          "cccc11101D111100dddd101zr1M0mmmm", vcvt_to_int, 1),
    inst!("VCVT (to s32)",           FloatingPoint,          "cccc11101D111101dddd101zr1M0mmmm", vcvt_to_int, 1),
    inst!("VCVT (to fixed)",         Undefined,              "cccc11101D11111Udddd101zx1i0vvvv", undefined, 1),
    inst!("VRINT{A,N,P,M}",          Undefined,              "111111101D1110mmdddd101z01M0mmmm", undefined, 1),
    inst!("VCVT{A,N,P,M}",           Undefined,              "111111101D1111mmdddd101zU1M0mmmm", undefined, 1),

    // Floating-point move instructions
    inst!("VMOV (core to f64)",      FloatingPointTransfer,  "cccc11100000ddddtttt1011D0010000", vmov_core_scalar, 2),
    inst!("VMOV (f64 to core)",      FloatingPointTransfer,  "cccc11100001nnnntttt1011N0010000", vmov_core_scalar, 2),
    inst!("VMOV (core to f32)",      FloatingPointTransfer,  "cccc11100000nnnntttt1010N0010000", vmov_core_single, 2),
    inst!("VMOV (f32 to core)",      FloatingPointTransfer,  "cccc11100001nnnntttt1010N0010000", vmov_core_single, 2),
    inst!("VMOV (2xcore to 2xf32)",  FloatingPointTransfer,  "cccc11000100uuuutttt101000M1mmmm", vmov_core_two_singles, 2),
    inst!("VMOV (2xf32 to 2xcore)",  FloatingPointTransfer,  "cccc11000101uuuutttt101000M1mmmm", vmov_core_two_singles, 2),
    inst!("VMOV (2xcore to f64)",    FloatingPointTransfer,  "cccc11000100uuuutttt101100M1mmmm", vmov_core_double, 2),
    inst!("VMOV (f64 to 2xcore)",    FloatingPointTransfer,  "cccc11000101uuuutttt101100M1mmmm", vmov_core_double, 2),
    inst!("VMOV (core to i32)",      FloatingPointTransfer,  "cccc111000i0nnnntttt1011N0010000", vmov_core_scalar, 2),
    inst!("VMOV (core to i16)",      Undefined,              "cccc111000i0nnnntttt1011Ni110000", undefined, 2),
    inst!("VMOV (core to i8)",       Undefined,              "cccc111001i0nnnntttt1011Nii10000", undefined, 2),
    inst!("VMOV (i32 to core)",      FloatingPointTransfer,  "cccc111000i1nnnntttt1011N0010000", vmov_core_scalar, 2),
    inst!("VMOV (i16 to core)",      Undefined,              "cccc1110U0i1nnnntttt1011Ni110000", undefined, 2),
    inst!("VMOV (i8 to core)",       Undefined,              "cccc1110U1i1nnnntttt1011Nii10000", undefined, 2),
    inst!("VDUP (from core)",        Undefined,              "cccc11101BQ0ddddtttt1011D0E10000", undefined, 2),

    // Floating-point system register access
    inst!("VMSR",                    FloatingPointTransfer,  "cccc111011100001tttt101000010000", vmsr, 2),
    inst!("VMRS",                    FloatingPointTransfer,  "cccc111011110001tttt101000010000", vmrs, 2),

    // Extension register load-store instructions
    inst!("VPUSH",                   FloatingPointLoadStore, "cccc11010D101101dddd101zvvvvvvvv", vldm_vstm, 1),
    inst!("VPOP",                    FloatingPointLoadStore, "cccc11001D111101dddd101zvvvvvvvv", vldm_vstm, 1),
    inst!("VLDR",                    FloatingPointLoadStore, "cccc1101UD01nnnndddd101zvvvvvvvv", vldr_vstr, 1),
    inst!("VSTR",                    FloatingPointLoadStore, "cccc1101UD00nnnndddd101zvvvvvvvv", vldr_vstr, 1),
    inst!("Undefined VSTM/VLDM",     Undefined,              "----11000-0---------101---------", undefined, 1),
    inst!("VSTM (A1)",               FloatingPointLoadStore, "cccc110puDw0nnnndddd1011vvvvvvvv", vldm_vstm, 1),
    inst!("VSTM (A2)",               FloatingPointLoadStore, "cccc110puDw0nnnndddd1010vvvvvvvv", vldm_vstm, 1),
    inst!("VLDM (A1)",               FloatingPointLoadStore, "cccc110puDw1nnnndddd1011vvvvvvvv", vldm_vstm, 1),
    inst!("VLDM (A2)",               FloatingPointLoadStore, "cccc110puDw1nnnndddd1010vvvvvvvv", vldm_vstm, 1),
];
