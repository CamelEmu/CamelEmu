use emu_pica_types::math_util::Rectangle;
use emu_pica_types::regs::RegsInternal;

use crate::framebuffer_base::{FramebufferHelper, FramebufferParams};
use crate::pixel_format::{pixel_format_from_color_format, pixel_format_from_depth_format};
use crate::rasterizer_cache_base::{Framebuffer, RasterizerCache, Runtime, ScaleMatch};
use crate::slot_id::SurfaceId;
use crate::surface_base::SurfaceFlagBits;
use crate::surface_params::SurfaceParams;

impl<R: Runtime> RasterizerCache<R> {
    /// Get the color and depth surfaces based on the framebuffer configuration
    pub fn get_framebuffer_surfaces(
        &mut self,
        regs: &RegsInternal,
        using_color_fb: bool,
        using_depth_fb: bool,
    ) -> FramebufferHelper {
        let config = &regs.framebuffer.framebuffer;

        let framebuffer_width = config.get_width() as i32;
        let framebuffer_height = config.get_height() as i32;
        let viewport_rect = regs.rasterizer.get_viewport_rect();
        let viewport_clamped = Rectangle::new(
            viewport_rect.left.clamp(0, framebuffer_width) as u32,
            viewport_rect.top.clamp(0, framebuffer_height) as u32,
            viewport_rect.right.clamp(0, framebuffer_width) as u32,
            viewport_rect.bottom.clamp(0, framebuffer_height) as u32,
        );

        let mut color_params = SurfaceParams {
            is_tiled: true,
            res_scale: self.resolution_scale_factor,
            width: config.get_width(),
            height: config.get_height(),
            ..Default::default()
        };
        let mut depth_params = color_params;

        color_params.addr = config.get_color_buffer_physical_address();
        color_params.pixel_format = pixel_format_from_color_format(config.color_format());
        color_params.update_params();

        depth_params.addr = config.get_depth_buffer_physical_address();
        depth_params.pixel_format = pixel_format_from_depth_format(config.depth_format());
        depth_params.update_params();

        let color_vp_interval = color_params.get_sub_rect_interval(viewport_clamped, 0);
        let depth_vp_interval = depth_params.get_sub_rect_interval(viewport_clamped, 0);

        let mut color_rect = Rectangle::default();
        let mut color_id = SurfaceId::default();
        let mut color_level = 0;
        if using_color_fb {
            (color_id, color_rect) =
                self.get_surface_sub_rect(&color_params, ScaleMatch::Exact, false);
        }

        let mut depth_rect = Rectangle::default();
        let mut depth_id = SurfaceId::default();
        let mut depth_level = 0;
        if using_depth_fb {
            (depth_id, depth_rect) =
                self.get_surface_sub_rect(&depth_params, ScaleMatch::Exact, false);
        }

        let mut fb_rect = Rectangle::default();
        if color_id.is_valid() && depth_id.is_valid() {
            fb_rect = color_rect;
            // Color and Depth surfaces must have the same dimensions and offsets
            if color_rect.bottom != depth_rect.bottom
                || color_rect.top != depth_rect.top
                || color_rect.left != depth_rect.left
                || color_rect.right != depth_rect.right
            {
                color_id = self.get_surface_for_params(&color_params, ScaleMatch::Exact, false);
                depth_id = self.get_surface_for_params(&depth_params, ScaleMatch::Exact, false);
                fb_rect = self.slot_surfaces[color_id].get_scaled_rect(0);
            }
        } else if color_id.is_valid() {
            fb_rect = color_rect;
        } else if depth_id.is_valid() {
            fb_rect = depth_rect;
        }

        if color_id.is_valid() {
            let color_surface = &mut self.slot_surfaces[color_id];
            color_level = color_surface.level_of(color_params.addr);
            color_surface.flags.insert(SurfaceFlagBits::RENDER_TARGET);
            self.validate_surface(
                color_id,
                color_vp_interval.first(),
                color_vp_interval.length(),
            );
        }
        if depth_id.is_valid() {
            let depth_surface = &mut self.slot_surfaces[depth_id];
            depth_level = depth_surface.level_of(depth_params.addr);
            depth_surface.flags.insert(SurfaceFlagBits::RENDER_TARGET);
            self.validate_surface(
                depth_id,
                depth_vp_interval.first(),
                depth_vp_interval.length(),
            );
        }

        let fb_params = FramebufferParams {
            color_id,
            depth_id,
            color_level,
            depth_level,
            shadow_rendering: regs.framebuffer.is_shadow_rendering(),
        };

        let framebuffer_id = match self.framebuffers.get(&fb_params) {
            Some(&framebuffer_id) => framebuffer_id,
            None => {
                let color_surface = color_id.is_valid().then(|| &self.slot_surfaces[color_id]);
                let depth_surface = depth_id.is_valid().then(|| &self.slot_surfaces[depth_id]);
                let framebuffer =
                    self.runtime
                        .create_framebuffer(&fb_params, color_surface, depth_surface);
                let framebuffer_id = self.slot_framebuffers.insert(framebuffer);
                self.framebuffers.insert(fb_params, framebuffer_id);
                framebuffer_id
            }
        };

        FramebufferHelper::new(
            framebuffer_id,
            fb_params,
            self.slot_framebuffers[framebuffer_id].scale(),
            regs.framebuffer.framebuffer.is_flipped(),
            &regs.rasterizer,
            fb_rect,
        )
    }

    /// Returns the framebuffer of the helper.
    pub fn get_framebuffer(&mut self, helper: &FramebufferHelper) -> &mut R::Framebuffer {
        &mut self.slot_framebuffers[helper.framebuffer()]
    }

    /// Invalidates the regions the framebuffer drew, as the destructor of `FramebufferHelper`
    /// does.
    pub fn release_framebuffer(&mut self, helper: FramebufferHelper) {
        let draw_rect_unscaled = helper.draw_rect().div(helper.res_scale);
        let invalidate = |cache: &mut Self, surface_id: SurfaceId, level: u32| {
            let surface = &cache.slot_surfaces[surface_id];
            let interval = surface.get_sub_rect_interval(draw_rect_unscaled, level);
            let addr = interval.first();
            let size = interval.length();
            cache.invalidate_region(addr, size, surface_id);
        };
        if helper.params.color_id.is_valid() {
            invalidate(self, helper.params.color_id, helper.params.color_level);
        }
        if helper.params.depth_id.is_valid() {
            invalidate(self, helper.params.depth_id, helper.params.depth_level);
        }
    }
}
