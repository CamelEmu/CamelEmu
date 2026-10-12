//! The instructions of the ARM state.

pub mod alu;
mod branch;
mod data;
mod memory;
mod multiply;
mod parallel;
pub mod system;

use crate::instruction::{Instruction, bit, bits, inst};

fn dp_imm_cycles(raw: u32) -> u32 {
    if bits(raw, 15, 12) == 15 { 7 } else { 1 }
}

fn dp_reg_cycles(raw: u32) -> u32 {
    if bits(raw, 15, 12) == 15 { 7 } else { 1 }
}

fn dp_rsr_cycles(raw: u32) -> u32 {
    if bits(raw, 15, 12) == 15 { 8 } else { 2 }
}

fn multiply_cycles(raw: u32) -> u32 {
    if bit(raw, 20) { 5 } else { 2 }
}

fn multiply_long_cycles(raw: u32) -> u32 {
    if bit(raw, 20) { 6 } else { 3 }
}

fn msr_cycles(raw: u32) -> u32 {
    if bits(raw, 19, 16) == 0b1000 { 1 } else { 4 }
}

use branch::*;
use data::*;
use memory::*;
use multiply::*;
use parallel::parallel;
use system::*;

#[rustfmt::skip]
pub static TABLE: &[Instruction] = &[
    // Barrier instructions
    inst!("DMB",                 Barrier,             "1111010101111111111100000101oooo", barrier, 1),
    inst!("DSB",                 Barrier,             "1111010101111111111100000100oooo", barrier, 1),
    inst!("ISB",                 Barrier,             "1111010101111111111100000110oooo", barrier, 1),

    // Branch instructions
    inst!("BLX (imm)",           Branch,              "1111101hvvvvvvvvvvvvvvvvvvvvvvvv", blx_imm, 5),
    inst!("BLX (reg)",           Branch,              "cccc000100101111111111110011mmmm", blx_reg, 6),
    inst!("B",                   Branch,              "cccc1010vvvvvvvvvvvvvvvvvvvvvvvv", b, 4),
    inst!("BL",                  Branch,              "cccc1011vvvvvvvvvvvvvvvvvvvvvvvv", bl, 4),
    inst!("BX",                  Branch,              "cccc000100101111111111110001mmmm", bx, 5),
    inst!("BXJ",                 Branch,              "cccc000100101111111111110010mmmm", bx, 1),

    // CRC32 instructions, which the ARM11 does not have
    inst!("CRC32",               Undefined,           "cccc00010zz0nnnndddd00000100mmmm", undefined, 1),
    inst!("CRC32C",              Undefined,           "cccc00010zz0nnnndddd00100100mmmm", undefined, 1),

    // Coprocessor instructions
    inst!("CDP",                 Coprocessor,         "cccc1110ooooNNNNDDDDppppooo0MMMM", undefined, 1),
    inst!("LDC",                 Coprocessor,         "cccc110pudw1nnnnDDDDppppvvvvvvvv", undefined, 1),
    inst!("MCR",                 Coprocessor,         "cccc1110ooo0NNNNttttppppooo1MMMM", mcr, 2),
    inst!("MCRR",                Coprocessor,         "cccc11000100uuuuttttppppooooMMMM", undefined, 2),
    inst!("MRC",                 Coprocessor,         "cccc1110ooo1NNNNttttppppooo1MMMM", mrc, 2),
    inst!("MRRC",                Coprocessor,         "cccc11000101uuuuttttppppooooMMMM", undefined, 2),
    inst!("STC",                 Coprocessor,         "cccc110pudw0nnnnDDDDppppvvvvvvvv", undefined, 1),

    // Data Processing instructions
    inst!("ADC (imm)",           DataProcessing,      "cccc0010101Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("ADC (reg)",           DataProcessing,      "cccc0000101Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("ADC (rsr)",           DataProcessing,      "cccc0000101Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("ADD (imm)",           DataProcessing,      "cccc0010100Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("ADD (reg)",           DataProcessing,      "cccc0000100Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("ADD (rsr)",           DataProcessing,      "cccc0000100Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("AND (imm)",           DataProcessing,      "cccc0010000Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("AND (reg)",           DataProcessing,      "cccc0000000Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("AND (rsr)",           DataProcessing,      "cccc0000000Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("BIC (imm)",           DataProcessing,      "cccc0011110Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("BIC (reg)",           DataProcessing,      "cccc0001110Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("BIC (rsr)",           DataProcessing,      "cccc0001110Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("CMN (imm)",           DataProcessing,      "cccc00110111nnnn0000rrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("CMN (reg)",           DataProcessing,      "cccc00010111nnnn0000vvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("CMN (rsr)",           DataProcessing,      "cccc00010111nnnn0000ssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("CMP (imm)",           DataProcessing,      "cccc00110101nnnn0000rrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("CMP (reg)",           DataProcessing,      "cccc00010101nnnn0000vvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("CMP (rsr)",           DataProcessing,      "cccc00010101nnnn0000ssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("EOR (imm)",           DataProcessing,      "cccc0010001Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("EOR (reg)",           DataProcessing,      "cccc0000001Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("EOR (rsr)",           DataProcessing,      "cccc0000001Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("MOV (imm)",           DataProcessing,      "cccc0011101S0000ddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("MOV (reg)",           DataProcessing,      "cccc0001101S0000ddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("MOV (rsr)",           DataProcessing,      "cccc0001101S0000ddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("MVN (imm)",           DataProcessing,      "cccc0011111S0000ddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("MVN (reg)",           DataProcessing,      "cccc0001111S0000ddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("MVN (rsr)",           DataProcessing,      "cccc0001111S0000ddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("ORR (imm)",           DataProcessing,      "cccc0011100Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("ORR (reg)",           DataProcessing,      "cccc0001100Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("ORR (rsr)",           DataProcessing,      "cccc0001100Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("RSB (imm)",           DataProcessing,      "cccc0010011Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("RSB (reg)",           DataProcessing,      "cccc0000011Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("RSB (rsr)",           DataProcessing,      "cccc0000011Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("RSC (imm)",           DataProcessing,      "cccc0010111Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("RSC (reg)",           DataProcessing,      "cccc0000111Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("RSC (rsr)",           DataProcessing,      "cccc0000111Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("SBC (imm)",           DataProcessing,      "cccc0010110Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("SBC (reg)",           DataProcessing,      "cccc0000110Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("SBC (rsr)",           DataProcessing,      "cccc0000110Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("SUB (imm)",           DataProcessing,      "cccc0010010Snnnnddddrrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("SUB (reg)",           DataProcessing,      "cccc0000010Snnnnddddvvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("SUB (rsr)",           DataProcessing,      "cccc0000010Snnnnddddssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("TEQ (imm)",           DataProcessing,      "cccc00110011nnnn0000rrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("TEQ (reg)",           DataProcessing,      "cccc00010011nnnn0000vvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("TEQ (rsr)",           DataProcessing,      "cccc00010011nnnn0000ssss0rr1mmmm", dp_rsr, dp_rsr_cycles),
    inst!("TST (imm)",           DataProcessing,      "cccc00110001nnnn0000rrrrvvvvvvvv", dp_imm, dp_imm_cycles),
    inst!("TST (reg)",           DataProcessing,      "cccc00010001nnnn0000vvvvvrr0mmmm", dp_reg, dp_reg_cycles),
    inst!("TST (rsr)",           DataProcessing,      "cccc00010001nnnn0000ssss0rr1mmmm", dp_rsr, dp_rsr_cycles),

    // Exception Generating instructions
    inst!("BKPT",                ExceptionGenerating, "cccc00010010vvvvvvvvvvvv0111vvvv", bkpt, 8),
    inst!("SVC",                 ExceptionGenerating, "cccc1111vvvvvvvvvvvvvvvvvvvvvvvv", svc, 8),
    inst!("UDF",                 Undefined,           "111001111111------------1111----", undefined, 8),

    // Extension instructions
    inst!("SXTB",                Extension,           "cccc011010101111ddddrr000111mmmm", extend, 1),
    inst!("SXTB16",              Extension,           "cccc011010001111ddddrr000111mmmm", extend, 1),
    inst!("SXTH",                Extension,           "cccc011010111111ddddrr000111mmmm", extend, 1),
    inst!("SXTAB",               Extension,           "cccc01101010nnnnddddrr000111mmmm", extend, 1),
    inst!("SXTAB16",             Extension,           "cccc01101000nnnnddddrr000111mmmm", extend, 1),
    inst!("SXTAH",               Extension,           "cccc01101011nnnnddddrr000111mmmm", extend, 1),
    inst!("UXTB",                Extension,           "cccc011011101111ddddrr000111mmmm", extend, 1),
    inst!("UXTB16",              Extension,           "cccc011011001111ddddrr000111mmmm", extend, 1),
    inst!("UXTH",                Extension,           "cccc011011111111ddddrr000111mmmm", extend, 1),
    inst!("UXTAB",               Extension,           "cccc01101110nnnnddddrr000111mmmm", extend, 1),
    inst!("UXTAB16",             Extension,           "cccc01101100nnnnddddrr000111mmmm", extend, 1),
    inst!("UXTAH",               Extension,           "cccc01101111nnnnddddrr000111mmmm", extend, 1),

    // Hint instructions
    inst!("PLD (imm)",           Hint,                "11110101uz01nnnn1111iiiiiiiiiiii", nop, 1),
    inst!("PLD (reg)",           Hint,                "11110111uz01nnnn1111iiiiitt0mmmm", nop, 1),
    inst!("SEV",                 Hint,                "----0011001000001111000000000100", nop, 1),
    inst!("SEVL",                Hint,                "----0011001000001111000000000101", nop, 1),
    inst!("WFE",                 Hint,                "----0011001000001111000000000010", nop, 1),
    inst!("WFI",                 Hint,                "----0011001000001111000000000011", nop, 1),
    inst!("YIELD",               Hint,                "----0011001000001111000000000001", nop, 1),
    inst!("Reserved Hint",       Hint,                "----0011001000001111------------", nop, 1),
    inst!("Reserved Hint",       Hint,                "----001100100000111100000000----", nop, 1),

    // Synchronization Primitive instructions
    inst!("CLREX",               Synchronization,     "11110101011111111111000000011111", clrex, 1),
    inst!("SWP",                 Synchronization,     "cccc00010000nnnntttt00001001uuuu", swap, 4),
    inst!("SWPB",                Synchronization,     "cccc00010100nnnntttt00001001uuuu", swap, 4),
    inst!("STL",                 Undefined,           "cccc00011000nnnn111111001001tttt", undefined, 1),
    inst!("STLEX",               Undefined,           "cccc00011000nnnndddd11101001tttt", undefined, 1),
    inst!("STREX",               Synchronization,     "cccc00011000nnnndddd11111001mmmm", store_exclusive, 2),
    inst!("LDA",                 Undefined,           "cccc00011001nnnndddd110010011111", undefined, 1),
    inst!("LDAEX",               Undefined,           "cccc00011001nnnndddd111010011111", undefined, 1),
    inst!("LDREX",               Synchronization,     "cccc00011001nnnndddd111110011111", load_exclusive, 2),
    inst!("STLEXD",              Undefined,           "cccc00011010nnnndddd11101001mmmm", undefined, 1),
    inst!("STREXD",              Synchronization,     "cccc00011010nnnndddd11111001mmmm", store_exclusive, 2),
    inst!("LDAEXD",              Undefined,           "cccc00011011nnnndddd111010011111", undefined, 1),
    inst!("LDREXD",              Synchronization,     "cccc00011011nnnndddd111110011111", load_exclusive, 2),
    inst!("STLB",                Undefined,           "cccc00011100nnnn111111001001tttt", undefined, 1),
    inst!("STLEXB",              Undefined,           "cccc00011100nnnndddd11101001mmmm", undefined, 1),
    inst!("STREXB",              Synchronization,     "cccc00011100nnnndddd11111001mmmm", store_exclusive, 2),
    inst!("LDAB",                Undefined,           "cccc00011101nnnndddd110010011111", undefined, 1),
    inst!("LDAEXB",              Undefined,           "cccc00011101nnnndddd111010011111", undefined, 1),
    inst!("LDREXB",              Synchronization,     "cccc00011101nnnndddd111110011111", load_exclusive, 2),
    inst!("STLH",                Undefined,           "cccc00011110nnnn111111001001mmmm", undefined, 1),
    inst!("STLEXH",              Undefined,           "cccc00011110nnnndddd11101001mmmm", undefined, 1),
    inst!("STREXH",              Synchronization,     "cccc00011110nnnndddd11111001mmmm", store_exclusive, 2),
    inst!("LDAH",                Undefined,           "cccc00011111nnnndddd110010011111", undefined, 1),
    inst!("LDAEXH",              Undefined,           "cccc00011111nnnndddd111010011111", undefined, 1),
    inst!("LDREXH",              Synchronization,     "cccc00011111nnnndddd111110011111", load_exclusive, 2),

    // Load/Store instructions
    inst!("LDRBT (A1)",          LoadStore,           "----0100-111--------------------", single_imm, 1),
    inst!("LDRBT (A2)",          LoadStore,           "----0110-111---------------0----", single_reg, 1),
    inst!("LDRHT (A1)",          LoadStore,           "----0000-111------------1011----", extra, 1),
    inst!("LDRHT (A1)",          LoadStore,           "----0000-1111111--------1011----", extra, 1),
    inst!("LDRHT (A2)",          LoadStore,           "----0000-011--------00001011----", extra, 1),
    inst!("LDRSBT (A1)",         LoadStore,           "----0000-111------------1101----", extra, 1),
    inst!("LDRSBT (A2)",         LoadStore,           "----0000-011--------00001101----", extra, 1),
    inst!("LDRSHT (A1)",         LoadStore,           "----0000-111------------1111----", extra, 1),
    inst!("LDRSHT (A2)",         LoadStore,           "----0000-011--------00001111----", extra, 1),
    inst!("LDRT (A1)",           LoadStore,           "----0100-011--------------------", single_imm, 1),
    inst!("LDRT (A2)",           LoadStore,           "----0110-011---------------0----", single_reg, 1),
    inst!("STRBT (A1)",          LoadStore,           "----0100-110--------------------", single_imm, 1),
    inst!("STRBT (A2)",          LoadStore,           "----0110-110---------------0----", single_reg, 1),
    inst!("STRHT (A1)",          LoadStore,           "----0000-110------------1011----", extra, 1),
    inst!("STRHT (A2)",          LoadStore,           "----0000-010--------00001011----", extra, 1),
    inst!("STRT (A1)",           LoadStore,           "----0100-010--------------------", single_imm, 1),
    inst!("STRT (A2)",           LoadStore,           "----0110-010---------------0----", single_reg, 1),
    inst!("LDR (lit)",           LoadStore,           "cccc0101u0011111ttttvvvvvvvvvvvv", single_imm, 2),
    inst!("LDR (imm)",           LoadStore,           "cccc010pu0w1nnnnttttvvvvvvvvvvvv", single_imm, 2),
    inst!("LDR (reg)",           LoadStore,           "cccc011pu0w1nnnnttttvvvvvrr0mmmm", single_reg, single_reg_cycles),
    inst!("LDRB (lit)",          LoadStore,           "cccc0101u1011111ttttvvvvvvvvvvvv", single_imm, 2),
    inst!("LDRB (imm)",          LoadStore,           "cccc010pu1w1nnnnttttvvvvvvvvvvvv", single_imm, 2),
    inst!("LDRB (reg)",          LoadStore,           "cccc011pu1w1nnnnttttvvvvvrr0mmmm", single_reg, single_reg_cycles),
    inst!("LDRD (lit)",          LoadStore,           "cccc0001u1001111ttttvvvv1101vvvv", extra, 2),
    inst!("LDRD (imm)",          LoadStore,           "cccc000pu1w0nnnnttttvvvv1101vvvv", extra, 2),
    inst!("LDRD (reg)",          LoadStore,           "cccc000pu0w0nnnntttt00001101mmmm", extra, extra_reg_cycles),
    inst!("LDRH (lit)",          LoadStore,           "cccc000pu1w11111ttttvvvv1011vvvv", extra, 2),
    inst!("LDRH (imm)",          LoadStore,           "cccc000pu1w1nnnnttttvvvv1011vvvv", extra, 2),
    inst!("LDRH (reg)",          LoadStore,           "cccc000pu0w1nnnntttt00001011mmmm", extra, extra_reg_cycles),
    inst!("LDRSB (lit)",         LoadStore,           "cccc0001u1011111ttttvvvv1101vvvv", extra, 2),
    inst!("LDRSB (imm)",         LoadStore,           "cccc000pu1w1nnnnttttvvvv1101vvvv", extra, 2),
    inst!("LDRSB (reg)",         LoadStore,           "cccc000pu0w1nnnntttt00001101mmmm", extra, extra_reg_cycles),
    inst!("LDRSH (lit)",         LoadStore,           "cccc0001u1011111ttttvvvv1111vvvv", extra, 2),
    inst!("LDRSH (imm)",         LoadStore,           "cccc000pu1w1nnnnttttvvvv1111vvvv", extra, 2),
    inst!("LDRSH (reg)",         LoadStore,           "cccc000pu0w1nnnntttt00001111mmmm", extra, extra_reg_cycles),
    inst!("STR (imm)",           LoadStore,           "cccc010pu0w0nnnnttttvvvvvvvvvvvv", single_imm, 2),
    inst!("STR (reg)",           LoadStore,           "cccc011pu0w0nnnnttttvvvvvrr0mmmm", single_reg, single_reg_cycles),
    inst!("STRB (imm)",          LoadStore,           "cccc010pu1w0nnnnttttvvvvvvvvvvvv", single_imm, 2),
    inst!("STRB (reg)",          LoadStore,           "cccc011pu1w0nnnnttttvvvvvrr0mmmm", single_reg, single_reg_cycles),
    inst!("STRD (imm)",          LoadStore,           "cccc000pu1w0nnnnttttvvvv1111vvvv", extra, 2),
    inst!("STRD (reg)",          LoadStore,           "cccc000pu0w0nnnntttt00001111mmmm", extra, extra_reg_cycles),
    inst!("STRH (imm)",          LoadStore,           "cccc000pu1w0nnnnttttvvvv1011vvvv", extra, 2),
    inst!("STRH (reg)",          LoadStore,           "cccc000pu0w0nnnntttt00001011mmmm", extra, extra_reg_cycles),

    // Load/Store Multiple instructions
    inst!("LDM",                 LoadStoreMultiple,   "cccc100010w1nnnnxxxxxxxxxxxxxxxx", multiple, multiple_cycles),
    inst!("LDMDA",               LoadStoreMultiple,   "cccc100000w1nnnnxxxxxxxxxxxxxxxx", multiple, multiple_cycles),
    inst!("LDMDB",               LoadStoreMultiple,   "cccc100100w1nnnnxxxxxxxxxxxxxxxx", multiple, multiple_cycles),
    inst!("LDMIB",               LoadStoreMultiple,   "cccc100110w1nnnnxxxxxxxxxxxxxxxx", multiple, multiple_cycles),
    inst!("LDM (usr reg)",       LoadStoreMultiple,   "----100--101--------------------", multiple, 1),
    inst!("LDM (exce ret)",      LoadStoreMultiple,   "----100--1-1----1---------------", multiple, 1),
    inst!("STM",                 LoadStoreMultiple,   "cccc100010w0nnnnxxxxxxxxxxxxxxxx", multiple, multiple_cycles),
    inst!("STMDA",               LoadStoreMultiple,   "cccc100000w0nnnnxxxxxxxxxxxxxxxx", multiple, multiple_cycles),
    inst!("STMDB",               LoadStoreMultiple,   "cccc100100w0nnnnxxxxxxxxxxxxxxxx", multiple, multiple_cycles),
    inst!("STMIB",               LoadStoreMultiple,   "cccc100110w0nnnnxxxxxxxxxxxxxxxx", multiple, multiple_cycles),
    inst!("STM (usr reg)",       LoadStoreMultiple,   "----100--100--------------------", multiple, 1),

    // Miscellaneous instructions
    inst!("BFC",                 Miscellaneous,       "cccc0111110vvvvvddddvvvvv0011111", bfc, 1),
    inst!("BFI",                 Miscellaneous,       "cccc0111110vvvvvddddvvvvv001nnnn", bfi, 1),
    inst!("CLZ",                 Miscellaneous,       "cccc000101101111dddd11110001mmmm", clz, 1),
    inst!("MOVT",                Miscellaneous,       "cccc00110100vvvvddddvvvvvvvvvvvv", movt, 1),
    inst!("MOVW",                Miscellaneous,       "cccc00110000vvvvddddvvvvvvvvvvvv", movw, 1),
    inst!("NOP",                 Miscellaneous,       "----0011001000001111000000000000", nop, 1),
    inst!("SBFX",                Miscellaneous,       "cccc0111101wwwwwddddvvvvv101nnnn", sbfx, 1),
    inst!("SEL",                 Miscellaneous,       "cccc01101000nnnndddd11111011mmmm", sel, 1),
    inst!("UBFX",                Miscellaneous,       "cccc0111111wwwwwddddvvvvv101nnnn", ubfx, 1),

    // Unsigned Sum of Absolute Differences instructions
    inst!("USAD8",               Miscellaneous,       "cccc01111000dddd1111mmmm0001nnnn", usad8, 1),
    inst!("USADA8",              Miscellaneous,       "cccc01111000ddddaaaammmm0001nnnn", usada8, 1),

    // Packing instructions
    inst!("PKHBT",               Packing,             "cccc01101000nnnnddddvvvvv001mmmm", pkh, 1),
    inst!("PKHTB",               Packing,             "cccc01101000nnnnddddvvvvv101mmmm", pkh, 1),

    // Reversal instructions
    inst!("RBIT",                Reversal,            "cccc011011111111dddd11110011mmmm", rbit, 1),
    inst!("REV",                 Reversal,            "cccc011010111111dddd11110011mmmm", rev, 1),
    inst!("REV16",               Reversal,            "cccc011010111111dddd11111011mmmm", rev16, 1),
    inst!("REVSH",               Reversal,            "cccc011011111111dddd11111011mmmm", revsh, 1),

    // Saturation instructions
    inst!("SSAT",                Saturation,          "cccc0110101vvvvvddddvvvvvr01nnnn", ssat, 1),
    inst!("SSAT16",              Saturation,          "cccc01101010vvvvdddd11110011nnnn", ssat16, 1),
    inst!("USAT",                Saturation,          "cccc0110111vvvvvddddvvvvvr01nnnn", usat, 1),
    inst!("USAT16",              Saturation,          "cccc01101110vvvvdddd11110011nnnn", usat16, 1),

    // Divide instructions
    inst!("SDIV",                Divide,              "cccc01110001dddd1111mmmm0001nnnn", sdiv, 1),
    inst!("UDIV",                Divide,              "cccc01110011dddd1111mmmm0001nnnn", udiv, 1),

    // Multiply (Normal) instructions
    inst!("MLA",                 Multiply,            "cccc0000001Sddddaaaammmm1001nnnn", mla, multiply_cycles),
    inst!("MLS",                 Multiply,            "cccc00000110ddddaaaammmm1001nnnn", mls, 1),
    inst!("MUL",                 Multiply,            "cccc0000000Sdddd0000mmmm1001nnnn", mul, multiply_cycles),

    // Multiply (Long) instructions
    inst!("SMLAL",               Multiply,            "cccc0000111Sddddaaaammmm1001nnnn", multiply_long, multiply_long_cycles),
    inst!("SMULL",               Multiply,            "cccc0000110Sddddaaaammmm1001nnnn", multiply_long, multiply_long_cycles),
    inst!("UMAAL",               Multiply,            "cccc00000100ddddaaaammmm1001nnnn", umaal, 3),
    inst!("UMLAL",               Multiply,            "cccc0000101Sddddaaaammmm1001nnnn", multiply_long, multiply_long_cycles),
    inst!("UMULL",               Multiply,            "cccc0000100Sddddaaaammmm1001nnnn", multiply_long, multiply_long_cycles),

    // Multiply (Halfword) instructions
    inst!("SMLALXY",             Multiply,            "cccc00010100ddddaaaammmm1xy0nnnn", smlalxy, 2),
    inst!("SMLAXY",              Multiply,            "cccc00010000ddddaaaammmm1xy0nnnn", smlaxy, 1),
    inst!("SMULXY",              Multiply,            "cccc00010110dddd0000mmmm1xy0nnnn", smulxy, 1),

    // Multiply (Word by Halfword) instructions
    inst!("SMLAWY",              Multiply,            "cccc00010010ddddaaaammmm1y00nnnn", smlawy, 1),
    inst!("SMULWY",              Multiply,            "cccc00010010dddd0000mmmm1y10nnnn", smlawy, 1),

    // Multiply (Most Significant Word) instructions
    inst!("SMMUL",               Multiply,            "cccc01110101dddd1111mmmm00R1nnnn", smmul, 2),
    inst!("SMMLA",               Multiply,            "cccc01110101ddddaaaammmm00R1nnnn", smmul, 2),
    inst!("SMMLS",               Multiply,            "cccc01110101ddddaaaammmm11R1nnnn", smmul, 2),

    // Multiply (Dual) instructions
    inst!("SMLAD",               Multiply,            "cccc01110000ddddaaaammmm00M1nnnn", smlad, 2),
    inst!("SMLALD",              Multiply,            "cccc01110100ddddaaaammmm00M1nnnn", smlald, 2),
    inst!("SMLSD",               Multiply,            "cccc01110000ddddaaaammmm01M1nnnn", smlad, 2),
    inst!("SMLSLD",              Multiply,            "cccc01110100ddddaaaammmm01M1nnnn", smlald, 2),
    inst!("SMUAD",               Multiply,            "cccc01110000dddd1111mmmm00M1nnnn", smlad, 2),
    inst!("SMUSD",               Multiply,            "cccc01110000dddd1111mmmm01M1nnnn", smlad, 2),

    // Parallel Add/Subtract (Modulo) instructions
    inst!("SADD8",               ParallelArithmetic,  "cccc01100001nnnndddd11111001mmmm", parallel, 1),
    inst!("SADD16",              ParallelArithmetic,  "cccc01100001nnnndddd11110001mmmm", parallel, 1),
    inst!("SASX",                ParallelArithmetic,  "cccc01100001nnnndddd11110011mmmm", parallel, 1),
    inst!("SSAX",                ParallelArithmetic,  "cccc01100001nnnndddd11110101mmmm", parallel, 1),
    inst!("SSUB8",               ParallelArithmetic,  "cccc01100001nnnndddd11111111mmmm", parallel, 1),
    inst!("SSUB16",              ParallelArithmetic,  "cccc01100001nnnndddd11110111mmmm", parallel, 1),
    inst!("UADD8",               ParallelArithmetic,  "cccc01100101nnnndddd11111001mmmm", parallel, 1),
    inst!("UADD16",              ParallelArithmetic,  "cccc01100101nnnndddd11110001mmmm", parallel, 1),
    inst!("UASX",                ParallelArithmetic,  "cccc01100101nnnndddd11110011mmmm", parallel, 1),
    inst!("USAX",                ParallelArithmetic,  "cccc01100101nnnndddd11110101mmmm", parallel, 1),
    inst!("USUB8",               ParallelArithmetic,  "cccc01100101nnnndddd11111111mmmm", parallel, 1),
    inst!("USUB16",              ParallelArithmetic,  "cccc01100101nnnndddd11110111mmmm", parallel, 1),

    // Parallel Add/Subtract (Saturating) instructions
    inst!("QADD8",               ParallelArithmetic,  "cccc01100010nnnndddd11111001mmmm", parallel, 1),
    inst!("QADD16",              ParallelArithmetic,  "cccc01100010nnnndddd11110001mmmm", parallel, 1),
    inst!("QASX",                ParallelArithmetic,  "cccc01100010nnnndddd11110011mmmm", parallel, 1),
    inst!("QSAX",                ParallelArithmetic,  "cccc01100010nnnndddd11110101mmmm", parallel, 1),
    inst!("QSUB8",               ParallelArithmetic,  "cccc01100010nnnndddd11111111mmmm", parallel, 1),
    inst!("QSUB16",              ParallelArithmetic,  "cccc01100010nnnndddd11110111mmmm", parallel, 1),
    inst!("UQADD8",              ParallelArithmetic,  "cccc01100110nnnndddd11111001mmmm", parallel, 1),
    inst!("UQADD16",             ParallelArithmetic,  "cccc01100110nnnndddd11110001mmmm", parallel, 1),
    inst!("UQASX",               ParallelArithmetic,  "cccc01100110nnnndddd11110011mmmm", parallel, 1),
    inst!("UQSAX",               ParallelArithmetic,  "cccc01100110nnnndddd11110101mmmm", parallel, 1),
    inst!("UQSUB8",              ParallelArithmetic,  "cccc01100110nnnndddd11111111mmmm", parallel, 1),
    inst!("UQSUB16",             ParallelArithmetic,  "cccc01100110nnnndddd11110111mmmm", parallel, 1),

    // Parallel Add/Subtract (Halving) instructions
    inst!("SHADD8",              ParallelArithmetic,  "cccc01100011nnnndddd11111001mmmm", parallel, 1),
    inst!("SHADD16",             ParallelArithmetic,  "cccc01100011nnnndddd11110001mmmm", parallel, 1),
    inst!("SHASX",               ParallelArithmetic,  "cccc01100011nnnndddd11110011mmmm", parallel, 1),
    inst!("SHSAX",               ParallelArithmetic,  "cccc01100011nnnndddd11110101mmmm", parallel, 1),
    inst!("SHSUB8",              ParallelArithmetic,  "cccc01100011nnnndddd11111111mmmm", parallel, 1),
    inst!("SHSUB16",             ParallelArithmetic,  "cccc01100011nnnndddd11110111mmmm", parallel, 1),
    inst!("UHADD8",              ParallelArithmetic,  "cccc01100111nnnndddd11111001mmmm", parallel, 1),
    inst!("UHADD16",             ParallelArithmetic,  "cccc01100111nnnndddd11110001mmmm", parallel, 1),
    inst!("UHASX",               ParallelArithmetic,  "cccc01100111nnnndddd11110011mmmm", parallel, 1),
    inst!("UHSAX",               ParallelArithmetic,  "cccc01100111nnnndddd11110101mmmm", parallel, 1),
    inst!("UHSUB8",              ParallelArithmetic,  "cccc01100111nnnndddd11111111mmmm", parallel, 1),
    inst!("UHSUB16",             ParallelArithmetic,  "cccc01100111nnnndddd11110111mmmm", parallel, 1),

    // Saturated Add/Subtract instructions
    inst!("QADD",                Saturation,          "cccc00010000nnnndddd00000101mmmm", saturating_add_subtract, 1),
    inst!("QSUB",                Saturation,          "cccc00010010nnnndddd00000101mmmm", saturating_add_subtract, 1),
    inst!("QDADD",               Saturation,          "cccc00010100nnnndddd00000101mmmm", saturating_add_subtract, 1),
    inst!("QDSUB",               Saturation,          "cccc00010110nnnndddd00000101mmmm", saturating_add_subtract, 1),

    // Status Register Access instructions
    inst!("CPS",                 StatusRegister,      "111100010000---00000000---0-----", nop, 1),
    inst!("SETEND",              StatusRegister,      "1111000100000001000000e000000000", setend, 1),
    inst!("MRS",                 StatusRegister,      "cccc000100001111dddd000000000000", mrs, 1),
    inst!("MSR (imm)",           StatusRegister,      "cccc00110010mmmm1111rrrrvvvvvvvv", msr_imm, msr_cycles),
    inst!("MSR (reg)",           StatusRegister,      "cccc00010010mmmm111100000000nnnn", msr_reg, msr_cycles),
    inst!("RFE",                 Undefined,           "1111100--0-1----0000101000000000", undefined, 9),
    inst!("SRS",                 Undefined,           "1111100--1-0110100000101000-----", undefined, 1),
];
