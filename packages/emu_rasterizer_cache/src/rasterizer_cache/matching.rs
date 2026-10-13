use super::CITRA_PAGEBITS;
use crate::pixel_format::SurfaceType;
use crate::rasterizer_cache_base::{MatchFlags, RasterizerCache, Runtime, ScaleMatch};
use crate::slot_id::SurfaceId;
use crate::surface_base::SurfaceFlagBits;
use crate::surface_params::{SurfaceInterval, SurfaceParams};

impl<R: Runtime> RasterizerCache<R> {
    /// Iterate over all page indices in a range
    pub(super) fn for_each_page(addr: u32, size: usize, mut func: impl FnMut(u64)) {
        let page_end = (u64::from(addr) + size as u64 - 1) >> CITRA_PAGEBITS;
        for page in (u64::from(addr) >> CITRA_PAGEBITS)..=page_end {
            func(page);
        }
    }

    /// Iterates over all the surfaces in a region calling func
    pub(super) fn for_each_surface_in_region(
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
    pub(super) fn find_match(
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
}
