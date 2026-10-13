mod base;

use std::sync::atomic::AtomicBool;

pub use base::{
    FrameHooks, RendererBase, RendererSettings, SCREEN_BOTTOM_HEIGHT, SCREEN_BOTTOM_WIDTH,
    SCREEN_TOP_HEIGHT, SCREEN_TOP_WIDTH, ScreenId, ScreenSurface, ScreenshotCallback,
};
pub use emu_opengles::{OpenGlesRenderer, RasterizerOpenGl};
use emu_pica_types::{
    DiskResourceLoadCallback, DisplayTransferConfig, MemoryFillConfig, OutputVertex,
};
pub use emu_vulkan::{RasterizerVulkan, VulkanRenderer};

pub trait Rasterizer {
    fn add_triangle(&mut self, v0: &OutputVertex, v1: &OutputVertex, v2: &OutputVertex);

    fn draw_triangles(&mut self);

    fn notify_pica_register_changed(&mut self, id: u32);

    fn flush_all(&mut self);

    fn flush_region(&mut self, addr: u32, size: u32);

    fn invalidate_region(&mut self, addr: u32, size: u32);

    fn flush_and_invalidate_region(&mut self, addr: u32, size: u32);

    fn clear_all(&mut self, flush: bool);

    fn accelerate_display_transfer(&mut self, config: &DisplayTransferConfig) -> bool;

    fn accelerate_texture_copy(&mut self, config: &DisplayTransferConfig) -> bool;

    fn accelerate_fill(&mut self, config: &MemoryFillConfig) -> bool;

    fn accelerate_draw_batch(&mut self, is_indexed: bool) -> bool;

    fn load_disk_resources(
        &mut self,
        stop_loading: &AtomicBool,
        callback: DiskResourceLoadCallback,
    );

    fn sync_entire_state(&mut self);

    fn set_accurate_mul(&mut self, accurate_mul: bool);
}

pub trait Renderer {
    fn rasterizer(&mut self) -> &mut dyn Rasterizer;

    fn swap_buffers(&mut self);

    fn try_present(&mut self, timeout_ms: i32, is_secondary: bool);

    fn prepare_video_dumping(&mut self);

    fn cleanup_video_dumping(&mut self);

    fn sync(&mut self);

    fn notify_surface_changed(&mut self, is_secondary: bool);
}

macro_rules! impl_rasterizer {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Rasterizer for $ty {
                fn add_triangle(&mut self, v0: &OutputVertex, v1: &OutputVertex, v2: &OutputVertex) {
                    <$ty>::add_triangle(self, v0, v1, v2)
                }

                fn draw_triangles(&mut self) {
                    <$ty>::draw_triangles(self)
                }

                fn notify_pica_register_changed(&mut self, id: u32) {
                    <$ty>::notify_pica_register_changed(self, id)
                }

                fn flush_all(&mut self) {
                    <$ty>::flush_all(self)
                }

                fn flush_region(&mut self, addr: u32, size: u32) {
                    <$ty>::flush_region(self, addr, size)
                }

                fn invalidate_region(&mut self, addr: u32, size: u32) {
                    <$ty>::invalidate_region(self, addr, size)
                }

                fn flush_and_invalidate_region(&mut self, addr: u32, size: u32) {
                    <$ty>::flush_and_invalidate_region(self, addr, size)
                }

                fn clear_all(&mut self, flush: bool) {
                    <$ty>::clear_all(self, flush)
                }

                fn accelerate_display_transfer(&mut self, config: &DisplayTransferConfig) -> bool {
                    <$ty>::accelerate_display_transfer(self, config)
                }

                fn accelerate_texture_copy(&mut self, config: &DisplayTransferConfig) -> bool {
                    <$ty>::accelerate_texture_copy(self, config)
                }

                fn accelerate_fill(&mut self, config: &MemoryFillConfig) -> bool {
                    <$ty>::accelerate_fill(self, config)
                }

                fn accelerate_draw_batch(&mut self, is_indexed: bool) -> bool {
                    <$ty>::accelerate_draw_batch(self, is_indexed)
                }

                fn load_disk_resources(
                    &mut self,
                    stop_loading: &AtomicBool,
                    callback: DiskResourceLoadCallback,
                ) {
                    <$ty>::load_disk_resources(self, stop_loading, callback)
                }

                fn sync_entire_state(&mut self) {
                    <$ty>::sync_entire_state(self)
                }

                fn set_accurate_mul(&mut self, accurate_mul: bool) {
                    <$ty>::set_accurate_mul(self, accurate_mul)
                }
            }
        )*
    };
}

macro_rules! impl_renderer {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Renderer for $ty {
                fn rasterizer(&mut self) -> &mut dyn Rasterizer {
                    <$ty>::rasterizer(self)
                }

                fn swap_buffers(&mut self) {
                    <$ty>::swap_buffers(self)
                }

                fn try_present(&mut self, timeout_ms: i32, is_secondary: bool) {
                    <$ty>::try_present(self, timeout_ms, is_secondary)
                }

                fn prepare_video_dumping(&mut self) {
                    <$ty>::prepare_video_dumping(self)
                }

                fn cleanup_video_dumping(&mut self) {
                    <$ty>::cleanup_video_dumping(self)
                }

                fn sync(&mut self) {
                    <$ty>::sync(self)
                }

                fn notify_surface_changed(&mut self, is_secondary: bool) {
                    <$ty>::notify_surface_changed(self, is_secondary)
                }
            }
        )*
    };
}

impl_rasterizer!(RasterizerVulkan, RasterizerOpenGl);
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
