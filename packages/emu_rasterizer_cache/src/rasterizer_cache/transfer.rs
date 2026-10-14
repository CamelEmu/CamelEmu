use std::sync::Arc;

use emu_pica_types::hash::compute_hash64;

use super::get_write_bytes;
use crate::custom_textures::material::Material;
use crate::pixel_format::get_format_bytes_per_pixel;
use crate::rasterizer_cache_base::{AsyncUpload, RasterizerCache, Runtime, Surface};
use crate::slot_id::SurfaceId;
use crate::surface_base::{SurfaceBase, SurfaceFlagBits};
use crate::surface_params::{SurfaceInterval, SurfaceParams};
use crate::utils::{BufferTextureCopy, decode_texture, encode_texture};

impl<R: Runtime> RasterizerCache<R> {
    /// Copies pixel data in interval from the guest VRAM to the host GPU surface
    pub(super) fn upload_surface(&mut self, surface_id: SurfaceId, interval: SurfaceInterval) {
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
    pub(super) fn upload_custom_surface(
        &mut self,
        surface_id: SurfaceId,
        interval: SurfaceInterval,
    ) -> bool {
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
    pub(super) fn upload_custom(
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
    pub(super) fn download_surface(&mut self, surface_id: SurfaceId, interval: SurfaceInterval) {
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
    pub(super) fn download_fill_surface(
        &mut self,
        surface_id: SurfaceId,
        interval: SurfaceInterval,
    ) {
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
}
