mod accelerate;
mod framebuffer;
mod matching;
mod registry;
mod surfaces;
mod transfer;
mod validation;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use emu_pica_types::math_util::Rectangle;
use emu_pica_types::regs::texturing::{TextureFilter, WrapMode};

use crate::custom_textures::custom_tex_manager::{CustomTexManager, MAX_UPLOADS_PER_TICK};
use crate::interval::IntervalMap;
use crate::pixel_format::{PixelFormat, SurfaceType, TextureType};
use crate::rasterizer_cache_base::{
    AsyncUpload, CacheMemory, CacheSettings, RasterizerCache, Runtime,
};
use crate::sampler_params::SamplerParams;
use crate::slot_id::{NULL_SURFACE_ID, SurfaceId};
use crate::slot_vector::SlotVector;
use crate::surface_base::SurfaceFlagBits;
use crate::surface_params::SurfaceParams;
use crate::utils::{ClearValue, TextureClear};

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
}

impl<R: Runtime> Drop for RasterizerCache<R> {
    fn drop(&mut self) {
        self.clear_all(false);
    }
}
