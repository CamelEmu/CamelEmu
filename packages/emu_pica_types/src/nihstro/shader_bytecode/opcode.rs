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

use crate::bits::pica_enum;

pica_enum!(OpCodeId {
    ADD = 0x00,
    DP3 = 0x01,
    DP4 = 0x02,
    // Dot product of Vec4 and Vec3; the Vec3 is made into
    // a Vec4 by appending 1.0 as the fourth component
    DPH = 0x03,
    // Distance, same as in vs_3_0
    DST = 0x04,
    // Base-2 exponential
    EX2 = 0x05,
    // Base-2 logarithm
    LG2 = 0x06,
    // Clamp for lighting
    LIT = 0x07,
    MUL = 0x08,
    // Set to 1.0 if SRC1 is greater or equal to SRC2
    SGE = 0x09,
    // Set to 1.0 if SRC1 is less than SRC2
    SLT = 0x0A,
    FLR = 0x0B,
    MAX = 0x0C,
    MIN = 0x0D,
    // Reciprocal
    RCP = 0x0E,
    // Reciprocal of square root
    RSQ = 0x0F,

    // Move to Address Register
    MOVA = 0x12,
    MOV = 0x13,

    DPHI = 0x18,
    DSTI = 0x19,
    SGEI = 0x1A,
    SLTI = 0x1B,

    BREAK = 0x20,
    NOP = 0x21,
    END = 0x22,
    BREAKC = 0x23,
    CALL = 0x24,
    CALLC = 0x25,
    CALLU = 0x26,
    IFU = 0x27,
    IFC = 0x28,
    LOOP = 0x29,
    EMIT = 0x2A,
    SETEMIT = 0x2B,
    JMPC = 0x2C,
    JMPU = 0x2D,
    // LSB opcode bit ignored
    CMP = 0x2E,

    // lower 3 opcode bits ignored for these
    MADI = 0x30,
    // lower 3 opcode bits ignored
    MAD = 0x38,

    // Pseudo-instructions, used internally by the assembler
    PSEUDO_INSTRUCTION_START = 0x40,

    // Generic IF (IFC or IFU)
    GEN_IF = 0x40,
    ELSE = 0x41,
    ENDIF = 0x42,
    // Generic CALL (CALL, CALC, or CALLU)
    GEN_CALL = 0x43,
    // Generic JMP (JMPC or JMPU)
    GEN_JMP = 0x44,
    //RET,          // Return from function (not supported yet)
    ENDLOOP = 0x45,
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpCodeType {
    // 3dbrew format 0
    Trivial,
    // 3dbrew format 1
    Arithmetic,
    // 3dbrew format 2
    Conditional,
    // 3dbrew format 3
    UniformFlowControl,
    // 3dbrew format 4
    SetEmit,
    // 3dbrew format 5
    MultiplyAdd,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
pub struct OpCodeInfo {
    pub ty: OpCodeType,
    pub subtype: u32,
    pub name: &'static str,
}

impl OpCodeInfo {
    // Arithmetic
    pub const OP_DESC: u32 = 1;
    pub const SRC1: u32 = 2;
    pub const SRC2: u32 = 4;
    pub const IDX: u32 = 8;
    pub const DEST: u32 = 16;
    pub const SRC_INVERSED: u32 = 32;
    pub const COMPARE_OPS: u32 = 64;
    pub const MOVA: u32 = 128 | Self::OP_DESC | Self::SRC1 | Self::IDX;
    pub const ONE_ARGUMENT: u32 = Self::OP_DESC | Self::SRC1 | Self::IDX | Self::DEST;
    pub const TWO_ARGUMENTS: u32 = Self::ONE_ARGUMENT | Self::SRC2;
    pub const COMPARE: u32 =
        Self::OP_DESC | Self::IDX | Self::SRC1 | Self::SRC2 | Self::COMPARE_OPS;

    // Flow Control
    pub const HAS_UNIFORM_INDEX: u32 = 1;
    pub const HAS_CONDITION: u32 = 2;
    // target code given explicitly and context-independently (contrary to e.g. BREAKC)
    pub const HAS_EXPLICIT_DEST: u32 = 4;
    // last instruction until returning to caller
    pub const HAS_FINISH_POINT: u32 = 8;
    // has an "else" branch
    pub const HAS_ALTERNATIVE: u32 = 16;
    pub const LOOP: u32 = 32;

    pub const BREAKC: u32 = Self::HAS_CONDITION;

    pub const JMP: u32 = Self::HAS_EXPLICIT_DEST;
    pub const JMPC: u32 = Self::JMP | Self::HAS_CONDITION;
    pub const JMPU: u32 = Self::JMP | Self::HAS_UNIFORM_INDEX;

    pub const CALL: u32 = Self::JMP | Self::HAS_FINISH_POINT;
    pub const CALLC: u32 = Self::CALL | Self::HAS_CONDITION;
    pub const CALLU: u32 = Self::CALL | Self::HAS_UNIFORM_INDEX;
    pub const IFU: u32 = Self::CALLU | Self::HAS_ALTERNATIVE;
    pub const IFC: u32 = Self::CALLC | Self::HAS_ALTERNATIVE;

    pub const FULL_AND_BOOL: u32 = 0;
    pub const SIMPLE_AND_INT: u32 = 1;

    // TODO: Deprecate.
    pub fn num_arguments(&self) -> usize {
        if self.ty == OpCodeType::Arithmetic {
            if self.subtype & Self::SRC2 != 0 {
                return 3;
            } else if self.subtype & Self::SRC1 != 0 {
                return 2;
            }
        }

        0
    }
}

const fn info(ty: OpCodeType, subtype: u32, name: &'static str) -> OpCodeInfo {
    OpCodeInfo { ty, subtype, name }
}

const UNKNOWN_INSTRUCTION: OpCodeInfo = info(OpCodeType::Unknown, 0, "UNK");

static INFO_TABLE: [OpCodeInfo; 64] = {
    use OpCodeType::*;
    const TWO: u32 = OpCodeInfo::TWO_ARGUMENTS;
    const ONE: u32 = OpCodeInfo::ONE_ARGUMENT;
    const TWO_INV: u32 = OpCodeInfo::TWO_ARGUMENTS | OpCodeInfo::SRC_INVERSED;
    [
        info(Arithmetic, TWO, "add"),
        info(Arithmetic, TWO, "dp3"),
        info(Arithmetic, TWO, "dp4"),
        info(Arithmetic, TWO, "dph"),
        info(Arithmetic, TWO, "dst"),
        info(Arithmetic, ONE, "exp"),
        info(Arithmetic, ONE, "log"),
        info(Arithmetic, ONE, "lit"),
        info(Arithmetic, TWO, "mul"),
        info(Arithmetic, TWO, "sge"),
        info(Arithmetic, TWO, "slt"),
        info(Arithmetic, ONE, "flr"),
        info(Arithmetic, TWO, "max"),
        info(Arithmetic, TWO, "min"),
        info(Arithmetic, ONE, "rcp"),
        info(Arithmetic, ONE, "rsq"),
        UNKNOWN_INSTRUCTION,
        UNKNOWN_INSTRUCTION,
        info(Arithmetic, OpCodeInfo::MOVA, "mova"),
        info(Arithmetic, ONE, "mov"),
        UNKNOWN_INSTRUCTION,
        UNKNOWN_INSTRUCTION,
        UNKNOWN_INSTRUCTION,
        UNKNOWN_INSTRUCTION,
        info(Arithmetic, TWO_INV, "dphi"),
        info(Arithmetic, TWO_INV, "dsti"),
        info(Arithmetic, TWO_INV, "sgei"),
        info(Arithmetic, TWO_INV, "slti"),
        UNKNOWN_INSTRUCTION,
        UNKNOWN_INSTRUCTION,
        UNKNOWN_INSTRUCTION,
        UNKNOWN_INSTRUCTION,
        info(Trivial, 0, "break"),
        info(Trivial, 0, "nop"),
        info(Trivial, 0, "end"),
        info(Conditional, OpCodeInfo::BREAKC, "breakc"),
        info(Conditional, OpCodeInfo::CALL, "call"),
        info(Conditional, OpCodeInfo::CALLC, "callc"),
        info(UniformFlowControl, OpCodeInfo::CALLU, "callu"),
        info(UniformFlowControl, OpCodeInfo::IFU, "ifu"),
        info(Conditional, OpCodeInfo::IFC, "ifc"),
        info(UniformFlowControl, OpCodeInfo::LOOP, "loop"),
        info(Trivial, 0, "emit"),
        info(SetEmit, 0, "setemit"),
        info(Conditional, OpCodeInfo::JMPC, "jmpc"),
        info(Conditional, OpCodeInfo::JMPU, "jmpu"),
        info(Arithmetic, OpCodeInfo::COMPARE, "cmp"),
        info(Arithmetic, OpCodeInfo::COMPARE, "cmp"),
        info(MultiplyAdd, OpCodeInfo::SRC_INVERSED, "madi"),
        info(MultiplyAdd, OpCodeInfo::SRC_INVERSED, "madi"),
        info(MultiplyAdd, OpCodeInfo::SRC_INVERSED, "madi"),
        info(MultiplyAdd, OpCodeInfo::SRC_INVERSED, "madi"),
        info(MultiplyAdd, OpCodeInfo::SRC_INVERSED, "madi"),
        info(MultiplyAdd, OpCodeInfo::SRC_INVERSED, "madi"),
        info(MultiplyAdd, OpCodeInfo::SRC_INVERSED, "madi"),
        info(MultiplyAdd, OpCodeInfo::SRC_INVERSED, "madi"),
        info(MultiplyAdd, 0, "mad"),
        info(MultiplyAdd, 0, "mad"),
        info(MultiplyAdd, 0, "mad"),
        info(MultiplyAdd, 0, "mad"),
        info(MultiplyAdd, 0, "mad"),
        info(MultiplyAdd, 0, "mad"),
        info(MultiplyAdd, 0, "mad"),
        info(MultiplyAdd, 0, "mad"),
    ]
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpCode(pub u32);

impl From<OpCodeId> for OpCode {
    fn from(id: OpCodeId) -> Self {
        Self(id.0)
    }
}

impl OpCode {
    pub fn effective_op_code(self) -> OpCodeId {
        let op = self.0;
        if OpCodeId(op & !0x7) == OpCodeId::MAD {
            OpCodeId::MAD
        } else if OpCodeId(op & !0x7) == OpCodeId::MADI {
            OpCodeId::MADI
        } else if OpCodeId(op & !0x1) == OpCodeId::CMP {
            OpCodeId::CMP
        } else {
            OpCodeId(self.0)
        }
    }

    pub fn get_info(self) -> &'static OpCodeInfo {
        &INFO_TABLE[self.0 as usize]
    }

    pub fn id(self) -> OpCodeId {
        OpCodeId(self.0)
    }
}
