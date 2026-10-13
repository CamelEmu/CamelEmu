use emu_pica_types::math_util::Rectangle;
use emu_pica_types::regs::texturing::{FullTextureConfig, TextureConfig, TextureFilter};

use super::{SurfaceRectTuple, align_up};
use crate::pixel_format::{SurfaceType, TextureType, pixel_format_from_texture_format};
use crate::rasterizer_cache_base::{
    MatchFlags, RasterizerCache, Runtime, ScaleMatch, Surface, TextureSampling,
};
use crate::sampler_params::SamplerParams;
use crate::slot_id::{NULL_SURFACE_CUBE_ID, NULL_SURFACE_ID, SamplerId, SurfaceId};
use crate::surface_base::SurfaceFlagBits;
use crate::surface_params::{SurfaceInterval, SurfaceParams};
use crate::texture::texture_decode::TextureInfo;
use crate::texture_cube::TextureCubeConfig;
use crate::utils::{Extent, Offset, TextureBlit, TextureClear, TextureCopy, mip_levels};

impl<R: Runtime> RasterizerCache<R> {
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
}
