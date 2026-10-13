use crate::cpu::Cpu;

/// What an instruction does, which the translator uses to decide where a block ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Barrier,
    Branch,
    Coprocessor,
    DataProcessing,
    Divide,
    ExceptionGenerating,
    Extension,
    FloatingPoint,
    FloatingPointLoadStore,
    FloatingPointTransfer,
    Hint,
    LoadStore,
    LoadStoreMultiple,
    Miscellaneous,
    Multiply,
    Packing,
    ParallelArithmetic,
    Reversal,
    Saturation,
    StatusRegister,
    Synchronization,
    Undefined,
}

impl Kind {
    /// Whether the instruction always leaves the block, so nothing after it is translated.
    pub fn ends_block(self) -> bool {
        matches!(self, Kind::Branch | Kind::ExceptionGenerating | Kind::Undefined)
    }
}

/// What the execution of one instruction asks of the dispatcher. The native code returns it as
/// its value.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Continue = 0,
    Branch = 1,
    Exit = 2,
}

pub type Execute = fn(&mut Cpu, u32) -> Step;

/// The cycles that an instruction takes, as `Core::TicksForInstruction` of the core counts them.
#[derive(Clone, Copy)]
pub enum Cycles {
    Fixed(u32),
    Computed(fn(u32) -> u32),
}

impl Cycles {
    pub fn of(self, raw: u32) -> u32 {
        match self {
            Cycles::Fixed(cycles) => cycles,
            Cycles::Computed(cycles) => cycles(raw),
        }
    }
}

/// An instruction of the table: what kind it is, its name, its encoding and what it executes.
pub struct Instruction {
    pub name: &'static str,
    pub kind: Kind,
    pub mask: u32,
    pub expect: u32,
    pub execute: Execute,
    pub cycles: Cycles,
}

impl std::fmt::Debug for Instruction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} ({:?})", self.name, self.kind)
    }
}

impl Instruction {
    /// `pattern` is the encoding from the most significant bit, in which `0` and `1` are fixed
    /// bits and any other character is a field.
    pub const fn new(
        name: &'static str,
        kind: Kind,
        pattern: &'static str,
        execute: Execute,
        cycles: Cycles,
    ) -> Self {
        let bytes = pattern.as_bytes();
        let mut mask = 0u32;
        let mut expect = 0u32;
        let mut i = 0;
        while i < bytes.len() {
            let bit = 1u32 << (bytes.len() - 1 - i);
            match bytes[i] {
                b'0' => mask |= bit,
                b'1' => {
                    mask |= bit;
                    expect |= bit;
                }
                _ => {}
            }
            i += 1;
        }
        Self {
            name,
            kind,
            mask,
            expect,
            execute,
            cycles,
        }
    }

    pub fn matches(&self, raw: u32) -> bool {
        raw & self.mask == self.expect
    }
}

/// Builds an entry of an instruction table.
macro_rules! inst {
    ($name:literal, $kind:ident, $pattern:literal, $execute:expr, $cycles:literal) => {
        $crate::instruction::Instruction::new(
            $name,
            $crate::instruction::Kind::$kind,
            $pattern,
            $execute,
            $crate::instruction::Cycles::Fixed($cycles),
        )
    };
    ($name:literal, $kind:ident, $pattern:literal, $execute:expr, $cycles:expr) => {
        $crate::instruction::Instruction::new(
            $name,
            $crate::instruction::Kind::$kind,
            $pattern,
            $execute,
            $crate::instruction::Cycles::Computed($cycles),
        )
    };
}
pub(crate) use inst;

/// Orders a table so that the entries with more fixed bits, which are more specific, come first.
pub fn sorted(table: &'static [Instruction]) -> Vec<&'static Instruction> {
    let mut list: Vec<_> = table.iter().collect();
    list.sort_by_key(|instruction| std::cmp::Reverse(instruction.mask.count_ones()));
    list
}

/// Returns the bits `high:low` of `value`.
#[inline(always)]
pub fn bits(value: u32, high: u32, low: u32) -> u32 {
    (value >> low) & (u32::MAX >> (31 - (high - low)))
}

#[inline(always)]
pub fn bit(value: u32, index: u32) -> bool {
    (value >> index) & 1 != 0
}
