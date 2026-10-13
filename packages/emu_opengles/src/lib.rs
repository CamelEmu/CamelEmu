#[derive(Default)]
pub struct OpenGlesRenderer {}

impl OpenGlesRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn swap_buffers(&mut self) {}

    pub fn try_present(&mut self, _timeout_ms: i32, _is_secondary: bool) {}

    pub fn notify_surface_changed(&mut self, _is_secondary: bool) {}
}
