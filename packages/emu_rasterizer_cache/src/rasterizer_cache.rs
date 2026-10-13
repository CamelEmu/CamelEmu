use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use emu_pica_types::hash::compute_hash64;
use emu_pica_types::math_util::Rectangle;
use emu_pica_types::regs::RegsInternal;
use emu_pica_types::regs::texturing::{FullTextureConfig, TextureConfig, TextureFilter, WrapMode};
use emu_pica_types::{DisplayTransferConfig, MemoryFillConfig, ScalingMode};

use crate::custom_textures::custom_tex_manager::{CustomTexManager, MAX_UPLOADS_PER_TICK};
use crate::custom_textures::material::Material;
use crate::framebuffer_base::{FramebufferHelper, FramebufferParams};
use crate::interval::{Interval, IntervalMap};
use crate::pixel_format::{
    PixelFormat, SurfaceType, TextureType, check_formats_blittable, get_format_bytes_per_pixel,
    pixel_format_from_color_format, pixel_format_from_depth_format,
    pixel_format_from_gpu_pixel_format, pixel_format_from_texture_format,
};
use crate::rasterizer_cache_base::{
    AsyncUpload, CacheMemory, CacheSettings, Framebuffer, MatchFlags, RasterizerCache, Runtime,
    ScaleMatch, Surface, TextureSampling,
};
use crate::sampler_params::SamplerParams;
use crate::slot_id::{NULL_SURFACE_CUBE_ID, NULL_SURFACE_ID, SamplerId, SurfaceId};
use crate::slot_vector::SlotVector;
use crate::surface_base::{SurfaceBase, SurfaceFlagBits, SurfaceRegions};
use crate::surface_params::{SurfaceInterval, SurfaceParams};
use crate::texture::texture_decode::TextureInfo;
use crate::texture_cube::TextureCubeConfig;
use crate::utils::{
    BufferTextureCopy, ClearValue, Extent, Offset, TextureBlit, TextureClear, TextureCopy,
    decode_texture, encode_texture, mip_levels,
};

/// Address shift for caching surfaces into a hash table
const CITRA_PAGEBITS: u64 = 18;

const CITRA_PAGE_BITS: u32 = 12;

pub type SurfaceRectTuple = (SurfaceId, Rectangle<u32>);

const fn align_up(value: u32, size: u32) -> u32 {
    let remainder = value % size;
    if remainder == 0 {
        value
    } else {
        value - remainder + size
    }
}

/// The physical memory from the address, `size` bytes long, as `MemoryRef::GetWriteBytes` returns
/// it, or `None` when nothing backs the address.
fn get_write_bytes<'a>(memory: &dyn CacheMemory, address: u32, size: u32) -> Option<&'a mut [u8]> {
    let backing = memory.get_physical_ref(address)?;
    let backing = unsafe { &mut *backing };
    let size = (size as usize).min(backing.len());
    Some(&mut backing[..size])
}

impl<R: Runtime> RasterizerCache<R> {
    pub fn new(
        memory: Rc<dyn CacheMemory>,
        custom_tex_manager: Rc<RefCell<dyn CustomTexManager>>,
        runtime: R,
        resolution_scale_factor: u32,
        settings: CacheSettings,
    ) -> Self {
        let mut cache = Self {
            memory,
            custom_tex_manager,
            runtime,
            settings,
            texture_cube_cache: HashMap::new(),
            page_table: HashMap::new(),
            framebuffers: HashMap::new(),
            samplers: HashMap::new(),
            sentenced: Vec::new(),
            slot_surfaces: SlotVector::new(),
            slot_samplers: SlotVector::new(),
            slot_framebuffers: SlotVector::new(),
            dirty_regions: IntervalMap::new(),
            cached_pages: IntervalMap::new(),
            resolution_scale_factor,
            frame_tick: 0,
            filter: settings.texture_filter,
            dump_textures: settings.dump_textures,
            use_custom_textures: settings.custom_textures,
            async_uploads: Vec::new(),
        };

        // Create null handles for all cached resources
        let null_surface = cache.runtime.create_surface(&SurfaceParams {
            width: 1,
            height: 1,
            stride: 1,
            texture_type: TextureType::Texture2D,
            pixel_format: PixelFormat::RGBA8,
            ty: SurfaceType::Color,
            ..Default::default()
        });
        let _ = cache.slot_surfaces.insert(null_surface);
        let null_cube_surface = cache.runtime.create_surface(&SurfaceParams {
            width: 1,
            height: 1,
            stride: 1,
            texture_type: TextureType::CubeMap,
            pixel_format: PixelFormat::RGBA8,
            ty: SurfaceType::Color,
            ..Default::default()
        });
        let _ = cache.slot_surfaces.insert(null_cube_surface);
        let null_sampler = cache.runtime.create_sampler(&SamplerParams {
            mag_filter: TextureFilter::LINEAR,
            min_filter: TextureFilter::LINEAR,
            mip_filter: TextureFilter::LINEAR,
            wrap_s: WrapMode::CLAMP_TO_BORDER,
            wrap_t: WrapMode::CLAMP_TO_BORDER,
            ..Default::default()
        });
        let _ = cache.slot_samplers.insert(null_sampler);

        let null_surface = &mut cache.slot_surfaces[NULL_SURFACE_ID];
        let clear = TextureClear {
            texture_level: 0,
            texture_rect: null_surface.get_scaled_rect(0),
            value: ClearValue {
                color: [0.0, 0.0, 0.0, 0.0],
                ..Default::default()
            },
        };
        cache.runtime.clear_texture(null_surface, &clear);
        cache
    }

    /// Notify the cache that a new frame has been queued
    pub fn tick_frame(&mut self, scale_factor: u32) {
        self.process_async_uploads();
        self.run_garbage_collector();

        let new_filter = self.settings.texture_filter;
        if self.filter != new_filter {
            self.filter = new_filter;
            self.unregister_all();
        }

        let resolution_scale_changed = self.resolution_scale_factor != scale_factor;
        let use_custom_texture_changed = self.settings.custom_textures != self.use_custom_textures;

        if resolution_scale_changed || use_custom_texture_changed {
            self.resolution_scale_factor = scale_factor;
            self.use_custom_textures = self.settings.custom_textures;
            if self.use_custom_textures {
                self.custom_tex_manager.borrow_mut().find_custom_textures();
            }
            self.unregister_all();
        }
    }

    /// Processes queued texture uploads, as `CustomTexManager::TickFrame` does.
    fn process_async_uploads(&mut self) {
        if !self.custom_tex_manager.borrow().textures_loaded() {
            return;
        }
        let mut num_uploads = 0;
        let mut index = 0;
        while index < self.async_uploads.len() {
            if num_uploads >= MAX_UPLOADS_PER_TICK {
                return;
            }
            let upload = &self.async_uploads[index];
            if upload.material.is_decoded() {
                let AsyncUpload {
                    material,
                    surface_id,
                    level,
                } = self.async_uploads.remove(index);
                if self.slot_surfaces.get(surface_id).is_some() {
                    self.upload_custom(surface_id, &material, level);
                }
                num_uploads += 1;
            } else if upload.material.is_failed() {
                self.async_uploads.remove(index);
            } else {
                index += 1;
            }
        }
    }

    /// Unregisters sentenced surfaces that have surpassed the destruction threshold.
    fn run_garbage_collector(&mut self) {
        self.frame_tick += 1;
        let mut index = 0;
        while index < self.sentenced.len() {
            let (surface_id, tick) = self.sentenced[index];
            if self.frame_tick - tick <= self.runtime.remove_threshold() {
                index += 1;
                continue;
            }
            self.remove_framebuffers(surface_id);
            self.slot_surfaces.erase(surface_id);
            self.sentenced.remove(index);
        }
    }

    /// Removes any framebuffers that reference the provided surface_id.
    fn remove_framebuffers(&mut self, surface_id: SurfaceId) {
        let slot_framebuffers = &mut self.slot_framebuffers;
        self.framebuffers.retain(|params, &mut framebuffer_id| {
            if params.color_id == surface_id || params.depth_id == surface_id {
                slot_framebuffers.erase(framebuffer_id);
                false
            } else {
                true
            }
        });
    }

    /// Removes any references of the provided surface id from cached texture cubes.
    fn remove_texture_cube_face(&mut self, surface_id: SurfaceId) {
        if !self.slot_surfaces[surface_id]
            .flags
            .contains(SurfaceFlagBits::TRACKED)
        {
            return;
        }

        let sentenced = &mut self.sentenced;
        let frame_tick = self.frame_tick;
        self.texture_cube_cache.retain(|_, cube| {
            for face_id in cube.face_ids.iter_mut() {
                if *face_id == surface_id {
                    *face_id = SurfaceId::default();
                }
            }
            if cube.face_ids.iter().all(|id| !id.is_valid()) {
                sentenced.push((cube.surface_id, frame_tick));
                false
            } else {
                true
            }
        });
    }

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

    /// Returns a reference to the surface object assigned to surface_id
    pub fn get_surface(&mut self, surface_id: SurfaceId) -> &mut R::Surface {
        &mut self.slot_surfaces[surface_id]
    }

    pub fn get_sampler(&mut self, sampler_id: SamplerId) -> &mut R::Sampler {
        &mut self.slot_samplers[sampler_id]
    }

    /// Returns a reference to the sampler object matching the provided configuration
    pub fn get_sampler_for_config(&mut self, config: &TextureConfig) -> &mut R::Sampler {
        let texture_sampling = self.settings.texture_sampling;
        let get_filter = |filter: TextureFilter| match texture_sampling {
            TextureSampling::GameControlled => filter,
            TextureSampling::NearestNeighbor => TextureFilter::NEAREST,
            TextureSampling::Linear => TextureFilter::LINEAR,
        };

        let params = SamplerParams {
            mag_filter: get_filter(config.mag_filter()),
            min_filter: get_filter(config.min_filter()),
            mip_filter: get_filter(config.mip_filter()),
            wrap_s: config.wrap_s(),
            wrap_t: config.wrap_t(),
            border_color: config.border_color,
            lod_min: config.lod_min_level(),
            lod_max: config.lod_max_level(),
            lod_bias: config.lod_bias(),
        };

        let sampler_id = match self.samplers.get(&params) {
            Some(&sampler_id) => sampler_id,
            None => {
                let sampler = self.runtime.create_sampler(&params);
                let sampler_id = self.slot_samplers.insert(sampler);
                self.samplers.insert(params, sampler_id);
                sampler_id
            }
        };

        &mut self.slot_samplers[sampler_id]
    }

    /// Copy one surface's region to another
    pub fn copy_surface(
        &mut self,
        src_surface_id: SurfaceId,
        dst_surface_id: SurfaceId,
        copy_interval: SurfaceInterval,
    ) {
        let copy_addr = copy_interval.lower;
        let (src_surface, dst_surface) = self
            .slot_surfaces
            .get_pair_mut(src_surface_id, dst_surface_id);
        let subrect_params = dst_surface.from_interval(copy_interval);
        assert!(
            subrect_params.get_interval() == copy_interval,
            "Assertion Failed!"
        );

        if src_surface.ty == SurfaceType::Fill {
            let clear = TextureClear {
                texture_level: dst_surface.level_of(copy_addr),
                texture_rect: dst_surface.get_scaled_sub_rect(&subrect_params),
                value: src_surface.make_clear_value(copy_addr, dst_surface.pixel_format),
            };
            self.runtime.clear_texture(dst_surface, &clear);
            return;
        }

        let src_scale = src_surface.res_scale;
        let dst_scale = dst_surface.res_scale;
        if src_scale > dst_scale {
            dst_surface.scale_up(&mut self.runtime, src_scale);
        }

        let src_rect = src_surface.get_scaled_sub_rect(&subrect_params);
        let dst_rect = dst_surface.get_scaled_sub_rect(&subrect_params);
        if src_scale == dst_scale {
            let copy = TextureCopy {
                src_level: src_surface.level_of(copy_addr),
                dst_level: dst_surface.level_of(copy_addr),
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
                .copy_textures(src_surface, dst_surface, &[copy]);
        } else {
            let blit = TextureBlit {
                src_level: src_surface.level_of(copy_addr),
                dst_level: dst_surface.level_of(copy_addr),
                src_rect,
                dst_rect,
                ..Default::default()
            };
            self.runtime.blit_textures(src_surface, dst_surface, &blit);
        }
    }

    /// Load a texture from 3DS memory to OpenGL and cache it (if not already cached)
    pub fn get_surface_for_params(
        &mut self,
        params: &SurfaceParams,
        match_res_scale: ScaleMatch,
        load_if_create: bool,
    ) -> SurfaceId {
        if params.addr == 0 || params.height * params.width == 0 {
            return SurfaceId::default();
        }
        // Use GetSurfaceSubRect instead
        assert!(params.width == params.stride, "Assertion Failed!");
        assert!(
            !params.is_tiled || (params.width.is_multiple_of(8) && params.height.is_multiple_of(8)),
            "Assertion Failed!"
        );

        // Check for an exact match in existing surfaces
        let mut surface_id = self.find_match(MatchFlags::EXACT, params, match_res_scale, None);

        if !surface_id.is_valid() {
            surface_id = self.create_surface(params);
            self.register_surface(surface_id);
        }

        if load_if_create {
            self.validate_surface(surface_id, params.addr, params.size);
        }

        surface_id
    }

    /// Attempt to find a subrect (resolution scaled) of a surface, otherwise loads a texture from
    /// 3DS memory to OpenGL and caches it (if not already cached)
    pub fn get_surface_sub_rect(
        &mut self,
        params: &SurfaceParams,
        match_res_scale: ScaleMatch,
        load_if_create: bool,
    ) -> SurfaceRectTuple {
        if params.addr == 0 || params.height * params.width == 0 {
            return (SurfaceId::default(), Rectangle::default());
        }

        // Attempt to find encompassing surface
        let mut surface_id = self.find_match(MatchFlags::SUB_RECT, params, match_res_scale, None);

        // Check if FindMatch failed because of res scaling. If that's the case create a new surface
        // with the dimensions of the lower res_scale surface to suggest it should not be used again.
        if !surface_id.is_valid() && match_res_scale != ScaleMatch::Ignore {
            surface_id = self.find_match(MatchFlags::SUB_RECT, params, ScaleMatch::Ignore, None);
            if surface_id.is_valid() {
                let mut new_params = self.slot_surfaces[surface_id].params;
                new_params.res_scale = params.res_scale;

                surface_id = self.create_surface(&new_params);
                self.register_surface(surface_id);
            }
        }

        let mut aligned_params = *params;
        if params.is_tiled {
            aligned_params.height = align_up(params.height, 8);
            aligned_params.width = align_up(params.width, 8);
            aligned_params.stride = align_up(params.stride, 8);
            aligned_params.update_params();
        }

        // No subrect found - create and return a new surface
        if !surface_id.is_valid() {
            let mut new_params = aligned_params;
            // Can't have gaps in a surface
            new_params.width = aligned_params.stride;
            new_params.update_params();
            // GetSurface will create the new surface and possibly adjust res_scale if necessary
            surface_id = self.get_surface_for_params(&new_params, match_res_scale, load_if_create);
        } else if load_if_create {
            self.validate_surface(surface_id, aligned_params.addr, aligned_params.size);
        }

        (
            surface_id,
            self.slot_surfaces[surface_id].get_scaled_sub_rect(params),
        )
    }

    /// Get a surface based on the texture configuration
    pub fn get_texture_surface(&mut self, config: &FullTextureConfig) -> &mut R::Surface {
        let info = TextureInfo::from_pica_register(&config.config, config.format);
        let max_level = mip_levels(info.width, info.height, config.config.lod_max_level()) - 1;
        let surface_id = self.get_texture_surface_for_info(&info, max_level);
        &mut self.slot_surfaces[surface_id]
    }

    pub fn get_texture_surface_for_info(
        &mut self,
        info: &TextureInfo,
        max_level: u32,
    ) -> SurfaceId {
        if info.physical_address == 0 {
            // Can occur when texture addr is null or its memory is unmapped/invalid
            // HACK: In this case, the correct behaviour for the PICA is to use the last
            // rendered colour. But because this would be impractical to implement, the
            // next best alternative is to use a clear texture, essentially skipping
            // the geometry in question.
            // For example: a bug in Pokemon X/Y causes NULL-texture squares to be drawn
            // on the male character's face, which in the OpenGL default appear black.
            return NULL_SURFACE_ID;
        }

        let mut params = SurfaceParams {
            addr: info.physical_address,
            width: info.width,
            height: info.height,
            levels: max_level + 1,
            is_tiled: true,
            pixel_format: pixel_format_from_texture_format(info.format),
            res_scale: if self.filter != crate::rasterizer_cache_base::TextureFilter::NoFilter {
                self.resolution_scale_factor
            } else {
                1
            },
            ..Default::default()
        };
        params.update_params();

        let min_width = info.width >> max_level;
        let min_height = info.height >> max_level;
        if !min_width.is_multiple_of(8) || !min_height.is_multiple_of(8) {
            if !min_width.is_multiple_of(4) || !min_height.is_multiple_of(4) {
                log::error!(
                    target: "HW_GPU",
                    "Texture size ({}x{}) is not multiple of 4",
                    min_width,
                    min_height
                );
                return NULL_SURFACE_ID;
            }
            let (src_surface_id, rect) =
                self.get_surface_sub_rect(&params, ScaleMatch::Ignore, true);

            params.res_scale = self.slot_surfaces[src_surface_id].res_scale;
            let tmp_surface_id = self.create_surface(&params);
            self.sentenced.push((tmp_surface_id, self.frame_tick));

            let (src_surface, tmp_surface) = self
                .slot_surfaces
                .get_pair_mut(src_surface_id, tmp_surface_id);
            let blit = TextureBlit {
                src_level: src_surface.level_of(params.addr),
                dst_level: 0,
                src_rect: rect,
                dst_rect: tmp_surface.get_scaled_rect(0),
                ..Default::default()
            };
            self.runtime.blit_textures(src_surface, tmp_surface, &blit);
            return tmp_surface_id;
        }
        if info.width != (min_width << max_level) || info.height != (min_height << max_level) {
            log::error!(
                target: "HW_GPU",
                "Texture size ({}x{}) does not support required mipmap level ({})",
                params.width,
                params.height,
                max_level
            );
            return NULL_SURFACE_ID;
        }

        let surface_id = self.get_surface_for_params(&params, ScaleMatch::Ignore, true);
        if surface_id.is_valid() {
            surface_id
        } else {
            NULL_SURFACE_ID
        }
    }

    /// Get a texture cube based on the texture configuration
    pub fn get_texture_cube(&mut self, config: &TextureCubeConfig) -> &mut R::Surface {
        if config.width == 0 {
            return &mut self.slot_surfaces[NULL_SURFACE_CUBE_ID];
        }

        let new_surface = !self.texture_cube_cache.contains_key(config);
        let mut cube = self
            .texture_cube_cache
            .get(config)
            .copied()
            .unwrap_or_default();

        let addresses = [
            config.px, config.nx, config.py, config.ny, config.pz, config.nz,
        ];

        if new_surface {
            let mut info = TextureInfo {
                width: config.width,
                height: config.width,
                format: config.format,
                ..Default::default()
            };
            info.set_default_stride();

            let mut res_scale = 1;
            for (i, &address) in addresses.iter().enumerate() {
                if address == 0 {
                    continue;
                }

                if !cube.face_ids[i].is_valid() {
                    info.physical_address = address;
                    cube.face_ids[i] = self.get_texture_surface_for_info(&info, config.levels - 1);
                    let surface = &mut self.slot_surfaces[cube.face_ids[i]];
                    assert!(
                        surface.levels >= config.levels,
                        "Texture cube face levels are not enough to validate the levels requested"
                    );
                    surface.flags.insert(SurfaceFlagBits::TRACKED);
                }
                let surface = &self.slot_surfaces[cube.face_ids[i]];
                res_scale = surface.res_scale.max(res_scale);
            }

            let mut cube_params = SurfaceParams {
                addr: config.px,
                width: config.width,
                height: config.width,
                stride: config.width,
                levels: config.levels,
                res_scale,
                texture_type: TextureType::CubeMap,
                pixel_format: pixel_format_from_texture_format(config.format),
                ty: SurfaceType::Texture,
                ..Default::default()
            };
            cube_params.update_params();
            cube.surface_id = self.create_surface(&cube_params);
        }

        for (i, &address) in addresses.iter().enumerate() {
            let face_id = cube.face_ids[i];
            if address == 0 || !face_id.is_valid() {
                continue;
            }
            let (surface, cube_surface) = self.slot_surfaces.get_pair_mut(face_id, cube.surface_id);
            if cube.ticks[i] == surface.modification_tick {
                continue;
            }
            cube.ticks[i] = surface.modification_tick;
            let mut upload_copies = Vec::with_capacity(8);
            for level in 0..config.levels {
                let width_lod = surface.get_scaled_width() >> level;
                upload_copies.push(TextureCopy {
                    src_level: level,
                    dst_level: level,
                    src_layer: 0,
                    dst_layer: i as u32,
                    src_offset: Offset { x: 0, y: 0 },
                    dst_offset: Offset { x: 0, y: 0 },
                    extent: Extent {
                        width: width_lod,
                        height: width_lod,
                    },
                });
            }
            self.runtime
                .copy_textures(surface, cube_surface, &upload_copies);
        }

        self.texture_cube_cache.insert(*config, cube);
        &mut self.slot_surfaces[cube.surface_id]
    }

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

    /// Get a surface that matches a "texture copy" display transfer config
    pub fn get_tex_copy_surface(&mut self, params: &SurfaceParams) -> SurfaceRectTuple {
        let mut rect = Rectangle::default();

        let match_id = self.find_match(MatchFlags::TEX_COPY, params, ScaleMatch::Ignore, None);

        if match_id.is_valid() {
            self.validate_surface(match_id, params.addr, params.size);

            let match_subrect;
            let match_surface = &self.slot_surfaces[match_id];
            if params.width != params.stride {
                let tiled_size = if match_surface.is_tiled { 8 } else { 1 };
                let mut subrect = *params;
                subrect.width = match_surface.pixels_in_bytes(params.width) / tiled_size;
                subrect.stride = match_surface.pixels_in_bytes(params.stride) / tiled_size;
                subrect.height *= tiled_size;
                match_subrect = subrect;
            } else {
                match_subrect = match_surface.from_interval(params.get_interval());
                assert!(
                    match_subrect.get_interval() == params.get_interval(),
                    "Assertion Failed!"
                );
            }

            rect = match_surface.get_scaled_sub_rect(&match_subrect);
        }

        (match_id, rect)
    }

    /// Iterate over all page indices in a range
    fn for_each_page(addr: u32, size: usize, mut func: impl FnMut(u64)) {
        let page_end = (u64::from(addr) + size as u64 - 1) >> CITRA_PAGEBITS;
        for page in (u64::from(addr) >> CITRA_PAGEBITS)..=page_end {
            func(page);
        }
    }

    /// Iterates over all the surfaces in a region calling func
    fn for_each_surface_in_region(
        &mut self,
        addr: u32,
        size: usize,
        mut func: impl FnMut(&mut Self, SurfaceId),
    ) {
        let mut surfaces: Vec<SurfaceId> = Vec::with_capacity(8);
        let mut pages = Vec::new();
        Self::for_each_page(addr, size, |page| pages.push(page));
        for page in pages {
            let Some(page_surfaces) = self.page_table.get(&page) else {
                continue;
            };
            for surface_id in page_surfaces.clone() {
                let surface = &mut self.slot_surfaces[surface_id];
                if surface.flags.contains(SurfaceFlagBits::PICKED) {
                    continue;
                }
                if !surface.overlaps(addr, size) {
                    continue;
                }

                surface.flags.insert(SurfaceFlagBits::PICKED);
                surfaces.push(surface_id);
                func(self, surface_id);
            }
        }
        for surface_id in surfaces {
            self.slot_surfaces[surface_id]
                .flags
                .remove(SurfaceFlagBits::PICKED);
        }
    }

    /// Get the best surface match (and its match type) for the given flags
    fn find_match(
        &mut self,
        find_flags: MatchFlags,
        params: &SurfaceParams,
        match_scale_type: ScaleMatch,
        validate_interval: Option<SurfaceInterval>,
    ) -> SurfaceId {
        let mut match_id = SurfaceId::default();
        let mut match_valid = false;
        let mut match_scale = 0;
        let mut match_interval = SurfaceInterval::default();

        self.for_each_surface_in_region(params.addr, params.size as usize, |cache, surface_id| {
            let surface = &cache.slot_surfaces[surface_id];
            let res_scale_matched = if match_scale_type == ScaleMatch::Exact {
                params.res_scale == surface.res_scale
            } else {
                params.res_scale <= surface.res_scale
            };
            let is_valid = if find_flags.contains(MatchFlags::COPY) {
                true
            } else {
                surface.is_region_valid(validate_interval.unwrap_or(params.get_interval()))
            };

            let mut is_match_helper =
                |check_type: MatchFlags, match_fn: &dyn Fn() -> (bool, SurfaceInterval)| {
                    if !find_flags.contains(check_type) {
                        return;
                    }

                    let (matched, surface_interval) = match_fn();
                    if !matched {
                        return;
                    }

                    if !res_scale_matched
                        && match_scale_type != ScaleMatch::Ignore
                        && surface.ty != SurfaceType::Fill
                    {
                        return;
                    }

                    // Found a match, update only if this is better than the previous one
                    macro_rules! update_match {
                        () => {
                            match_id = surface_id;
                            match_valid = is_valid;
                            match_scale = surface.res_scale;
                            match_interval = surface_interval;
                        };
                    }

                    if surface.res_scale > match_scale {
                        update_match!();
                        return;
                    } else if surface.res_scale < match_scale {
                        return;
                    }

                    if is_valid && !match_valid {
                        update_match!();
                        return;
                    } else if is_valid != match_valid {
                        return;
                    }

                    if surface_interval.length() > match_interval.length() {
                        update_match!();
                    }
                };
            is_match_helper(MatchFlags::EXACT, &|| {
                (surface.exact_match(params), surface.get_interval())
            });
            is_match_helper(MatchFlags::SUB_RECT, &|| {
                (surface.can_sub_rect(params), surface.get_interval())
            });
            is_match_helper(MatchFlags::COPY, &|| {
                let validate_interval = validate_interval.expect("Assertion Failed!");
                let copy_interval =
                    surface.get_copyable_interval(&params.from_interval(validate_interval));
                let matched = copy_interval.intersect(&validate_interval).length() != 0
                    && surface.can_copy(params, copy_interval);
                (matched, copy_interval)
            });
            is_match_helper(MatchFlags::REINTERPRET, &|| {
                let validate_interval = validate_interval.expect("Assertion Failed!");
                let copy_interval =
                    surface.get_copyable_interval(&params.from_interval(validate_interval));
                let matched = copy_interval.intersect(&validate_interval).length() != 0
                    && surface.can_reinterpret(params);
                (matched, copy_interval)
            });
            is_match_helper(MatchFlags::TEX_COPY, &|| {
                (surface.can_tex_copy(params), surface.get_interval())
            });
        });
        match_id
    }

    /// Update surface's texture for given region when necessary
    fn validate_surface(&mut self, surface_id: SurfaceId, addr: u32, size: u32) {
        if size == 0 {
            return;
        }

        let validate_interval = SurfaceInterval::new(addr, addr.wrapping_add(size));

        let surface = &self.slot_surfaces[surface_id];
        if surface.ty == SurfaceType::Fill {
            assert!(
                surface.is_region_valid(validate_interval),
                "Attempted to validate a non-valid fill surface"
            );
            return;
        }

        let mut validate_regions: SurfaceRegions =
            surface.invalid_regions.intersect(validate_interval);

        if validate_regions.is_empty() {
            return;
        }

        let notify_validated =
            |cache: &mut Self, validate_regions: &mut SurfaceRegions, interval: SurfaceInterval| {
                cache.slot_surfaces[surface_id].mark_valid(interval);
                validate_regions.erase(interval);
            };

        self.runtime.push_debug_scope(
            [0.0, 1.0, 0.0, 1.0],
            format_args!(
                "RasterizerCache::ValidateSurface (from {:#x} to {:#x})",
                addr,
                addr.wrapping_add(size)
            ),
        );

        let mut level = self.slot_surfaces[surface_id].level_of(addr);
        let mut level_interval = self.slot_surfaces[surface_id].level_interval(level);
        while let Some(first) = validate_regions.first() {
            // Take an invalid interval from the validation regions and clamp it
            // to the current level interval. If the interval is empty
            // then we have validated the entire level so move to the next.
            let interval = first.intersect(&level_interval);
            if interval.is_empty() {
                level += 1;
                level_interval = self.slot_surfaces[surface_id].level_interval(level);
                continue;
            }

            // Look for a valid surface to copy from.
            let params = self.slot_surfaces[surface_id].from_interval(interval);
            let copy_surface_id = self.find_match(
                MatchFlags::COPY,
                &params,
                ScaleMatch::Ignore,
                Some(interval),
            );
            if copy_surface_id.is_valid() && copy_surface_id != surface_id {
                let copy_interval =
                    self.slot_surfaces[copy_surface_id].get_copyable_interval(&params);
                self.copy_surface(copy_surface_id, surface_id, copy_interval);
                notify_validated(self, &mut validate_regions, copy_interval);
                continue;
            }

            // Try to find surface in cache with different format
            // that can can be reinterpreted to the requested format.
            if self.validate_by_reinterpretation(surface_id, params, interval) {
                notify_validated(self, &mut validate_regions, interval);
                continue;
            }

            self.flush_region(params.addr, params.size, SurfaceId::default());
            if !self.use_custom_textures || !self.upload_custom_surface(surface_id, interval) {
                self.upload_surface(surface_id, interval);
            }
            notify_validated(self, &mut validate_regions, params.get_interval());
        }

        // Filtered mipmaps often look really bad. We can achieve better quality by
        // generating them from the base level.
        let surface = &mut self.slot_surfaces[surface_id];
        if surface.res_scale != 1 && level != 0 {
            self.runtime.generate_mipmaps(surface);
        }
        self.runtime.pop_debug_scope();
    }

    /// Copies pixel data in interval from the guest VRAM to the host GPU surface
    fn upload_surface(&mut self, surface_id: SurfaceId, interval: SurfaceInterval) {
        let surface = &self.slot_surfaces[surface_id];
        let load_info = surface.from_interval(interval);
        assert!(
            load_info.addr >= surface.addr && load_info.end <= surface.end,
            "Assertion Failed!"
        );

        let staging = self.runtime.find_staging(
            load_info.width * load_info.height * surface.get_internal_bytes_per_pixel(),
            true,
        );

        let Some(upload_data) = get_write_bytes(
            self.memory.as_ref(),
            load_info.addr,
            load_info.end - load_info.addr,
        ) else {
            return;
        };

        decode_texture(
            &load_info,
            load_info.addr,
            load_info.end,
            upload_data,
            unsafe { staging.mapped() },
            self.runtime.needs_conversion(surface.pixel_format),
        );

        let should_dump = !surface.flags.contains(SurfaceFlagBits::CUSTOM)
            && !surface.flags.contains(SurfaceFlagBits::RENDER_TARGET);
        if self.dump_textures && should_dump {
            let hash = self.compute_hash(&load_info, upload_data);
            let level = surface.level_of(load_info.addr);
            self.custom_tex_manager
                .borrow_mut()
                .dump_texture(&load_info, level, upload_data, hash);
        }

        let upload = BufferTextureCopy {
            buffer_offset: staging.offset,
            buffer_size: staging.size,
            texture_rect: surface.get_sub_rect(&load_info),
            texture_level: surface.level_of(load_info.addr),
        };
        let surface = &mut self.slot_surfaces[surface_id];
        surface.upload(&mut self.runtime, &upload, &staging);
    }

    /// Computes the hash of the provided texture data.
    fn compute_hash(&self, load_info: &SurfaceParams, upload_data: &mut [u8]) -> u64 {
        if !self.custom_tex_manager.borrow().use_new_hash() {
            let width = load_info.width;
            let height = load_info.height;
            let bpp = get_format_bytes_per_pixel(load_info.pixel_format);
            let mut decoded = vec![0u8; (width * height * bpp) as usize];
            decode_texture(
                load_info,
                load_info.addr,
                load_info.end,
                upload_data,
                &mut decoded,
                false,
            );
            compute_hash64(&decoded)
        } else {
            compute_hash64(upload_data)
        }
    }

    /// Uploads a custom texture identified with hash to the target surface
    fn upload_custom_surface(&mut self, surface_id: SurfaceId, interval: SurfaceInterval) -> bool {
        let surface = &self.slot_surfaces[surface_id];
        let load_info = surface.from_interval(interval);
        assert!(
            load_info.addr >= surface.addr && load_info.end <= surface.end,
            "Assertion Failed!"
        );

        let Some(upload_data) = get_write_bytes(
            self.memory.as_ref(),
            load_info.addr,
            load_info.end - load_info.addr,
        ) else {
            return false;
        };
        let hash = self.compute_hash(&load_info, upload_data);

        let level = self.slot_surfaces[surface_id].level_of(load_info.addr);
        let material = self.custom_tex_manager.borrow_mut().get_material(hash);

        let Some(material) = material else {
            return self.slot_surfaces[surface_id].is_custom();
        };
        if level != 0 && self.custom_tex_manager.borrow().skip_mipmaps() {
            return true;
        }

        self.slot_surfaces[surface_id]
            .flags
            .insert(SurfaceFlagBits::CUSTOM);

        let decoded = self.custom_tex_manager.borrow_mut().decode(&material);
        if decoded {
            return self.upload_custom(surface_id, &material, level);
        }
        self.async_uploads.push(AsyncUpload {
            material,
            surface_id,
            level,
        });
        false
    }

    /// Replaces the surface with a custom surface if it is not one yet, and uploads the material.
    fn upload_custom(
        &mut self,
        surface_id: SurfaceId,
        material: &Arc<Material>,
        level: u32,
    ) -> bool {
        assert!(
            self.slot_surfaces[surface_id]
                .flags
                .contains(SurfaceFlagBits::CUSTOM),
            "Surface is not suitable for custom upload, aborting!"
        );
        if !self.slot_surfaces[surface_id].is_custom() {
            let old_surface: SurfaceBase = (*self.slot_surfaces[surface_id]).clone();
            let new_surface = self.runtime.create_custom_surface(&old_surface, material);
            let old_id = self.slot_surfaces.swap_and_insert(surface_id, new_surface);
            self.slot_surfaces[old_id]
                .flags
                .remove(SurfaceFlagBits::REGISTERED);
            self.sentenced.push((old_id, self.frame_tick));
        }
        let surface = &mut self.slot_surfaces[surface_id];
        surface.upload_custom(&mut self.runtime, material, level);
        if self.custom_tex_manager.borrow().skip_mipmaps() {
            self.runtime.generate_mipmaps(surface);
        }
        true
    }

    /// Copies pixel data in interval from the host GPU surface to the guest VRAM
    fn download_surface(&mut self, surface_id: SurfaceId, interval: SurfaceInterval) {
        let surface = &self.slot_surfaces[surface_id];
        let flush_info = surface.from_interval(interval);
        let flush_start = interval.first();
        let flush_end = interval.last_next();
        assert!(
            flush_start >= surface.addr && flush_end <= surface.end,
            "Assertion Failed!"
        );

        let staging = self.runtime.find_staging(
            flush_info.width * flush_info.height * surface.get_internal_bytes_per_pixel(),
            false,
        );

        let download = BufferTextureCopy {
            buffer_offset: staging.offset,
            buffer_size: staging.size,
            texture_rect: surface.get_sub_rect(&flush_info),
            texture_level: surface.level_of(flush_start),
        };
        let surface = &mut self.slot_surfaces[surface_id];
        surface.download(&mut self.runtime, &download, &staging);

        let Some(download_dest) =
            get_write_bytes(self.memory.as_ref(), flush_start, flush_end - flush_start)
        else {
            return;
        };

        encode_texture(
            &flush_info,
            flush_start,
            flush_end,
            unsafe { staging.mapped() },
            download_dest,
            self.runtime.needs_conversion(surface.pixel_format),
        );
    }

    /// Downloads a fill surface to guest VRAM
    fn download_fill_surface(&mut self, surface_id: SurfaceId, interval: SurfaceInterval) {
        let surface = &self.slot_surfaces[surface_id];
        let flush_start = interval.first();
        let flush_end = interval.last_next();
        assert!(
            flush_start >= surface.addr && flush_end <= surface.end,
            "Assertion Failed!"
        );

        let Some(dest_ptr) = self.memory.get_physical_ref(flush_start) else {
            return;
        };
        let dest_ptr = unsafe { &mut *dest_ptr };

        let start_offset = flush_start - surface.addr;
        let download_size = (flush_end - flush_start).clamp(0, dest_ptr.len() as u32);
        let coarse_start_offset = start_offset - (start_offset % surface.fill_size);
        let backup_bytes = (start_offset % surface.fill_size) as usize;

        let mut backup_data = [0u8; 4];
        if backup_bytes != 0 {
            let start = coarse_start_offset as usize;
            backup_data[..backup_bytes].copy_from_slice(&dest_ptr[start..start + backup_bytes]);
        }

        let mut offset = coarse_start_offset;
        while offset < download_size {
            let size = surface.fill_size.min(download_size - offset) as usize;
            dest_ptr[offset as usize..offset as usize + size]
                .copy_from_slice(&surface.fill_data[..size]);
            offset += surface.fill_size;
        }

        if backup_bytes != 0 {
            let start = coarse_start_offset as usize;
            dest_ptr[start..start + backup_bytes].copy_from_slice(&backup_data[..backup_bytes]);
        }
    }

    /// Attempt to find a reinterpretable surface in the cache and use it to copy for validation
    fn validate_by_reinterpretation(
        &mut self,
        surface_id: SurfaceId,
        params: SurfaceParams,
        interval: SurfaceInterval,
    ) -> bool {
        let reinterpret_id = self.find_match(
            MatchFlags::REINTERPRET,
            &params,
            ScaleMatch::Ignore,
            Some(interval),
        );
        if reinterpret_id.is_valid() {
            let copy_interval = self.slot_surfaces[reinterpret_id].get_copyable_interval(&params);
            if copy_interval.intersect(&interval).is_empty() {
                return false;
            }
            let (src_surface, surface) =
                self.slot_surfaces.get_pair_mut(reinterpret_id, surface_id);
            let res_scale = src_surface.res_scale;
            if res_scale > surface.res_scale {
                surface.scale_up(&mut self.runtime, res_scale);
            }
            let addr = interval.lower;
            let copy_params = surface.from_interval(copy_interval);
            let src_rect = src_surface.get_scaled_sub_rect(&copy_params);
            let dst_rect = surface.get_scaled_sub_rect(&copy_params);
            let reinterpret = TextureCopy {
                src_level: src_surface.level_of(addr),
                dst_level: surface.level_of(addr),
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
            return self.runtime.reinterpret(src_surface, surface, &reinterpret);
        }

        // No surfaces were found in the cache that had a matching bit-width.
        // Before entering the slow path, check if part of the interval is owned
        // by a gpu modified surface with a different stride than ours. This is indicative
        // of texture aliasing by the guest, which for the vast majority of cases we don't
        // need to validate.
        // TODO: While this works for the vast majority of cases, in Fire Emblem: Shadows of Valentia
        // the warping effect when running in dugeons relies on this stride reinterpretation.
        // In the future this transformation should be properly implemented with a GPU shader.
        match self.dirty_regions.find(interval) {
            Some((_, owner_id)) => {
                owner_id.is_valid()
                    && self.slot_surfaces[owner_id].stride != self.slot_surfaces[surface_id].stride
            }
            None => false,
        }
    }

    /// Clear all cached resources tracked by this cache manager
    pub fn clear_all(&mut self, flush: bool) {
        let flush_interval = Interval::new(0x0, 0xFFFFFFFF);
        // Force flush all surfaces from the cache
        if flush {
            self.flush_region(0x0, 0xFFFFFFFF, SurfaceId::default());
        }
        // Unmark all of the marked pages
        for (pages, _) in self.cached_pages.equal_range(flush_interval) {
            let interval = pages.intersect(&flush_interval);

            let interval_start_addr = interval.first() << CITRA_PAGE_BITS;
            let interval_end_addr = interval.last_next() << CITRA_PAGE_BITS;
            let interval_size = interval_end_addr.wrapping_sub(interval_start_addr);

            self.memory
                .rasterizer_mark_region_cached(interval_start_addr, interval_size, false);
        }

        // Remove the whole cache without really looking at it.
        self.cached_pages.erase(flush_interval);
        self.dirty_regions.clear();
        self.page_table.clear();
    }

    /// Write any cached resources overlapping the region back to memory (if dirty)
    pub fn flush_region(&mut self, addr: u32, size: u32, flush_surface_id: SurfaceId) {
        if size == 0 {
            return;
        }

        let flush_interval = SurfaceInterval::new(addr, addr.wrapping_add(size));
        let mut flushed_intervals = SurfaceRegions::new();

        for (region, surface_id) in self.dirty_regions.equal_range(flush_interval) {
            if flush_surface_id.is_valid() && surface_id != flush_surface_id {
                continue;
            }

            // Small sizes imply that this most likely comes from the cpu, flush the entire region
            // the point is to avoid thousands of small writes every frame if the cpu decides to
            // access that region, anything higher than 8 you're guaranteed it comes from a service
            let interval = if size <= 8 {
                region
            } else {
                region.intersect(&flush_interval)
            };
            let surface = &self.slot_surfaces[surface_id];
            assert!(
                surface.is_region_valid(interval),
                "Region owner has invalid regions"
            );

            self.runtime.push_debug_scope(
                [0.0, 0.0, 0.0, 1.0],
                format_args!(
                    "RasterizerCache::FlushRegion (from {:#x} to {:#x})",
                    interval.lower, interval.upper
                ),
            );

            if surface.ty == SurfaceType::Fill {
                self.download_fill_surface(surface_id, interval);
                flushed_intervals.insert(interval);
                self.runtime.pop_debug_scope();
                continue;
            }

            // Download each requested level of the surface.
            let start_level = surface.level_of(interval.lower);
            let end_level = surface.level_of(interval.upper);
            for level in start_level..=end_level {
                let download_interval =
                    interval.intersect(&self.slot_surfaces[surface_id].level_interval(level));
                if download_interval.is_empty() {
                    continue;
                }
                self.download_surface(surface_id, download_interval);
            }
            flushed_intervals.insert(interval);
            self.runtime.pop_debug_scope();
        }

        // Reset dirty regions
        self.dirty_regions.subtract(&flushed_intervals);
    }

    /// Flush all cached resources tracked by this cache manager
    pub fn flush_all(&mut self) {
        self.flush_region(0, 0xFFFFFFFF, SurfaceId::default());
    }

    /// Mark region as being invalidated by region_owner (nullptr if 3DS memory)
    pub fn invalidate_region(&mut self, addr: u32, size: u32, region_owner_id: SurfaceId) {
        if size == 0 {
            return;
        }

        let invalid_interval = SurfaceInterval::new(addr, addr.wrapping_add(size));

        if region_owner_id.is_valid() {
            let region_owner = &mut self.slot_surfaces[region_owner_id];
            assert!(region_owner.ty != SurfaceType::Texture, "Assertion Failed!");
            assert!(
                addr >= region_owner.addr && addr.wrapping_add(size) <= region_owner.end,
                "Assertion Failed!"
            );
            assert!(
                region_owner.width == region_owner.stride,
                "Assertion Failed!"
            );
            region_owner.mark_valid(invalid_interval);
        }

        let mut remove_surfaces: Vec<SurfaceId> = Vec::with_capacity(4);
        self.for_each_surface_in_region(addr, size as usize, |cache, surface_id| {
            if surface_id == region_owner_id {
                return;
            }
            // If the CPU is invalidating this region we want to remove it
            // to (likely) mark the memory pages as uncached
            if !region_owner_id.is_valid() && size <= 8 {
                let (surface_addr, surface_size) = {
                    let surface = &cache.slot_surfaces[surface_id];
                    (surface.addr, surface.size)
                };
                cache.flush_region(surface_addr, surface_size, surface_id);
                remove_surfaces.push(surface_id);
                return;
            }
            let surface = &mut cache.slot_surfaces[surface_id];
            let interval = surface.get_interval().intersect(&invalid_interval);
            surface.mark_invalid(interval);
            if !surface.is_fully_invalid() {
                return;
            }
            remove_surfaces.push(surface_id);
        });

        if region_owner_id.is_valid() {
            self.dirty_regions.set(invalid_interval, region_owner_id);
        } else {
            self.dirty_regions.erase(invalid_interval);
        }

        for surface_id in remove_surfaces {
            self.unregister_surface(surface_id);
        }
    }

    /// Create a new surface
    fn create_surface(&mut self, params: &SurfaceParams) -> SurfaceId {
        let surface_id = match self
            .sentenced
            .iter()
            .position(|&(surface_id, _)| self.slot_surfaces[surface_id].params == *params)
        {
            None => {
                let surface = self.runtime.create_surface(params);
                self.slot_surfaces.insert(surface)
            }
            Some(index) => {
                let (surface_id, _) = self.sentenced.remove(index);
                surface_id
            }
        };
        let surface = &mut self.slot_surfaces[surface_id];
        if params.res_scale > surface.res_scale {
            surface.scale_up(&mut self.runtime, params.res_scale);
        }
        let interval = surface.get_interval();
        surface.mark_invalid(interval);
        surface_id
    }

    /// Register surface into the cache
    fn register_surface(&mut self, surface_id: SurfaceId) {
        let surface = &mut self.slot_surfaces[surface_id];
        assert!(
            !surface.flags.contains(SurfaceFlagBits::REGISTERED),
            "Trying to register an already registered surface"
        );

        surface.flags.insert(SurfaceFlagBits::REGISTERED);
        let (addr, size) = (surface.addr, surface.size);
        self.update_pages_cached_count(addr, size, 1);
        let page_table = &mut self.page_table;
        Self::for_each_page(addr, size as usize, |page| {
            page_table.entry(page).or_default().push(surface_id);
        });
    }

    /// Remove surface from the cache
    fn unregister_surface(&mut self, surface_id: SurfaceId) {
        let surface = &mut self.slot_surfaces[surface_id];
        assert!(
            surface.flags.contains(SurfaceFlagBits::REGISTERED),
            "Trying to unregister an already unregistered surface"
        );

        surface.flags.remove(SurfaceFlagBits::REGISTERED);
        let (addr, size, ty) = (surface.addr, surface.size, surface.ty);
        self.update_pages_cached_count(addr, size, -1);
        let page_table = &mut self.page_table;
        Self::for_each_page(addr, size as usize, |page| {
            let Some(surfaces) = page_table.get_mut(&page) else {
                panic!(
                    "Unregistering unregistered page=0x{:x}",
                    page << CITRA_PAGEBITS
                );
            };
            let Some(position) = surfaces.iter().position(|&id| id == surface_id) else {
                panic!(
                    "Unregistering unregistered surface in page=0x{:x}",
                    page << CITRA_PAGEBITS
                );
            };
            surfaces.remove(position);
        });

        if ty != SurfaceType::Fill {
            self.remove_texture_cube_face(surface_id);
            self.sentenced.push((surface_id, self.frame_tick));
            return;
        }

        self.slot_surfaces.erase(surface_id);
    }

    /// Unregisters all surfaces from the cache
    fn unregister_all(&mut self) {
        self.flush_all();
        let pages: Vec<u64> = self.page_table.keys().copied().collect();
        for page in pages {
            while let Some(&surface_id) = self
                .page_table
                .get(&page)
                .and_then(|surfaces| surfaces.last())
            {
                self.unregister_surface(surface_id);
            }
        }
        self.runtime.finish();
        self.frame_tick += self.runtime.remove_threshold();
        self.run_garbage_collector();
    }

    /// Increase/decrease the number of surface in pages touching the specified region
    fn update_pages_cached_count(&mut self, addr: u32, size: u32, delta: i32) {
        let num_pages = ((addr.wrapping_add(size).wrapping_sub(1)) >> CITRA_PAGE_BITS)
            .wrapping_sub(addr >> CITRA_PAGE_BITS)
            .wrapping_add(1);
        let page_start = addr >> CITRA_PAGE_BITS;
        let page_end = page_start.wrapping_add(num_pages);

        // Interval maps will erase segments if count reaches 0, so if delta is negative we have to
        // subtract after iterating
        let pages_interval = Interval::new(page_start, page_end);
        if delta > 0 {
            self.cached_pages.add(pages_interval, delta);
        }

        for (pages, count) in self.cached_pages.equal_range(pages_interval) {
            let interval = pages.intersect(&pages_interval);

            let interval_start_addr = interval.first() << CITRA_PAGE_BITS;
            let interval_end_addr = interval.last_next() << CITRA_PAGE_BITS;
            let interval_size = interval_end_addr.wrapping_sub(interval_start_addr);

            if delta > 0 && count == delta {
                self.memory
                    .rasterizer_mark_region_cached(interval_start_addr, interval_size, true);
            } else if delta < 0 && count == -delta {
                self.memory.rasterizer_mark_region_cached(
                    interval_start_addr,
                    interval_size,
                    false,
                );
            } else {
                assert!(count >= 0, "Assertion Failed!");
            }
        }

        if delta < 0 {
            self.cached_pages.add(pages_interval, delta);
        }
    }
}

impl<R: Runtime> Drop for RasterizerCache<R> {
    fn drop(&mut self) {
        self.clear_all(false);
    }
}
