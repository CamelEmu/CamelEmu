pub use emu_opengles::OpenGlesRenderer;
pub use emu_vulkan::VulkanRenderer;

pub trait Renderer {
    fn swap_buffers(&mut self);

    fn try_present(&mut self, timeout_ms: i32, is_secondary: bool);

    fn notify_surface_changed(&mut self, is_secondary: bool);
}

macro_rules! impl_renderer {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Renderer for $ty {
                fn swap_buffers(&mut self) {
                    <$ty>::swap_buffers(self)
                }

                fn try_present(&mut self, timeout_ms: i32, is_secondary: bool) {
                    <$ty>::try_present(self, timeout_ms, is_secondary)
                }

                fn notify_surface_changed(&mut self, is_secondary: bool) {
                    <$ty>::notify_surface_changed(self, is_secondary)
                }
            }
        )*
    };
}

impl_renderer!(VulkanRenderer, OpenGlesRenderer);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum GraphicsApi {
    Software = 0,
    OpenGl = 1,
    Vulkan = 2,
}

pub fn create(api: GraphicsApi) -> Option<Box<dyn Renderer>> {
    match api {
        GraphicsApi::Software => None,
        GraphicsApi::OpenGl => Some(Box::new(OpenGlesRenderer::new())),
        GraphicsApi::Vulkan => Some(Box::new(VulkanRenderer::new())),
    }
}
