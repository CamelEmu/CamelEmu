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

use crate::bits::{bits, mask, pica_enum};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterType {
    Input,
    Output,
    Temporary,
    FloatUniform,
    IntUniform,
    BoolUniform,
    Address,
    ConditionalCode,
    Unknown,
}

pub fn get_register_name(ty: RegisterType) -> &'static str {
    match ty {
        RegisterType::Input => "v",
        RegisterType::Output => "o",
        RegisterType::Temporary => "r",
        RegisterType::FloatUniform => "c",
        RegisterType::IntUniform => "i",
        RegisterType::BoolUniform => "b",
        RegisterType::ConditionalCode => "cc",
        RegisterType::Unknown => "u",
        _ => "",
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceRegister(pub u32);

impl SourceRegister {
    pub fn get_register_type(self) -> RegisterType {
        if self.0 < 0x10 {
            RegisterType::Input
        } else if self.0 < 0x20 {
            RegisterType::Temporary
        } else {
            RegisterType::FloatUniform
        }
    }

    pub fn get_index(self) -> i32 {
        match self.get_register_type() {
            RegisterType::Input => self.0 as i32,
            RegisterType::Temporary => self.0 as i32 - 0x10,
            _ => self.0 as i32 - 0x20,
        }
    }

    pub fn from_type_and_index(ty: RegisterType, index: i32) -> Self {
        let mut reg = Self(0);
        if ty == RegisterType::Input {
            reg.0 = index as u32;
        } else if ty == RegisterType::Temporary {
            reg.0 = (index + 0x10) as u32;
        } else if ty == RegisterType::FloatUniform {
            reg.0 = (index + 0x20) as u32;
        } else {
            // TODO: Should throw an exception or something.
        }
        reg
    }

    pub fn make_input(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::Input, index)
    }

    pub fn make_temporary(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::Temporary, index)
    }

    pub fn make_float(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::FloatUniform, index)
    }

    pub fn get_name(self) -> String {
        format!(
            "{}{}",
            get_register_name(self.get_register_type()),
            self.get_index()
        )
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DestRegister(pub u32);

impl DestRegister {
    pub fn get_register_type(self) -> RegisterType {
        if self.0 < 0x10 {
            RegisterType::Output
        } else {
            RegisterType::Temporary
        }
    }

    pub fn get_index(self) -> i32 {
        match self.get_register_type() {
            RegisterType::Output => self.0 as i32,
            RegisterType::Temporary => self.0 as i32 - 0x10,
            // if (GetRegisterType() == RegisterType::FloatUniform)
            // TODO: This will lead to negative returned values...
            _ => self.0 as i32 - 0x20,
        }
    }

    pub fn from_type_and_index(ty: RegisterType, index: i32) -> Self {
        let mut reg = Self(0);
        if ty == RegisterType::Output {
            reg.0 = index as u32;
        } else if ty == RegisterType::Temporary {
            reg.0 = (index + 0x10) as u32;
        } else if ty == RegisterType::FloatUniform {
            // TODO: Wait what? These shouldn't be writable..
            reg.0 = (index + 0x20) as u32;
        } else {
            // TODO: Should throw an exception or something.
        }
        reg
    }

    pub fn make_output(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::Output, index)
    }

    pub fn make_temporary(index: i32) -> Self {
        Self::from_type_and_index(RegisterType::Temporary, index)
    }

    pub fn get_name(self) -> String {
        format!(
            "{}{}",
            get_register_name(self.get_register_type()),
            self.get_index()
        )
    }
}

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

pica_enum!(Selector {
    X = 0,
    Y = 1,
    Z = 2,
    W = 3,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwizzlePattern {
    pub hex: u32,
}

const _: () = assert!(
    size_of::<SwizzlePattern>() == 0x4,
    "Incorrect structure size"
);

impl SwizzlePattern {
    fn set(&mut self, position: u32, width: u32, value: u32) {
        let field = mask(width) << position;
        self.hex = (self.hex & !field) | ((value << position) & field);
    }

    /**
     * Gets the raw 8-bit selector for the specified (1-indexed) source register.
     */
    pub fn get_raw_selector(self, src: u32) -> u32 {
        if src == 0 || src > 3 {
            panic!("src needs to be between 1 and 3");
        }

        let selectors = [
            self.src1_selector(),
            self.src2_selector(),
            self.src3_selector(),
        ];
        selectors[src as usize - 1]
    }

    pub fn get_selector_src1(self, comp: usize) -> Selector {
        let selectors = [
            self.src1_selector_0(),
            self.src1_selector_1(),
            self.src1_selector_2(),
            self.src1_selector_3(),
        ];
        selectors[comp]
    }

    pub fn get_selector_src2(self, comp: usize) -> Selector {
        let selectors = [
            self.src2_selector_0(),
            self.src2_selector_1(),
            self.src2_selector_2(),
            self.src2_selector_3(),
        ];
        selectors[comp]
    }

    pub fn get_selector_src3(self, comp: usize) -> Selector {
        let selectors = [
            self.src3_selector_0(),
            self.src3_selector_1(),
            self.src3_selector_2(),
            self.src3_selector_3(),
        ];
        selectors[comp]
    }

    pub fn set_selector_src1(&mut self, comp: usize, value: Selector) {
        if comp > 3 {
            panic!("comp needs to be smaller than 4");
        }
        self.set(11 - 2 * comp as u32, 2, value.0);
    }

    pub fn set_selector_src2(&mut self, comp: usize, value: Selector) {
        if comp > 3 {
            panic!("comp needs to be smaller than 4");
        }
        self.set(20 - 2 * comp as u32, 2, value.0);
    }

    pub fn set_selector_src3(&mut self, comp: usize, value: Selector) {
        if comp > 3 {
            panic!("comp needs to be smaller than 4");
        }
        self.set(29 - 2 * comp as u32, 2, value.0);
    }

    pub fn selector_to_string(self, src2: bool) -> String {
        (0..4)
            .map(|i| {
                let selector = if src2 {
                    self.get_selector_src2(i)
                } else {
                    self.get_selector_src1(i)
                };
                ["x", "y", "z", "w"][selector.0 as usize]
            })
            .collect()
    }

    pub fn dest_component_enabled(self, i: u32) -> bool {
        (self.dest_mask() & (0x8 >> i)) != 0
    }

    pub fn set_dest_component_enabled(&mut self, i: u32, enabled: bool) {
        let mask = 0xffff & (0x8 >> i);
        let dest_mask = (self.dest_mask() & !mask) | (u32::from(enabled) * mask);
        self.set(0, 4, dest_mask);
    }

    pub fn dest_mask_to_string(self) -> String {
        (0..4)
            .map(|i| {
                if !self.dest_component_enabled(i) {
                    '_'
                } else {
                    ['x', 'y', 'z', 'w'][i as usize]
                }
            })
            .collect()
    }

    // Components of "dest" that should be written to: LSB=dest.w, MSB=dest.x
    pub fn dest_mask(self) -> u32 {
        bits(self.hex, 0, 4)
    }

    pub fn negate_src1(self) -> bool {
        bits(self.hex, 4, 1) != 0
    }

    pub fn src1_selector(self) -> u32 {
        bits(self.hex, 5, 8)
    }

    pub fn src1_selector_3(self) -> Selector {
        Selector(bits(self.hex, 5, 2))
    }

    pub fn src1_selector_2(self) -> Selector {
        Selector(bits(self.hex, 7, 2))
    }

    pub fn src1_selector_1(self) -> Selector {
        Selector(bits(self.hex, 9, 2))
    }

    pub fn src1_selector_0(self) -> Selector {
        Selector(bits(self.hex, 11, 2))
    }

    pub fn negate_src2(self) -> bool {
        bits(self.hex, 13, 1) != 0
    }

    pub fn src2_selector(self) -> u32 {
        bits(self.hex, 14, 8)
    }

    pub fn src2_selector_3(self) -> Selector {
        Selector(bits(self.hex, 14, 2))
    }

    pub fn src2_selector_2(self) -> Selector {
        Selector(bits(self.hex, 16, 2))
    }

    pub fn src2_selector_1(self) -> Selector {
        Selector(bits(self.hex, 18, 2))
    }

    pub fn src2_selector_0(self) -> Selector {
        Selector(bits(self.hex, 20, 2))
    }

    pub fn negate_src3(self) -> bool {
        bits(self.hex, 22, 1) != 0
    }

    pub fn src3_selector(self) -> u32 {
        bits(self.hex, 23, 8)
    }

    pub fn src3_selector_3(self) -> Selector {
        Selector(bits(self.hex, 23, 2))
    }

    pub fn src3_selector_2(self) -> Selector {
        Selector(bits(self.hex, 25, 2))
    }

    pub fn src3_selector_1(self) -> Selector {
        Selector(bits(self.hex, 27, 2))
    }

    pub fn src3_selector_0(self) -> Selector {
        Selector(bits(self.hex, 29, 2))
    }
}
