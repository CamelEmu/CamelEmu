use std::rc::Rc;

use emu_pica_types::AttributeBuffer;
use emu_renderer::Rasterizer;

use super::{PendingVertex, PicaCore};
use crate::output_vertex::output_vertex;
use crate::regs::pipeline::{IndexFormat, TriangleTopology, UseGs};
use crate::shader_unit::ShaderUnit;
use crate::vertex_loader::VertexLoader;

impl PicaCore {
    pub(super) fn draw_immediate(&mut self, rasterizer: &mut dyn Rasterizer) {
        let regs = Rc::clone(&self.regs);
        let regs = regs.borrow();

        // Compile the vertex shader.
        self.shader_engine
            .setup_batch(&mut self.vs_setup, regs.internal.vs.main_offset());

        let mut shader_unit = ShaderUnit::default();
        let mut output = AttributeBuffer::default();

        // Invoke the vertex shader for the vertex.
        shader_unit.load_input(&regs.internal.vs, &self.immediate.input_vertex);
        self.shader_engine.run(&self.vs_setup, &mut shader_unit);
        shader_unit.write_output(&regs.internal.vs, &mut output);

        // Reconfigure geometry pipeline if needed.
        if self.immediate.reset_geometry_pipeline {
            self.geometry_pipeline
                .reconfigure(&regs.internal, &mut self.gs_unit);
            self.immediate.reset_geometry_pipeline = false;
        }

        // Send to geometry pipeline.
        assert!(
            !self.geometry_pipeline.need_index_input(),
            "Assertion Failed!"
        );
        self.geometry_pipeline.setup(
            self.shader_engine.as_mut(),
            &regs.internal,
            &mut self.gs_setup,
        );
        self.submit_to_geometry_pipeline(rasterizer, &output);

        // Flush the immediate triangle.
        rasterizer.draw_triangles();
        self.immediate.current_attribute = 0;
    }

    pub(super) fn draw_arrays(&mut self, rasterizer: &mut dyn Rasterizer, is_indexed: bool) {
        let accelerate_draw = {
            // Geometry shaders cannot be accelerated due to register preservation.
            if self.regs.borrow().internal.pipeline.use_gs() == UseGs::YES {
                false
            } else {
                // TODO (wwylele): for Strip/Fan topology, if the primitive assember is not restarted
                // after this draw call, the buffered vertex from this draw should "leak" to the next
                // draw, in which case we should buffer the vertex into the software primitive assember,
                // or disable accelerate draw completely. However, there is not game found yet that does
                // this, so this is left unimplemented for now. Revisit this when an issue is found in
                // games.

                let mut accelerate_draw = self.use_hw_shader && self.primitive_assembler.is_empty();
                let topology = self.primitive_assembler.get_topology();
                if topology == TriangleTopology::SHADER || topology == TriangleTopology::LIST {
                    accelerate_draw = accelerate_draw
                        && self
                            .regs
                            .borrow()
                            .internal
                            .pipeline
                            .num_vertices
                            .is_multiple_of(3);
                }
                accelerate_draw
            }
        };

        // Attempt to use hardware vertex shaders if possible.
        if accelerate_draw && rasterizer.accelerate_draw_batch(is_indexed) {
            return;
        }

        // We cannot accelerate the draw, so load and execute the vertex shader for each vertex.
        self.load_vertices(rasterizer, is_indexed);

        // Draw emitted triangles.
        rasterizer.draw_triangles();
    }

    fn load_vertices(&mut self, rasterizer: &mut dyn Rasterizer, is_indexed: bool) {
        let regs = Rc::clone(&self.regs);
        let regs = regs.borrow();

        // Read and validate vertex information from the loaders
        let pipeline = regs.internal.pipeline;
        let base_address = pipeline.vertex_attributes.get_physical_base_address();
        let memory = self.memory.clone();
        let loader = VertexLoader::new(memory.as_ref(), &pipeline);
        regs.internal.rasterizer.validate_semantics();

        // Locate index buffer.
        let index_info = &pipeline.index_array;
        let index_address_8: *const u8 =
            memory.get_physical_pointer(base_address.wrapping_add(index_info.offset()));
        let index_address_16: *const u16 = index_address_8.cast();
        let index_u16 = index_info.format() != IndexFormat::BYTE;

        // Simple circular-replacement vertex cache
        const VERTEX_CACHE_SIZE: usize = 64;
        let mut vertex_cache_valid = [false; VERTEX_CACHE_SIZE];
        let mut vertex_cache_ids = [0u16; VERTEX_CACHE_SIZE];
        let mut vertex_cache = vec![AttributeBuffer::default(); VERTEX_CACHE_SIZE];
        let mut vertex_cache_pos = 0usize;

        // Compile the vertex shader for this batch.
        let mut shader_unit = ShaderUnit::default();
        let mut vs_output = AttributeBuffer::default();
        self.shader_engine
            .setup_batch(&mut self.vs_setup, regs.internal.vs.main_offset());

        // Setup geometry pipeline in case we are using a geometry shader.
        self.geometry_pipeline
            .reconfigure(&regs.internal, &mut self.gs_unit);
        self.geometry_pipeline.setup(
            self.shader_engine.as_mut(),
            &regs.internal,
            &mut self.gs_setup,
        );
        assert!(
            !self.geometry_pipeline.need_index_input() || is_indexed,
            "Assertion Failed!"
        );

        for index in 0..pipeline.num_vertices {
            // Indexed rendering doesn't use the start offset
            let vertex = if is_indexed {
                if index_u16 {
                    u32::from(unsafe { index_address_16.add(index as usize).read_unaligned() })
                } else {
                    u32::from(unsafe { index_address_8.add(index as usize).read() })
                }
            } else {
                index.wrapping_add(pipeline.vertex_offset)
            };

            let mut vertex_cache_hit = false;
            if is_indexed {
                if self.geometry_pipeline.need_index_input() {
                    self.geometry_pipeline.submit_index(
                        &regs.internal,
                        &mut self.gs_unit,
                        &mut self.gs_setup,
                        vertex,
                    );
                    continue;
                }

                for i in 0..VERTEX_CACHE_SIZE {
                    if vertex_cache_valid[i] && vertex == u32::from(vertex_cache_ids[i]) {
                        vs_output = vertex_cache[i];
                        vertex_cache_hit = true;
                        break;
                    }
                }
            }

            if !vertex_cache_hit {
                // Initialize data for the current vertex
                let mut input = AttributeBuffer::default();
                loader.load_vertex(
                    base_address,
                    index,
                    vertex,
                    &mut input,
                    &self.input_default_attributes,
                );

                // Invoke the vertex shader for this vertex.
                shader_unit.load_input(&regs.internal.vs, &input);
                self.shader_engine.run(&self.vs_setup, &mut shader_unit);
                shader_unit.write_output(&regs.internal.vs, &mut vs_output);

                // Cache the vertex when doing indexed rendering.
                if is_indexed {
                    vertex_cache[vertex_cache_pos] = vs_output;
                    vertex_cache_valid[vertex_cache_pos] = true;
                    vertex_cache_ids[vertex_cache_pos] = vertex as u16;
                    vertex_cache_pos = (vertex_cache_pos + 1) % VERTEX_CACHE_SIZE;
                }
            }

            // Send to geometry pipeline
            self.submit_to_geometry_pipeline(rasterizer, &vs_output);
        }
    }

    /// Sends the vertex to the geometry pipeline, then passes the vertices that the handlers
    /// received to the primitive assembler.
    fn submit_to_geometry_pipeline(
        &mut self,
        rasterizer: &mut dyn Rasterizer,
        input: &AttributeBuffer,
    ) {
        let regs = Rc::clone(&self.regs);
        let regs = regs.borrow();

        self.geometry_pipeline.submit_vertex(
            self.shader_engine.as_ref(),
            &regs.internal,
            &mut self.gs_unit,
            &mut self.gs_setup,
            input,
        );

        let pending_vertices = std::mem::take(&mut *self.pending_vertices.borrow_mut());
        for pending_vertex in pending_vertices {
            match pending_vertex {
                PendingVertex::Vertex(buffer) => {
                    let mut add_triangle = |v0: &_, v1: &_, v2: &_| {
                        rasterizer.add_triangle(v0, v1, v2);
                    };
                    let vertex = output_vertex(&regs.internal.rasterizer, &buffer);
                    self.primitive_assembler
                        .submit_vertex(&vertex, &mut add_triangle);
                }
                PendingVertex::Winding => self.primitive_assembler.set_winding(),
            }
        }
    }
}
