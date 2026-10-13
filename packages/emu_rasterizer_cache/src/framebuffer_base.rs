use emu_pica_types::math_util::Rectangle;
use emu_pica_types::regs::RasterizerRegs;

use crate::pixel_format::SurfaceType;
use crate::slot_id::{FramebufferId, SurfaceId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewportInfo {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FramebufferParams {
    pub color_id: SurfaceId,
    pub depth_id: SurfaceId,
    pub color_level: u32,
    pub depth_level: u32,
    pub shadow_rendering: bool,
}

impl FramebufferParams {
    pub fn index(&self, ty: SurfaceType) -> u32 {
        match ty {
            SurfaceType::Color => 0,
            SurfaceType::Depth | SurfaceType::DepthStencil => 1,
            _ => {
                log::error!(target: "HW_GPU", "Unknown surface type in framebuffer");
                0
            }
        }
    }
}

/**
 * @brief FramebufferHelper is a RAII wrapper over backend specific framebuffer handle that
 * provides the viewport/scissor/draw rectanges and performs automatic rasterizer cache invalidation
 * when out of scope.
 *
 * The cache stays usable while the helper lives, so the invalidation happens when the helper is
 * given back to `RasterizerCache::release_framebuffer`.
 */
#[derive(Clone, Copy, Debug)]
pub struct FramebufferHelper {
    pub(crate) framebuffer_id: FramebufferId,
    pub(crate) params: FramebufferParams,
    pub(crate) res_scale: u32,
    scissor_rect: Rectangle<i32>,
    draw_rect: Rectangle<u32>,
    viewport: ViewportInfo,
}

impl FramebufferHelper {
    pub(crate) fn new(
        framebuffer_id: FramebufferId,
        params: FramebufferParams,
        res_scale: u32,
        flip_rect: bool,
        regs: &RasterizerRegs,
        surfaces_rect: Rectangle<u32>,
    ) -> Self {
        let height = surfaces_rect.get_height() / res_scale;
        let scale = res_scale as i32;

        // Determine the draw rectangle (render area + scissor)
        let mut viewport_rect = regs.get_viewport_rect();
        if flip_rect {
            viewport_rect = viewport_rect.vertical_mirror(height as i32);
        }

        let draw_rect = Rectangle::new(
            (surfaces_rect.left as i32 + viewport_rect.left * scale)
                .clamp(surfaces_rect.left as i32, surfaces_rect.right as i32) as u32,
            (surfaces_rect.bottom as i32 + viewport_rect.top * scale)
                .clamp(surfaces_rect.bottom as i32, surfaces_rect.top as i32) as u32,
            (surfaces_rect.left as i32 + viewport_rect.right * scale)
                .clamp(surfaces_rect.left as i32, surfaces_rect.right as i32) as u32,
            (surfaces_rect.bottom as i32 + viewport_rect.bottom * scale)
                .clamp(surfaces_rect.bottom as i32, surfaces_rect.top as i32) as u32,
        );

        // Update viewport
        let viewport = ViewportInfo {
            x: surfaces_rect.left as i32 + viewport_rect.left * scale,
            y: surfaces_rect.bottom as i32 + viewport_rect.bottom * scale,
            width: viewport_rect.get_width() * scale,
            height: viewport_rect.get_height() * scale,
        };

        // Scissor checks are window-, not viewport-relative, which means that if the cached texture
        // sub-rect changes, the scissor bounds also need to be updated.
        let mut scissor_rect = Rectangle::new(
            (surfaces_rect.left + regs.scissor_test.x1() * res_scale) as i32,
            // x2, y2 have +1 added to cover the entire pixel area, otherwise you might get cracks when
            // scaling or doing multisampling.
            (surfaces_rect.bottom + (regs.scissor_test.y2() + 1) * res_scale) as i32,
            (surfaces_rect.left + (regs.scissor_test.x2() + 1) * res_scale) as i32,
            (surfaces_rect.bottom + regs.scissor_test.y1() * res_scale) as i32,
        );

        if flip_rect {
            scissor_rect = scissor_rect.vertical_mirror(height as i32);
        }

        Self {
            framebuffer_id,
            params,
            res_scale,
            scissor_rect,
            draw_rect,
            viewport,
        }
    }

    pub fn framebuffer(&self) -> FramebufferId {
        self.framebuffer_id
    }

    pub fn draw_rect(&self) -> Rectangle<u32> {
        self.draw_rect
    }

    pub fn scissor(&self) -> Rectangle<i32> {
        self.scissor_rect
    }

    pub fn viewport(&self) -> ViewportInfo {
        self.viewport
    }
}
