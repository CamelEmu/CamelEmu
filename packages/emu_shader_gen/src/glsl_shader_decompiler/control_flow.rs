use std::collections::{BTreeMap, BTreeSet};

use emu_pica_types::nihstro::shader_bytecode::{Instruction, OpCodeId};

use super::{DecompileFail, PROGRAM_END, ProgramCode};

/// Describes the behaviour of code path of a given entry point and a return point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExitMethod {
    Undetermined, // Internal value. Only occur when analyzing JMP loop.
    AlwaysReturn, // All code paths reach the return point.
    Conditional,  // Code path reaches the return point or an END instruction conditionally.
    AlwaysEnd,    // All code paths reach a END instruction.
}

/// A subroutine is a range of code refereced by a CALL, IF or LOOP instruction.
#[derive(Clone, Debug)]
pub(super) struct Subroutine {
    pub begin: u32,              // Entry point of the subroutine.
    pub end: u32,                // Return point of the subroutine.
    pub exit_method: ExitMethod, // Exit method of the subroutine.
    pub labels: BTreeSet<u32>,   // Addresses refereced by JMP instructions.
}

impl Subroutine {
    /// Generates a name suitable for GLSL source code.
    pub fn get_name(&self) -> String {
        format!("sub_{}_{}", self.begin, self.end)
    }
}

/// The subroutines in the order of `std::set<Subroutine>`, by begin and end.
pub(super) type Subroutines = BTreeMap<(u32, u32), Subroutine>;

/// Analyzes shader code and produces a set of subroutines.
pub(super) struct ControlFlowAnalyzer<'a> {
    program_code: &'a ProgramCode,
    subroutines: Subroutines,
    exit_method_map: BTreeMap<(u32, u32), ExitMethod>,
}

impl<'a> ControlFlowAnalyzer<'a> {
    pub fn new(program_code: &'a ProgramCode, main_offset: u32) -> Result<Self, DecompileFail> {
        let mut analyzer = Self {
            program_code,
            subroutines: Subroutines::new(),
            exit_method_map: BTreeMap::new(),
        };

        // Recursively finds all subroutines.
        let program_main = analyzer.add_subroutine(main_offset, PROGRAM_END)?;
        if program_main != ExitMethod::AlwaysEnd {
            return Err(DecompileFail("Program does not always end"));
        }
        Ok(analyzer)
    }

    pub fn move_subroutines(self) -> Subroutines {
        self.subroutines
    }

    /// Adds and analyzes a new subroutine if it is not added yet.
    fn add_subroutine(&mut self, begin: u32, end: u32) -> Result<ExitMethod, DecompileFail> {
        if let Some(subroutine) = self.subroutines.get(&(begin, end)) {
            return Ok(subroutine.exit_method);
        }

        let mut labels = BTreeSet::new();
        let exit_method = self.scan(begin, end, &mut labels)?;
        if exit_method == ExitMethod::Undetermined {
            return Err(DecompileFail("Recursive function detected"));
        }
        let subroutine = self.subroutines.entry((begin, end)).or_insert(Subroutine {
            begin,
            end,
            exit_method,
            labels,
        });
        Ok(subroutine.exit_method)
    }

    /// Merges exit method of two parallel branches.
    fn parallel_exit(a: ExitMethod, b: ExitMethod) -> ExitMethod {
        if a == ExitMethod::Undetermined {
            return b;
        }
        if b == ExitMethod::Undetermined {
            return a;
        }
        if a == b {
            return a;
        }
        ExitMethod::Conditional
    }

    /// Cascades exit method of two blocks of code.
    fn series_exit(a: ExitMethod, b: ExitMethod) -> ExitMethod {
        // This should be handled before evaluating b.
        debug_assert!(a != ExitMethod::AlwaysEnd);

        if a == ExitMethod::Undetermined {
            return ExitMethod::Undetermined;
        }

        if a == ExitMethod::AlwaysReturn {
            return b;
        }

        if b == ExitMethod::Undetermined || b == ExitMethod::AlwaysEnd {
            return ExitMethod::AlwaysEnd;
        }

        ExitMethod::Conditional
    }

    /// Scans a range of code for labels and determines the exit method.
    fn scan(
        &mut self,
        begin: u32,
        end: u32,
        labels: &mut BTreeSet<u32>,
    ) -> Result<ExitMethod, DecompileFail> {
        if let Some(&exit_method) = self.exit_method_map.get(&(begin, end)) {
            return Ok(exit_method);
        }
        self.exit_method_map
            .insert((begin, end), ExitMethod::Undetermined);

        let exit_method = self.scan_range(begin, end, labels)?;
        self.exit_method_map.insert((begin, end), exit_method);
        Ok(exit_method)
    }

    fn scan_range(
        &mut self,
        begin: u32,
        end: u32,
        labels: &mut BTreeSet<u32>,
    ) -> Result<ExitMethod, DecompileFail> {
        let mut offset = begin;
        while offset != end && offset != PROGRAM_END {
            let instr = Instruction {
                hex: self.program_code[offset as usize],
            };
            let flow_control = instr.flow_control();
            match instr.opcode().id() {
                OpCodeId::END => {
                    return Ok(ExitMethod::AlwaysEnd);
                }
                OpCodeId::JMPC | OpCodeId::JMPU => {
                    labels.insert(flow_control.dest_offset());
                    let no_jmp = self.scan(offset + 1, end, labels)?;
                    let jmp = self.scan(flow_control.dest_offset(), end, labels)?;
                    return Ok(Self::parallel_exit(no_jmp, jmp));
                }
                OpCodeId::CALL => {
                    let call = self.add_subroutine(
                        flow_control.dest_offset(),
                        flow_control.dest_offset() + flow_control.num_instructions(),
                    )?;
                    if call == ExitMethod::AlwaysEnd {
                        return Ok(ExitMethod::AlwaysEnd);
                    }
                    let after_call = self.scan(offset + 1, end, labels)?;
                    return Ok(Self::series_exit(call, after_call));
                }
                OpCodeId::LOOP => {
                    let loop_exit =
                        self.add_subroutine(offset + 1, flow_control.dest_offset() + 1)?;
                    if loop_exit == ExitMethod::AlwaysEnd {
                        return Ok(ExitMethod::AlwaysEnd);
                    }
                    let after_loop = self.scan(flow_control.dest_offset() + 1, end, labels)?;
                    return Ok(Self::series_exit(loop_exit, after_loop));
                }
                OpCodeId::CALLC | OpCodeId::CALLU => {
                    let call = self.add_subroutine(
                        flow_control.dest_offset(),
                        flow_control.dest_offset() + flow_control.num_instructions(),
                    )?;
                    let after_call = self.scan(offset + 1, end, labels)?;
                    return Ok(Self::series_exit(
                        Self::parallel_exit(call, ExitMethod::AlwaysReturn),
                        after_call,
                    ));
                }
                OpCodeId::IFU | OpCodeId::IFC => {
                    let if_sub = self.add_subroutine(offset + 1, flow_control.dest_offset())?;
                    let else_method = if flow_control.num_instructions() != 0 {
                        self.add_subroutine(
                            flow_control.dest_offset(),
                            flow_control.dest_offset() + flow_control.num_instructions(),
                        )?
                    } else {
                        ExitMethod::AlwaysReturn
                    };

                    let both = Self::parallel_exit(if_sub, else_method);
                    if both == ExitMethod::AlwaysEnd {
                        return Ok(ExitMethod::AlwaysEnd);
                    }
                    let after_call = self.scan(
                        flow_control.dest_offset() + flow_control.num_instructions(),
                        end,
                        labels,
                    )?;
                    return Ok(Self::series_exit(both, after_call));
                }
                _ => {}
            }
            offset += 1;
        }
        Ok(ExitMethod::AlwaysReturn)
    }
}
