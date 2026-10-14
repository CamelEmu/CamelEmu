use super::{CITRA_PAGE_BITS, CITRA_PAGEBITS};
use crate::interval::Interval;
use crate::pixel_format::SurfaceType;
use crate::rasterizer_cache_base::{RasterizerCache, Runtime, Surface};
use crate::slot_id::SurfaceId;
use crate::surface_base::{SurfaceFlagBits, SurfaceRegions};
use crate::surface_params::{SurfaceInterval, SurfaceParams};

impl<R: Runtime> RasterizerCache<R> {
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
    pub(super) fn create_surface(&mut self, params: &SurfaceParams) -> SurfaceId {
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
    pub(super) fn register_surface(&mut self, surface_id: SurfaceId) {
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
    pub(super) fn unregister_all(&mut self) {
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
