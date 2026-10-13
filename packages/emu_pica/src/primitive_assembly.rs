use emu_pica_types::OutputVertex;

use crate::regs::pipeline::TriangleTopology;

/// Utility class to build triangles from a series of vertices,
/// according to a given triangle topology.
#[derive(Clone, Copy, Debug)]
pub struct PrimitiveAssembler {
    topology: TriangleTopology,
    buffer_index: i32,
    buffer: [OutputVertex; 2],
    strip_ready: bool,
    winding: bool,
}

impl Default for PrimitiveAssembler {
    fn default() -> Self {
        Self::new(TriangleTopology::LIST)
    }
}

impl PrimitiveAssembler {
    pub fn new(topology: TriangleTopology) -> Self {
        Self {
            topology,
            buffer_index: 0,
            buffer: [OutputVertex::default(); 2],
            strip_ready: false,
            winding: false,
        }
    }

    /// Queues a vertex, builds primitives from the vertex queue according to the given
    /// triangle topology, and calls triangle_handler for each generated primitive.
    /// NOTE: We could specify the triangle handler in the constructor, but this way we can
    /// keep event and handler code next to each other.
    pub fn submit_vertex(
        &mut self,
        vtx: &OutputVertex,
        triangle_handler: &mut dyn FnMut(&OutputVertex, &OutputVertex, &OutputVertex),
    ) {
        match self.topology {
            TriangleTopology::LIST | TriangleTopology::SHADER => {
                if self.buffer_index < 2 {
                    self.buffer[self.buffer_index as usize] = *vtx;
                    self.buffer_index += 1;
                } else {
                    self.buffer_index = 0;
                    if self.topology == TriangleTopology::SHADER && self.winding {
                        triangle_handler(&self.buffer[1], &self.buffer[0], vtx);
                        self.winding = false;
                    } else {
                        triangle_handler(&self.buffer[0], &self.buffer[1], vtx);
                    }
                }
            }

            TriangleTopology::STRIP | TriangleTopology::FAN => {
                if self.strip_ready {
                    triangle_handler(&self.buffer[0], &self.buffer[1], vtx);
                }

                self.buffer[self.buffer_index as usize] = *vtx;
                self.strip_ready |= self.buffer_index == 1;

                if self.topology == TriangleTopology::STRIP {
                    self.buffer_index = i32::from(self.buffer_index == 0);
                } else if self.topology == TriangleTopology::FAN {
                    self.buffer_index = 1;
                }
            }

            _ => {
                log::error!(
                    target: "HW_GPU",
                    "Unknown triangle topology {:x}:",
                    self.topology.0
                );
            }
        }
    }

    /// Invert the vertex order of the next triangle. Called by geometry shader emitter.
    /// This only takes effect for TriangleTopology::Shader.
    pub fn set_winding(&mut self) {
        self.winding = true;
    }

    /// Resets the internal state of the PrimitiveAssembler.
    pub fn reset(&mut self) {
        self.buffer_index = 0;
        self.strip_ready = false;
        self.winding = false;
    }

    /// Reconfigures the PrimitiveAssembler to use a different triangle topology.
    pub fn reconfigure(&mut self, topology: TriangleTopology) {
        self.reset();
        self.topology = topology;
    }

    /// Returns whether the PrimitiveAssembler has an empty internal buffer.
    pub fn is_empty(&self) -> bool {
        self.buffer_index == 0 && !self.strip_ready
    }

    /// Returns the current topology.
    pub fn get_topology(&self) -> TriangleTopology {
        self.topology
    }
}
