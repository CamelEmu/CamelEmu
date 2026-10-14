use emu_pica_types::{AttributeBuffer, F24, Vec4};

use crate::regs::RegsInternal;
use crate::regs::pipeline::{GsMode, UseGs};
use crate::regs::shader::ShaderMode;
use crate::shader::ShaderEngine;
use crate::shader_setup::ShaderSetup;
use crate::shader_unit::{GeometryShaderUnit, VertexHandler};

/// The state of the PICA that the backends read and write, which the core holds by reference.
struct BackendContext<'a> {
    regs: &'a RegsInternal,
    unit: &'a mut GeometryShaderUnit,
    setup: &'a mut ShaderSetup,
}

/// An attribute buffering interface for different pipeline modes
trait GeometryPipelineBackend {
    /// Checks if there is no incomplete data transfer
    fn is_empty(&self) -> bool;

    /// Checks if the pipeline needs a direct input from index buffer
    fn need_index_input(&self) -> bool;

    /// Submits an index from index buffer
    fn submit_index(&mut self, context: &mut BackendContext, val: u32);

    /**
     * Submits vertex attributes
     * @param input attributes of a vertex output from vertex shader
     * @return if the buffer is full and the geometry shader should be invoked
     */
    fn submit_vertex(&mut self, context: &mut BackendContext, input: &AttributeBuffer) -> bool;
}

// In the Point mode, vertex attributes are sent to the input registers in the geometry shader unit.
// The size of vertex shader outputs and geometry shader inputs are constants. Geometry shader is
// invoked upon inputs buffer filled up by vertex shader outputs. For example, if we have a geometry
// shader that takes 6 inputs, and the vertex shader outputs 2 attributes, it would take 3 vertices
// for one geometry shader invocation.
// TODO: what happens when the input size is not divisible by the output size?
struct GeometryPipelinePoint {
    attribute_buffer: AttributeBuffer,
    buffer_cur: usize,
    buffer_end: usize,
    vs_output_num: u32,
}

impl GeometryPipelinePoint {
    fn new(regs: &RegsInternal) -> Self {
        assert!(regs.pipeline.variable_primitive() == 0, "Assertion Failed!");
        assert!(regs.gs.input_to_uniform() == 0, "Assertion Failed!");
        let vs_output_num = regs.pipeline.vs_outmap_total_minus_1_a() + 1;
        let gs_input_num = regs.gs.max_input_attribute_index() as usize + 1;
        assert!(
            gs_input_num.is_multiple_of(vs_output_num as usize),
            "Assertion Failed!"
        );
        Self {
            attribute_buffer: AttributeBuffer::default(),
            buffer_cur: 0,
            buffer_end: gs_input_num,
            vs_output_num,
        }
    }
}

impl GeometryPipelineBackend for GeometryPipelinePoint {
    fn is_empty(&self) -> bool {
        self.buffer_cur == 0
    }

    fn need_index_input(&self) -> bool {
        false
    }

    fn submit_index(&mut self, _context: &mut BackendContext, _val: u32) {
        unreachable!();
    }

    fn submit_vertex(&mut self, context: &mut BackendContext, input: &AttributeBuffer) -> bool {
        let count = self.vs_output_num as usize;
        self.attribute_buffer[self.buffer_cur..self.buffer_cur + count]
            .copy_from_slice(&input[..count]);
        self.buffer_cur += count;
        if self.buffer_cur == self.buffer_end {
            self.buffer_cur = 0;
            context
                .unit
                .unit
                .load_input(&context.regs.gs, &self.attribute_buffer);
            return true;
        }
        false
    }
}

// In VariablePrimitive mode, vertex attributes are buffered into the uniform registers in the
// geometry shader unit. The number of vertex is variable, which is specified by the first index
// value in the batch. This mode is usually used for subdivision.
struct GeometryPipelineVariablePrimitive {
    need_index: bool,
    main_vertex_num: u32,
    total_vertex_num: u32,
    buffer_cur: usize,
    vs_output_num: u32,
}

impl GeometryPipelineVariablePrimitive {
    fn new(regs: &RegsInternal) -> Self {
        assert!(regs.pipeline.variable_primitive() == 1, "Assertion Failed!");
        assert!(regs.gs.input_to_uniform() == 1, "Assertion Failed!");
        Self {
            need_index: true,
            main_vertex_num: 0,
            total_vertex_num: 0,
            buffer_cur: 0,
            vs_output_num: regs.pipeline.vs_outmap_total_minus_1_a() + 1,
        }
    }
}

impl GeometryPipelineBackend for GeometryPipelineVariablePrimitive {
    fn is_empty(&self) -> bool {
        self.need_index
    }

    fn need_index_input(&self) -> bool {
        self.need_index
    }

    fn submit_index(&mut self, context: &mut BackendContext, val: u32) {
        debug_assert!(self.need_index);

        // The number of vertex input is put to the uniform register
        let vertex_num = F24::from_float32(val as f32);
        context.setup.uniforms.f[0] = Vec4 {
            x: vertex_num,
            y: vertex_num,
            z: vertex_num,
            w: vertex_num,
        };

        // The second uniform register and so on are used for receiving input vertices
        self.buffer_cur = 1;

        self.main_vertex_num = context
            .regs
            .pipeline
            .variable_vertex_main_num_minus_1
            .wrapping_add(1);
        self.total_vertex_num = val;
        self.need_index = false;
    }

    fn submit_vertex(&mut self, context: &mut BackendContext, input: &AttributeBuffer) -> bool {
        debug_assert!(!self.need_index);
        let uniforms = &mut context.setup.uniforms.f;
        if self.main_vertex_num != 0 {
            // For main vertices, receive all attributes
            let count = self.vs_output_num as usize;
            uniforms[self.buffer_cur..self.buffer_cur + count].copy_from_slice(&input[..count]);
            self.buffer_cur += count;
            self.main_vertex_num -= 1;
        } else {
            // For other vertices, only receive the first attribute (usually the position)
            uniforms[self.buffer_cur] = input[0];
            self.buffer_cur += 1;
        }
        self.total_vertex_num = self.total_vertex_num.wrapping_sub(1);

        if self.total_vertex_num == 0 {
            self.need_index = true;
            return true;
        }

        false
    }
}

// In FixedPrimitive mode, vertex attributes are buffered into the uniform registers in the geometry
// shader unit. The number of vertex per shader invocation is constant. This is usually used for
// particle system.
struct GeometryPipelineFixedPrimitive {
    buffer_begin: usize,
    buffer_cur: usize,
    buffer_end: usize,
    vs_output_num: u32,
}

impl GeometryPipelineFixedPrimitive {
    fn new(regs: &RegsInternal) -> Self {
        assert!(regs.pipeline.variable_primitive() == 0, "Assertion Failed!");
        assert!(regs.gs.input_to_uniform() == 1, "Assertion Failed!");
        let vs_output_num = regs.pipeline.vs_outmap_total_minus_1_a() + 1;
        assert!(
            vs_output_num == regs.pipeline.gs_config.stride_minus_1() + 1,
            "Assertion Failed!"
        );
        let vertex_num = regs.pipeline.gs_config.fixed_vertex_num_minus_1() as usize + 1;
        let buffer_begin = regs.pipeline.gs_config.start_index() as usize;
        Self {
            buffer_begin,
            buffer_cur: buffer_begin,
            buffer_end: buffer_begin + vs_output_num as usize * vertex_num,
            vs_output_num,
        }
    }
}

impl GeometryPipelineBackend for GeometryPipelineFixedPrimitive {
    fn is_empty(&self) -> bool {
        self.buffer_cur == self.buffer_begin
    }

    fn need_index_input(&self) -> bool {
        false
    }

    fn submit_index(&mut self, _context: &mut BackendContext, _val: u32) {
        unreachable!();
    }

    fn submit_vertex(&mut self, context: &mut BackendContext, input: &AttributeBuffer) -> bool {
        let count = self.vs_output_num as usize;
        context.setup.uniforms.f[self.buffer_cur..self.buffer_cur + count]
            .copy_from_slice(&input[..count]);
        self.buffer_cur += count;
        if self.buffer_cur == self.buffer_end {
            self.buffer_cur = self.buffer_begin;
            return true;
        }
        false
    }
}

/// A pipeline receiving from vertex shader and sending to geometry shader and primitive assembler
#[derive(Default)]
pub struct GeometryPipeline {
    vertex_handler: Option<VertexHandler>,
    backend: Option<Box<dyn GeometryPipelineBackend>>,
}

impl GeometryPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the handler for receiving vertex outputs from vertex shader
    pub fn set_vertex_handler(&mut self, vertex_handler: VertexHandler) {
        self.vertex_handler = Some(vertex_handler);
    }

    /// Setup the geometry shader unit if it is in use
    pub fn setup(
        &mut self,
        shader_engine: &mut dyn ShaderEngine,
        regs: &RegsInternal,
        gs: &mut ShaderSetup,
    ) {
        if self.backend.is_none() {
            return;
        }

        shader_engine.setup_batch(gs, regs.gs.main_offset());
    }

    /// Reconfigures the pipeline according to current register settings
    pub fn reconfigure(&mut self, regs: &RegsInternal, gs_unit: &mut GeometryShaderUnit) {
        assert!(
            self.backend
                .as_ref()
                .is_none_or(|backend| backend.is_empty()),
            "Assertion Failed!"
        );

        if regs.pipeline.use_gs() == UseGs::NO {
            self.backend = None;
            return;
        }

        // The following assumes that when geometry shader is in use, the shader unit 3 is configured as
        // a geometry shader unit.
        // TODO: what happens if this is not true?
        assert!(regs.gs.shader_mode() == ShaderMode::GS, "Assertion Failed!");
        assert!(regs.pipeline.use_gs() == UseGs::YES, "Assertion Failed!");

        gs_unit.config_output(&regs.gs);

        assert!(
            regs.pipeline.vs_outmap_total_minus_1_a() == regs.pipeline.vs_outmap_total_minus_1_b(),
            "Assertion Failed!"
        );

        self.backend = Some(match regs.pipeline.gs_config.mode() {
            GsMode::POINT => Box::new(GeometryPipelinePoint::new(regs)),
            GsMode::VARIABLE_PRIMITIVE => Box::new(GeometryPipelineVariablePrimitive::new(regs)),
            GsMode::FIXED_PRIMITIVE => Box::new(GeometryPipelineFixedPrimitive::new(regs)),
            _ => unreachable!(),
        });
    }

    /// Checks if the pipeline needs a direct input from index buffer
    pub fn need_index_input(&self) -> bool {
        match &self.backend {
            None => false,
            Some(backend) => backend.need_index_input(),
        }
    }

    /// Submits an index from index buffer. Call this only when NeedIndexInput returns true
    pub fn submit_index(
        &mut self,
        regs: &RegsInternal,
        gs_unit: &mut GeometryShaderUnit,
        gs: &mut ShaderSetup,
        val: u32,
    ) {
        let backend = self
            .backend
            .as_mut()
            .expect("the geometry pipeline has a backend");
        backend.submit_index(
            &mut BackendContext {
                regs,
                unit: gs_unit,
                setup: gs,
            },
            val,
        );
    }

    /// Submits vertex attributes output from vertex shader
    pub fn submit_vertex(
        &mut self,
        shader_engine: &dyn ShaderEngine,
        regs: &RegsInternal,
        gs_unit: &mut GeometryShaderUnit,
        gs: &mut ShaderSetup,
        input: &AttributeBuffer,
    ) {
        match &mut self.backend {
            None => {
                // No backend means the geometry shader is disabled, so we send the vertex shader output
                // directly to the primitive assembler.
                let vertex_handler = self
                    .vertex_handler
                    .as_mut()
                    .expect("the vertex handler is set before vertices are submitted");
                vertex_handler(input);
            }
            Some(backend) => {
                let mut context = BackendContext {
                    regs,
                    unit: &mut *gs_unit,
                    setup: &mut *gs,
                };
                if backend.submit_vertex(&mut context, input) {
                    shader_engine.run(gs, &mut gs_unit.unit);

                    // The uniform b15 is set to true after every geometry shader invocation. This is useful
                    // for the shader to know if this is the first invocation in a batch, if the program set
                    // b15 to false first.
                    gs.uniforms.b[15] = true;
                }
            }
        }
    }
}
