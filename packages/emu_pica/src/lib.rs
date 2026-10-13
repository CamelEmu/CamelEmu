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

    pub fn flush_region(&mut self, _paddr: u32, _size: u32) {}

    pub fn invalidate_region(&mut self, _paddr: u32, _size: u32) {}
}
