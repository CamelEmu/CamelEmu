use std::sync::atomic::{AtomicBool, Ordering};

use crate::GraphicsApi;

pub const SCREEN_TOP_WIDTH: u32 = 400;
pub const SCREEN_TOP_HEIGHT: u32 = 240;
pub const SCREEN_BOTTOM_WIDTH: u32 = 320;
pub const SCREEN_BOTTOM_HEIGHT: u32 = 240;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenId {
    TopLeft,
    TopRight,
    Bottom,
}

/// The size of the surface that shows one screen. Every screen has its own surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScreenSurface {
    pub width: u32,
    pub height: u32,
}

impl ScreenSurface {
    /// The ratio of the size of the top screen on its surface to its native size.
    pub fn scaling_ratio(&self) -> u32 {
        (self.width.wrapping_sub(1) / SCREEN_TOP_WIDTH).wrapping_add(1)
    }
}

pub type ScreenshotCallback = Box<dyn FnOnce(bool) + Send>;

#[derive(Default)]
pub struct RendererSettings {
    pub screenshot_requested: AtomicBool,
    pub screenshot_bits: Option<*mut u8>,
    pub screenshot_complete_callback: Option<ScreenshotCallback>,
    pub screenshot_screen: Option<ScreenId>,
    pub bg_color_update_requested: AtomicBool,
    pub shader_update_requested: AtomicBool,
}

/// What the end of a frame asks of the rest of the emulator.
pub trait FrameHooks {
    fn end_system_frame(&mut self);

    fn do_frame_limiting(&mut self);

    fn begin_system_frame(&mut self);
}

pub struct RendererBase {
    settings: RendererSettings,
    graphics_api: GraphicsApi,
    resolution_factor: u32,
    top_surface: ScreenSurface,
    bottom_surface: ScreenSurface,
    current_fps: f32,
    current_frame: i32,
}

impl RendererBase {
    pub fn new(graphics_api: GraphicsApi, resolution_factor: u32) -> Self {
        Self {
            settings: RendererSettings::default(),
            graphics_api,
            resolution_factor,
            top_surface: ScreenSurface::default(),
            bottom_surface: ScreenSurface::default(),
            current_fps: 0.0,
            current_frame: 0,
        }
    }

    pub fn resolution_scale_factor(&self) -> u32 {
        if self.graphics_api == GraphicsApi::Software {
            // Software renderer always render at native resolution
            return 1;
        }
        if self.resolution_factor != 0 {
            self.resolution_factor
        } else {
            self.top_surface.scaling_ratio()
        }
    }

    pub fn set_surface(&mut self, is_secondary: bool, surface: ScreenSurface) {
        if is_secondary {
            self.bottom_surface = surface;
        } else {
            self.top_surface = surface;
        }
    }

    pub fn surface(&self, is_secondary: bool) -> ScreenSurface {
        if is_secondary {
            self.bottom_surface
        } else {
            self.top_surface
        }
    }

    pub fn end_frame(&mut self, hooks: &mut dyn FrameHooks) {
        self.current_frame += 1;
        hooks.end_system_frame();
        hooks.do_frame_limiting();
        hooks.begin_system_frame();
    }

    pub fn current_fps(&self) -> f32 {
        self.current_fps
    }

    pub fn set_current_fps(&mut self, fps: f32) {
        self.current_fps = fps;
    }

    pub fn current_frame(&self) -> i32 {
        self.current_frame
    }

    pub fn settings(&mut self) -> &mut RendererSettings {
        &mut self.settings
    }

    pub fn is_screenshot_pending(&self) -> bool {
        self.settings.screenshot_requested.load(Ordering::Acquire)
    }

    pub fn request_screenshot(
        &mut self,
        data: *mut u8,
        callback: ScreenshotCallback,
        screen: ScreenId,
    ) {
        if self.settings.screenshot_requested.load(Ordering::Acquire) {
            log::error!(
                target: "Render",
                "A screenshot is already requested or in progress, ignoring the request"
            );
            return;
        }
        self.settings.screenshot_bits = Some(data);
        self.settings.screenshot_complete_callback = Some(callback);
        self.settings.screenshot_screen = Some(screen);
        self.settings
            .screenshot_requested
            .store(true, Ordering::Release);
    }
}
