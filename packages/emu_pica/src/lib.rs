mod aligned;
mod bits;
pub mod memory;
pub mod nihstro;
pub mod output_vertex;
pub mod packed_attribute;
pub mod primitive_assembly;
pub mod regs;
pub mod shader;
pub mod shader_setup;
pub mod shader_unit;
mod vector;
pub mod vertex_loader;

use emu_renderer::Renderer;

pub struct Gpu {
    renderer: Box<dyn Renderer>,
}

impl Gpu {
    pub fn new(renderer: Box<dyn Renderer>) -> Self {
        Self { renderer }
    }

    pub fn renderer(&mut self) -> &mut dyn Renderer {
        self.renderer.as_mut()
    }

    pub fn read_reg(&self, _vaddr: u32) -> u32 {
        0
    }

    pub fn write_reg(&mut self, _vaddr: u32, _data: u32) {}

    pub fn flush_region(&mut self, paddr: u32, size: u32) {
        self.renderer.rasterizer().flush_region(paddr, size);
    }

    pub fn invalidate_region(&mut self, paddr: u32, size: u32) {
        self.renderer.rasterizer().invalidate_region(paddr, size);
    }
}
