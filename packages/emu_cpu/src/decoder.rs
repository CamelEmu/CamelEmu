use std::sync::OnceLock;

use crate::arm::system::undefined;
use crate::instruction::{Instruction, Kind, sorted};
use crate::{arm, thumb, vfp};

static UNDEFINED: Instruction = Instruction::new(
    "Undefined",
    Kind::Undefined,
    "--------------------------------",
    undefined,
    crate::instruction::Cycles::Fixed(1),
);

/// The index of the lookup of the ARM table, from the bits 27:20 and 7:4 as dynarmic does.
fn arm_lookup_index(raw: u32) -> usize {
    (((raw >> 4) & 0x00F) | ((raw >> 16) & 0xFF0)) as usize
}

struct Tables {
    arm: Vec<Vec<&'static Instruction>>,
    vfp_unconditional: Vec<&'static Instruction>,
    vfp_conditional: Vec<&'static Instruction>,
    thumb: Vec<Vec<&'static Instruction>>,
    thumb32: Vec<&'static Instruction>,
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let arm_list = sorted(arm::TABLE);
        let arm = (0..0x1000)
            .map(|index| {
                arm_list
                    .iter()
                    .copied()
                    .filter(|instruction| {
                        let mask = arm_lookup_index(instruction.mask);
                        let expect = arm_lookup_index(instruction.expect);
                        index & mask == expect
                    })
                    .collect()
            })
            .collect();

        let (vfp_unconditional, vfp_conditional) = vfp::TABLE
            .iter()
            .partition(|instruction| instruction.mask & 0xF000_0000 == 0xF000_0000);

        let thumb_list = sorted(thumb::TABLE);
        let thumb = (0..0x100u32)
            .map(|index| {
                thumb_list
                    .iter()
                    .copied()
                    .filter(|instruction| {
                        (index << 8) & instruction.mask & 0xFF00 == instruction.expect & 0xFF00
                    })
                    .collect()
            })
            .collect();

        Tables {
            arm,
            vfp_unconditional,
            vfp_conditional,
            thumb,
            thumb32: sorted(thumb::TABLE32),
        }
    })
}

/// Finds the instruction of an ARM encoding, trying VFP first as dynarmic does.
pub fn decode_arm(raw: u32) -> &'static Instruction {
    let tables = tables();
    let vfp = if raw & 0xF000_0000 == 0xF000_0000 {
        &tables.vfp_unconditional
    } else {
        &tables.vfp_conditional
    };
    if let Some(instruction) = vfp.iter().find(|instruction| instruction.matches(raw)) {
        return instruction;
    }
    tables.arm[arm_lookup_index(raw)]
        .iter()
        .find(|instruction| instruction.matches(raw))
        .copied()
        .unwrap_or(&UNDEFINED)
}

pub fn decode_thumb16(raw: u16) -> &'static Instruction {
    let raw = raw as u32;
    tables().thumb[(raw >> 8) as usize]
        .iter()
        .find(|instruction| instruction.matches(raw))
        .copied()
        .unwrap_or(&UNDEFINED)
}

/// Finds a 32-bit Thumb instruction from its two halves, if they form one.
pub fn decode_thumb32(first: u16, second: u16) -> Option<&'static Instruction> {
    let raw = ((first as u32) << 16) | second as u32;
    tables().thumb32.iter().find(|instruction| instruction.matches(raw)).copied()
}

/// Whether a halfword starts a 32-bit Thumb instruction.
pub fn is_thumb32_prefix(first: u16) -> bool {
    first >> 11 == 0b11110
}
