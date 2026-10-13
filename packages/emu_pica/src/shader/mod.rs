pub mod shader_interpreter;

use crate::shader_setup::ShaderSetup;
use crate::shader_unit::ShaderUnit;

pub trait ShaderEngine {
    /**
     * Performs any shader unit setup that only needs to happen once per shader (as opposed to once
     * per vertex, which would happen within the `Run` function).
     */
    fn setup_batch(&mut self, setup: &mut ShaderSetup, entry_point: u32);

    /**
     * Runs the currently setup shader.
     *
     * @param setup Shader engine state, must be setup with SetupBatch on each shader change.
     * @param state Shader unit state, must be setup with input data before each shader invocation.
     */
    fn run(&self, setup: &ShaderSetup, state: &mut ShaderUnit);
}

pub fn create_engine(_use_jit: bool) -> Box<dyn ShaderEngine> {
    Box::new(shader_interpreter::InterpreterEngine)
}
