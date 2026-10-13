// Copyright 2014 Tony Wasserka
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
//     * Redistributions of source code must retain the above copyright
//       notice, this list of conditions and the following disclaimer.
//     * Redistributions in binary form must reproduce the above copyright
//       notice, this list of conditions and the following disclaimer in the
//       documentation and/or other materials provided with the distribution.
//     * Neither the name of the owner nor the names of its contributors may
//       be used to endorse or promote products derived from this software
//       without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use super::opcode::OpCode;
use super::register::{DestRegister, SourceRegister};
use crate::bits::{bits, pica_enum};

pica_enum!(CompareOp {
    EQUAL = 0,
    NOT_EQUAL = 1,
    LESS_THAN = 2,
    LESS_EQUAL = 3,
    GREATER_THAN = 4,
    GREATER_EQUAL = 5,
    UNK6 = 6,
    UNK7 = 7,
});

impl CompareOp {
    pub fn to_str(self) -> &'static str {
        match self {
            Self::EQUAL => "==",
            Self::NOT_EQUAL => "!=",
            Self::LESS_THAN => "<",
            Self::LESS_EQUAL => "<=",
            Self::GREATER_THAN => ">",
            Self::GREATER_EQUAL => ">=",
            Self::UNK6 => "UNK6",
            Self::UNK7 => "UNK7",
            _ => "",
        }
    }
}

pica_enum!(FlowControlOp {
    OR = 0,
    AND = 1,
    JUST_X = 2,
    JUST_Y = 3,
});

fn address_register_name(address_register_index: u32) -> &'static str {
    if address_register_index == 0 {
        ""
    } else if address_register_index == 1 {
        "a0.x"
    } else if address_register_index == 2 {
        "a0.y"
    } else {
        /*if (address_register_index == 3)*/
        "aL"
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Instruction {
    pub hex: u32,
}

const _: () = assert!(size_of::<Instruction>() == 0x4, "Incorrect structure size");

impl Instruction {
    pub fn opcode(self) -> OpCode {
        OpCode(bits(self.hex, 0x1a, 0x6))
    }

    // General notes:
    //
    // When two input registers are used, one of them uses a 5-bit index while the other
    // one uses a 7-bit index. This is because at most one floating point uniform may be used
    // as an input.

    // Format used e.g. by arithmetic instructions and comparisons
    pub fn common(self) -> Common {
        Common { hex: self.hex }
    }

    pub fn flow_control(self) -> FlowControl {
        FlowControl { hex: self.hex }
    }

    pub fn mad(self) -> Mad {
        Mad { hex: self.hex }
    }

    pub fn setemit(self) -> SetEmit {
        SetEmit { hex: self.hex }
    }
}

// TODO: Remove name
#[derive(Clone, Copy, Debug)]
pub struct Common {
    hex: u32,
}

impl Common {
    pub fn operand_desc_id(self) -> u32 {
        bits(self.hex, 0x00, 0x7)
    }

    pub fn get_src1(self, is_inverted: bool) -> SourceRegister {
        if !is_inverted {
            self.src1()
        } else {
            self.src1i()
        }
    }

    pub fn get_src2(self, is_inverted: bool) -> SourceRegister {
        if !is_inverted {
            self.src2()
        } else {
            self.src2i()
        }
    }

    /**
     * Source inputs may be reordered for certain instructions.
     * Use GetSrc1 and GetSrc2 instead to access the input register indices hence.
     */
    pub fn src2(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x07, 0x5))
    }

    pub fn src1(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x0c, 0x7))
    }

    pub fn src2i(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x07, 0x7))
    }

    pub fn src1i(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x0e, 0x5))
    }

    // Address register value is used for relative addressing of src1 / src2 (inverted)
    pub fn address_register_index(self) -> u32 {
        bits(self.hex, 0x13, 0x2)
    }

    // TODO: Make nameless once MSVC supports it
    pub fn compare_op(self) -> CompareOpType {
        CompareOpType { hex: self.hex }
    }

    pub fn address_register_name(self) -> &'static str {
        address_register_name(self.address_register_index())
    }

    pub fn dest(self) -> DestRegister {
        DestRegister(bits(self.hex, 0x15, 0x5))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CompareOpType {
    hex: u32,
}

impl CompareOpType {
    pub fn y(self) -> CompareOp {
        CompareOp(bits(self.hex, 0x15, 0x3))
    }

    pub fn x(self) -> CompareOp {
        CompareOp(bits(self.hex, 0x18, 0x3))
    }

    pub fn to_str(self, op: CompareOp) -> &'static str {
        op.to_str()
    }
}

// TODO: Make nameless once MSVC supports it
#[derive(Clone, Copy, Debug)]
pub struct FlowControl {
    hex: u32,
}

impl FlowControl {
    pub fn num_instructions(self) -> u32 {
        bits(self.hex, 0x00, 0x8)
    }

    pub fn dest_offset(self) -> u32 {
        bits(self.hex, 0x0a, 0xc)
    }

    pub fn op(self) -> FlowControlOp {
        FlowControlOp(bits(self.hex, 0x16, 0x2))
    }

    pub fn bool_uniform_id(self) -> u32 {
        bits(self.hex, 0x16, 0x4)
    }

    // TODO: Verify that only this many bits are used...
    pub fn int_uniform_id(self) -> u32 {
        bits(self.hex, 0x16, 0x2)
    }

    pub fn refy(self) -> bool {
        bits(self.hex, 0x18, 1) != 0
    }

    pub fn refx(self) -> bool {
        bits(self.hex, 0x19, 1) != 0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Mad {
    hex: u32,
}

impl Mad {
    pub fn get_src1(self, _is_inverted: bool) -> SourceRegister {
        // The inverted form for src1 is the same, this function is just here for consistency
        self.src1()
    }

    pub fn get_src2(self, is_inverted: bool) -> SourceRegister {
        if !is_inverted {
            self.src2()
        } else {
            self.src2i()
        }
    }

    pub fn get_src3(self, is_inverted: bool) -> SourceRegister {
        if !is_inverted {
            self.src3()
        } else {
            self.src3i()
        }
    }

    pub fn operand_desc_id(self) -> u32 {
        bits(self.hex, 0x00, 0x5)
    }

    pub fn src3(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x05, 0x5))
    }

    pub fn src2(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x0a, 0x7))
    }

    pub fn src1(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x11, 0x5))
    }

    pub fn src3i(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x05, 0x7))
    }

    pub fn src2i(self) -> SourceRegister {
        SourceRegister(bits(self.hex, 0x0c, 0x5))
    }

    // Address register value is used for relative addressing of src2 / src3 (inverted)
    pub fn address_register_index(self) -> u32 {
        bits(self.hex, 0x16, 0x2)
    }

    pub fn address_register_name(self) -> &'static str {
        address_register_name(self.address_register_index())
    }

    pub fn dest(self) -> DestRegister {
        DestRegister(bits(self.hex, 0x18, 0x5))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SetEmit {
    hex: u32,
}

impl SetEmit {
    pub fn winding(self) -> u32 {
        bits(self.hex, 0x16, 1)
    }

    pub fn prim_emit(self) -> u32 {
        bits(self.hex, 0x17, 1)
    }

    pub fn vertex_id(self) -> u32 {
        bits(self.hex, 0x18, 2)
    }
}
