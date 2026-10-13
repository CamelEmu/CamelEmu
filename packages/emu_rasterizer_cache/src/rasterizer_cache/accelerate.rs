use emu_pica_types::{DisplayTransferConfig, MemoryFillConfig, ScalingMode};

use crate::pixel_format::{
    SurfaceType, check_formats_blittable, pixel_format_from_gpu_pixel_format,
};
use crate::rasterizer_cache_base::{RasterizerCache, Runtime, ScaleMatch};
use crate::surface_params::SurfaceParams;
use crate::utils::{Extent, Offset, TextureBlit, TextureCopy};

impl<R: Runtime> RasterizerCache<R> {
    /// Perform hardware accelerated texture copy according to the provided configuration
    pub fn accelerate_texture_copy(&mut self, config: &DisplayTransferConfig) -> bool {
        self.runtime.push_debug_scope(
            [0.0, 0.0, 1.0, 1.0],
            format_args!(
                "RasterizerCache::AccelerateTextureCopy ({})",
                config.debug_name()
            ),
        );
        let result = self.accelerate_texture_copy_impl(config);
        self.runtime.pop_debug_scope();
        result
    }

    fn accelerate_texture_copy_impl(&mut self, config: &DisplayTransferConfig) -> bool {
        // Texture copy size is aligned to 16 byte units
        let copy_size = config.texture_copy.size & !15;
        if copy_size == 0 {
            return false;
        }

        let mut input_gap = config.texture_copy.input_gap() * 16;
        let mut input_width = config.texture_copy.input_width() * 16;
        if input_width == 0 && input_gap != 0 {
            return false;
        }
        if input_gap == 0 || input_width >= copy_size {
            input_width = copy_size;
            input_gap = 0;
        }
        if !copy_size.is_multiple_of(input_width) {
            return false;
        }

        let mut output_gap = config.texture_copy.output_gap() * 16;
        let mut output_width = config.texture_copy.output_width() * 16;
        if output_width == 0 && output_gap != 0 {
            return false;
        }
        if output_gap == 0 || output_width >= copy_size {
            output_width = copy_size;
            output_gap = 0;
        }
        if !copy_size.is_multiple_of(output_width) {
            return false;
        }

        let mut src_params = SurfaceParams {
            addr: config.physical_input_address(),
            stride: input_width + input_gap, // stride in bytes
            width: input_width,              // width in bytes
            height: copy_size / input_width,
            ..Default::default()
        };
        src_params.size = ((src_params.height - 1) * src_params.stride) + src_params.width;
        src_params.end = src_params.addr.wrapping_add(src_params.size);

        let (src_surface_id, src_rect) = self.get_tex_copy_surface(&src_params);
        if !src_surface_id.is_valid() {
            return false;
        }

        let src_info: SurfaceParams = self.slot_surfaces[src_surface_id].params;
        if output_gap != 0
            && (output_width
                != src_info.bytes_in_pixels(src_rect.get_width() / src_info.res_scale)
                    * if src_info.is_tiled { 8 } else { 1 }
                || !output_gap.is_multiple_of(src_info.bytes_in_pixels(if src_info.is_tiled {
                    64
                } else {
                    1
                })))
        {
            return false;
        }

        let mut dst_params = src_info;
        dst_params.addr = config.physical_output_address();
        dst_params.width = src_rect.get_width() / src_info.res_scale;
        dst_params.stride = dst_params.width
            + src_info.pixels_in_bytes(if src_info.is_tiled {
                output_gap / 8
            } else {
                output_gap
            });
        dst_params.height = src_rect.get_height() / src_info.res_scale;
        dst_params.res_scale = src_info.res_scale;
        dst_params.update_params();

        // Since we are going to invalidate the gap if there is one, we will have to load it first
        let load_gap = output_gap != 0;
        let (dst_surface_id, dst_rect) =
            self.get_surface_sub_rect(&dst_params, ScaleMatch::Upscale, load_gap);
        if !dst_surface_id.is_valid() {
            return false;
        }

        {
            let src_surface = &self.slot_surfaces[src_surface_id];
            let dst_surface = &self.slot_surfaces[dst_surface_id];

            if dst_surface.ty == SurfaceType::Texture
                || !check_formats_blittable(src_surface.pixel_format, dst_surface.pixel_format)
            {
                return false;
            }
        }

        assert!(
            src_rect.get_width() == dst_rect.get_width(),
            "Assertion Failed!"
        );

        let (src_surface, dst_surface) = self
            .slot_surfaces
            .get_pair_mut(src_surface_id, dst_surface_id);
        let texture_copy = TextureCopy {
            src_level: src_surface.level_of(src_params.addr),
            dst_level: dst_surface.level_of(dst_params.addr),
            src_offset: Offset {
                x: src_rect.left,
                y: src_rect.bottom,
            },
            dst_offset: Offset {
                x: dst_rect.left,
                y: dst_rect.bottom,
            },
            extent: Extent {
                width: src_rect.get_width(),
                height: src_rect.get_height(),
            },
            ..Default::default()
        };
        self.runtime
            .copy_textures(src_surface, dst_surface, &[texture_copy]);

        self.invalidate_region(dst_params.addr, dst_params.size, dst_surface_id);
        true
    }

    /// Perform hardware accelerated display transfer according to the provided configuration
    pub fn accelerate_display_transfer(&mut self, config: &DisplayTransferConfig) -> bool {
        self.runtime.push_debug_scope(
            [0.0, 0.0, 1.0, 1.0],
            format_args!(
                "RasterizerCache::AccelerateDisplayTransfer ({})",
                config.debug_name()
            ),
        );
        let result = self.accelerate_display_transfer_impl(config);
        self.runtime.pop_debug_scope();
        result
    }

    fn accelerate_display_transfer_impl(&mut self, config: &DisplayTransferConfig) -> bool {
        let mut src_params = SurfaceParams {
            addr: config.physical_input_address(),
            width: config.output_width(),
            stride: config.input_width(),
            height: config.output_height(),
            is_tiled: !config.input_linear(),
            pixel_format: pixel_format_from_gpu_pixel_format(config.input_format()),
            ..Default::default()
        };
        src_params.update_params();

        let mut dst_params = SurfaceParams {
            addr: config.physical_output_address(),
            width: if config.scaling() != ScalingMode::NO_SCALE {
                config.output_width() / 2
            } else {
                config.output_width()
            },
            height: if config.scaling() == ScalingMode::SCALE_XY {
                config.output_height() / 2
            } else {
                config.output_height()
            },
            is_tiled: config.input_linear() != config.dont_swizzle(),
            pixel_format: pixel_format_from_gpu_pixel_format(config.output_format()),
            ..Default::default()
        };
        dst_params.update_params();

        // Using flip_vertically alongside crop_input_lines produces skewed output on hardware.
        // We have to emulate this because some games rely on this behaviour to render correctly.
        if config.flip_vertically() && config.crop_input_lines() {
            dst_params.addr = dst_params.addr.wrapping_add(
                config
                    .input_width()
                    .wrapping_sub(config.output_width())
                    .wrapping_mul(config.output_height().wrapping_sub(1))
                    .wrapping_mul(config.output_format().bytes_per_pixel()),
            );
        }

        let (src_surface_id, mut src_rect) =
            self.get_surface_sub_rect(&src_params, ScaleMatch::Ignore, true);
        if !src_surface_id.is_valid() {
            return false;
        }

        dst_params.res_scale = self.slot_surfaces[src_surface_id].res_scale;

        let (dst_surface_id, dst_rect) =
            self.get_surface_sub_rect(&dst_params, ScaleMatch::Upscale, false);
        if !dst_surface_id.is_valid() {
            return false;
        }

        let (src_surface, dst_surface) = self
            .slot_surfaces
            .get_pair_mut(src_surface_id, dst_surface_id);

        if src_surface.is_tiled != dst_surface.is_tiled {
            std::mem::swap(&mut src_rect.top, &mut src_rect.bottom);
        }
        if config.flip_vertically() {
            std::mem::swap(&mut src_rect.top, &mut src_rect.bottom);
        }

        if !check_formats_blittable(src_surface.pixel_format, dst_surface.pixel_format) {
            return false;
        }

        let texture_blit = TextureBlit {
            src_level: src_surface.level_of(src_params.addr),
            dst_level: dst_surface.level_of(dst_params.addr),
            src_rect,
            dst_rect,
            ..Default::default()
        };
        self.runtime
            .blit_textures(src_surface, dst_surface, &texture_blit);

        self.invalidate_region(dst_params.addr, dst_params.size, dst_surface_id);
        true
    }

    /// Perform hardware accelerated memory fill according to the provided configuration
    pub fn accelerate_fill(&mut self, config: &MemoryFillConfig) -> bool {
        self.runtime.push_debug_scope(
            [1.0, 0.0, 1.0, 1.0],
            format_args!("RasterizerCache::AccelerateFill ({})", config.debug_name()),
        );

        let mut params = SurfaceParams {
            addr: config.start_address(),
            end: config.end_address(),
            ty: SurfaceType::Fill,
            res_scale: u32::from(u16::MAX),
            ..Default::default()
        };
        params.size = params.end.wrapping_sub(params.addr);

        let surface = self.runtime.create_surface(&params);
        let fill_surface_id = self.slot_surfaces.insert(surface);
        let fill_surface = &mut self.slot_surfaces[fill_surface_id];

        fill_surface.fill_data = config.value_32bit.to_ne_bytes();
        if config.fill_32bit() {
            fill_surface.fill_size = 4;
        } else if config.fill_24bit() {
            fill_surface.fill_size = 3;
        } else {
            fill_surface.fill_size = 2;
        }
        let (addr, size) = (fill_surface.addr, fill_surface.size);

        self.register_surface(fill_surface_id);
        self.invalidate_region(addr, size, fill_surface_id);
        self.runtime.pop_debug_scope();
        true
    }
}
