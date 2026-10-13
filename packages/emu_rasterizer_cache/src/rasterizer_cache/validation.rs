use crate::pixel_format::SurfaceType;
use crate::rasterizer_cache_base::{MatchFlags, RasterizerCache, Runtime, ScaleMatch, Surface};
use crate::slot_id::SurfaceId;
use crate::surface_base::SurfaceRegions;
use crate::surface_params::{SurfaceInterval, SurfaceParams};
use crate::utils::{Extent, Offset, TextureCopy};

impl<R: Runtime> RasterizerCache<R> {
    /// Update surface's texture for given region when necessary
    pub(super) fn validate_surface(&mut self, surface_id: SurfaceId, addr: u32, size: u32) {
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
}
