mod control_flow;
mod generator;
mod instruction;
mod writer;

use control_flow::ControlFlowAnalyzer;
use generator::GlslGenerator;

pub const MAX_PROGRAM_CODE_LENGTH: usize = 4096;
pub const MAX_SWIZZLE_DATA_LENGTH: usize = 4096;

pub type ProgramCode = [u32; MAX_PROGRAM_CODE_LENGTH];
pub type SwizzleData = [u32; MAX_SWIZZLE_DATA_LENGTH];

pub type RegGetter<'a> = &'a mut dyn FnMut(u32) -> String;

const PROGRAM_END: u32 = MAX_PROGRAM_CODE_LENGTH as u32;

#[derive(Debug)]
struct DecompileFail(&'static str);

pub fn decompile_program(
    program_code: &ProgramCode,
    swizzle_data: &SwizzleData,
    main_offset: u32,
    inputreg_getter: RegGetter,
    outputreg_getter: RegGetter,
    sanitize_mul: bool,
) -> String {
    let result = ControlFlowAnalyzer::new(program_code, main_offset).and_then(|analyzer| {
        let subroutines = analyzer.move_subroutines();
        GlslGenerator::new(
            &subroutines,
            program_code,
            swizzle_data,
            main_offset,
            inputreg_getter,
            outputreg_getter,
            sanitize_mul,
        )
        .move_shader_code()
    });
    match result {
        Ok(source) => source,
        Err(DecompileFail(message)) => {
            log::info!(target: "HW_GPU", "Shader decompilation failed: {}", message);
            String::new()
        }
    }
}
