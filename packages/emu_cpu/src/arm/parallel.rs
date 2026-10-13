use crate::cpu::Cpu;
use crate::instruction::{Step, bits};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The results wrap and set the GE flags.
    Modulo,
    Saturating,
    Halving,
}

/// The lanes of an operation as pairs of the lane of Rn and the lane of Rm, and whether to add.
const ADD16: [(usize, usize, bool); 2] = [(0, 0, true), (1, 1, true)];
const ASX: [(usize, usize, bool); 2] = [(0, 1, false), (1, 0, true)];
const SAX: [(usize, usize, bool); 2] = [(0, 1, true), (1, 0, false)];
const SUB16: [(usize, usize, bool); 2] = [(0, 0, false), (1, 1, false)];
const ADD8: [(usize, usize, bool); 4] = [(0, 0, true), (1, 1, true), (2, 2, true), (3, 3, true)];
const SUB8: [(usize, usize, bool); 4] = [(0, 0, false), (1, 1, false), (2, 2, false), (3, 3, false)];

fn lane(value: u32, index: usize, width: u32, signed: bool) -> i64 {
    let raw = (value >> (index as u32 * width)) & ((1u32 << width) - 1);
    if signed {
        ((raw << (32 - width)) as i32 >> (32 - width)) as i64
    } else {
        raw as i64
    }
}

/// The parallel additions and subtractions. The bits 22:20 select the signedness and the mode,
/// and the bits 7:5 the operation.
pub fn parallel(cpu: &mut Cpu, raw: u32) -> Step {
    let (signed, mode) = match bits(raw, 22, 20) {
        0b001 => (true, Mode::Modulo),
        0b010 => (true, Mode::Saturating),
        0b011 => (true, Mode::Halving),
        0b101 => (false, Mode::Modulo),
        0b110 => (false, Mode::Saturating),
        _ => (false, Mode::Halving),
    };
    let (lanes, width): (&[(usize, usize, bool)], u32) = match bits(raw, 7, 5) {
        0b000 => (&ADD16, 16),
        0b001 => (&ASX, 16),
        0b010 => (&SAX, 16),
        0b011 => (&SUB16, 16),
        0b100 => (&ADD8, 8),
        _ => (&SUB8, 8),
    };
    let n = cpu.reg(bits(raw, 19, 16));
    let m = cpu.reg(bits(raw, 3, 0));
    let lane_mask = (1u32 << width) - 1;
    let flags_per_lane = 4 / lanes.len();

    let mut result = 0u32;
    let mut ge = 0u32;
    for (index, &(n_lane, m_lane, add)) in lanes.iter().enumerate() {
        let a = lane(n, n_lane, width, signed);
        let b = lane(m, m_lane, width, signed);
        let full = if add { a + b } else { a - b };
        let value = match mode {
            Mode::Modulo => {
                let passed = if signed {
                    full >= 0
                } else if add {
                    full >= 1 << width
                } else {
                    full >= 0
                };
                if passed {
                    ge |= ((1 << flags_per_lane) - 1) << (index * flags_per_lane);
                }
                full as u32
            }
            Mode::Saturating => {
                let (min, max) = if signed {
                    (-(1i64 << (width - 1)), (1i64 << (width - 1)) - 1)
                } else {
                    (0, (1i64 << width) - 1)
                };
                full.clamp(min, max) as u32
            }
            Mode::Halving => (full >> 1) as u32,
        };
        result |= (value & lane_mask) << (index as u32 * width);
    }
    if mode == Mode::Modulo {
        cpu.state().set_ge(ge);
    }
    cpu.write_alu(bits(raw, 15, 12), result)
}
