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
}
