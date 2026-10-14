use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use emu_pica_types::regs::texturing::TextureFormat;

use crate::custom_textures::custom_format::CustomPixelFormat;
use crate::custom_textures::material::{MapType, Material};
use crate::interval::IntervalSet;
use crate::pixel_format::{PixelFormat, SurfaceType, get_format_type};
use crate::surface_params::{SurfaceInterval, SurfaceParams};
use crate::texture::texture_decode::{TextureInfo, lookup_texture};
use crate::utils::{ClearValue, Extent};

pub type SurfaceRegions = IntervalSet;

const fn align_down(value: u32, size: u32) -> u32 {
    value - value % size
}

const fn align_up(value: u32, size: u32) -> u32 {
    let remainder = value % size;
    if remainder == 0 {
        value
    } else {
        value - remainder + size
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SurfaceFlagBits(pub u32);

impl SurfaceFlagBits {
    pub const REGISTERED: Self = Self(1 << 0); // Surface is registed in the rasterizer cache.
    pub const PICKED: Self = Self(1 << 1); // Surface has been picked when searching for a match.
    pub const TRACKED: Self = Self(1 << 2); // Surface is part of a texture cube and should be tracked.
    pub const CUSTOM: Self = Self(1 << 3); // Surface texture has been replaced with a custom texture.
    pub const SHADOW_MAP: Self = Self(1 << 4); // Surface is used during shadow rendering.
    pub const RENDER_TARGET: Self = Self(1 << 5); // Surface was a render target.

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: Self) {
        self.0 &= !other.0;
    }
}

#[derive(Clone, Debug)]
pub struct SurfaceBase {
    pub params: SurfaceParams,
    pub flags: SurfaceFlagBits,
    pub material: Option<Arc<Material>>,
    pub invalid_regions: SurfaceRegions,
    pub fill_size: u32,
    pub fill_data: [u8; 4],
    pub modification_tick: u64,
}

impl Deref for SurfaceBase {
    type Target = SurfaceParams;

    fn deref(&self) -> &SurfaceParams {
        &self.params
    }
}

impl DerefMut for SurfaceBase {
    fn deref_mut(&mut self) -> &mut SurfaceParams {
        &mut self.params
    }
}

impl SurfaceBase {
    pub fn new(params: &SurfaceParams) -> Self {
        Self {
            params: *params,
            flags: SurfaceFlagBits::default(),
            material: None,
            invalid_regions: SurfaceRegions::new(),
            fill_size: 0,
            fill_data: [0; 4],
            modification_tick: 1,
        }
    }

    /// Returns true when this surface can be used to fill the fill_interval of dest_surface
    pub fn can_fill(&self, dest_surface: &SurfaceParams, fill_interval: SurfaceInterval) -> bool {
        if self.ty == SurfaceType::Fill
            && self.is_region_valid(fill_interval)
            && fill_interval.first() >= self.addr
            && fill_interval.last_next() <= self.end // dest_surface is within our fill range
            && dest_surface.from_interval(fill_interval).get_interval() == fill_interval
        {
            // make sure interval is a rectangle in dest surface
            if self.fill_size * 8 != dest_surface.get_format_bpp() {
                // Check if bits repeat for our fill_size
                let dest_bytes_per_pixel = (dest_surface.get_format_bpp() / 8).max(1) as usize;
                let fill_size = self.fill_size as usize;
                let mut fill_test = vec![0u8; fill_size * dest_bytes_per_pixel];

                for i in 0..dest_bytes_per_pixel {
                    fill_test[i * fill_size..(i + 1) * fill_size]
                        .copy_from_slice(&self.fill_data[..fill_size]);
                }

                for i in 0..fill_size {
                    if fill_test[dest_bytes_per_pixel * i..dest_bytes_per_pixel * (i + 1)]
                        != fill_test[..dest_bytes_per_pixel]
                    {
                        return false;
                    }
                }

                if dest_surface.get_format_bpp() == 4 && (fill_test[0] & 0xF) != (fill_test[0] >> 4)
                {
                    return false;
                }
            }
            return true;
        }
        false
    }

    /// Returns true when surface can validate copy_interval of dest_surface
    pub fn can_copy(&self, dest_surface: &SurfaceParams, copy_interval: SurfaceInterval) -> bool {
        let subrect_params = dest_surface.from_interval(copy_interval);
        assert!(
            subrect_params.get_interval() == copy_interval,
            "Assertion Failed!"
        );

        if self.can_sub_rect(&subrect_params) {
            return true;
        }

        if self.can_fill(dest_surface, copy_interval) {
            return true;
        }

        false
    }

    /// Returns the region of the biggest valid rectange within interval
    pub fn get_copyable_interval(&self, params: &SurfaceParams) -> SurfaceInterval {
        let mut result = SurfaceInterval::default();
        let tile_align = params.bytes_in_pixels(if params.is_tiled { 8 * 8 } else { 1 });
        let valid_regions =
            SurfaceRegions::from_interval(params.get_interval().intersect(&self.get_interval()))
                .difference(&self.invalid_regions);

        for valid_interval in valid_regions.iter() {
            let aligned_interval = SurfaceInterval::new(
                params.addr + align_up(valid_interval.first() - params.addr, tile_align),
                params.addr + align_down(valid_interval.last_next() - params.addr, tile_align),
            );

            if tile_align > valid_interval.length() || aligned_interval.length() == 0 {
                continue;
            }

            // Get the rectangle within aligned_interval
            let stride_bytes =
                params.bytes_in_pixels(params.stride) * if params.is_tiled { 8 } else { 1 };
            let mut rect_interval = SurfaceInterval::new(
                params.addr + align_up(aligned_interval.first() - params.addr, stride_bytes),
                params.addr + align_down(aligned_interval.last_next() - params.addr, stride_bytes),
            );

            if rect_interval.first() > rect_interval.last_next() {
                // 1 row
                rect_interval = aligned_interval;
            } else if rect_interval.length() == 0 {
                // 2 rows that do not make a rectangle, return the larger one
                let row1 = SurfaceInterval::new(aligned_interval.first(), rect_interval.first());
                let row2 =
                    SurfaceInterval::new(rect_interval.first(), aligned_interval.last_next());
                rect_interval = if row1.length() > row2.length() {
                    row1
                } else {
                    row2
                };
            }

            if rect_interval.length() > result.length() {
                result = rect_interval;
            }
        }
        result
    }

    /// Returns the clear value used to validate another surface from this fill surface
    pub fn make_clear_value(&self, copy_addr: u32, dst_format: PixelFormat) -> ClearValue {
        let dst_type = get_format_type(dst_format);
        let fill_buffer = self.make_fill_buffer(copy_addr);

        let mut result = ClearValue::default();
        match dst_type {
            SurfaceType::Color | SurfaceType::Texture | SurfaceType::Fill => {
                let tex_info = TextureInfo {
                    format: TextureFormat(dst_format as u32),
                    ..Default::default()
                };
                let color = lookup_texture(fill_buffer.as_ptr(), 0, 0, &tex_info, false);
                result.color = [
                    f32::from(color.x) / 255.0,
                    f32::from(color.y) / 255.0,
                    f32::from(color.z) / 255.0,
                    f32::from(color.w) / 255.0,
                ];
            }
            SurfaceType::Depth => {
                let mut depth_uint = 0u32;
                if dst_format == PixelFormat::D16 {
                    depth_uint = u32::from(u16::from_le_bytes([fill_buffer[0], fill_buffer[1]]));
                    result.depth = depth_uint as f32 / 65535.0; // 2^16 - 1
                } else if dst_format == PixelFormat::D24 {
                    depth_uint |=
                        u32::from_le_bytes([fill_buffer[0], fill_buffer[1], fill_buffer[2], 0]);
                    result.depth = depth_uint as f32 / 16777215.0; // 2^24 - 1
                }
            }
            SurfaceType::DepthStencil => {
                let clear_value_uint = u32::from_ne_bytes(fill_buffer);
                result.depth = (clear_value_uint & 0xFFFFFF) as f32 / 16777215.0; // 2^24 - 1
                result.stencil = (clear_value_uint >> 24) as u8;
            }
            _ => unreachable!("Invalid surface type!"),
        }

        result
    }

    /// Returns the internal surface extent.
    pub fn real_extent(&self, scaled: bool) -> Extent {
        let is_custom = self.is_custom();
        let mut real_width = self.width;
        let mut real_height = self.height;
        if is_custom {
            let material = self
                .material
                .as_ref()
                .expect("a custom surface has a material");
            real_width = material.width;
            real_height = material.height;
        } else if scaled {
            real_width = self.get_scaled_width();
            real_height = self.get_scaled_height();
        }
        Extent {
            width: real_width,
            height: real_height,
        }
    }

    /// Returns true if the surface contains a custom material with a normal map.
    pub fn has_normal_map(&self) -> bool {
        self.material
            .as_ref()
            .is_some_and(|material| material.map(MapType::Normal).is_some())
    }

    pub fn overlaps(&self, overlap_addr: u32, overlap_size: usize) -> bool {
        let overlap_end = overlap_addr.wrapping_add(overlap_size as u32);
        self.addr < overlap_end && overlap_addr < self.end
    }

    pub fn modification_tick(&self) -> u64 {
        self.modification_tick
    }

    pub fn is_custom(&self) -> bool {
        self.flags.contains(SurfaceFlagBits::CUSTOM)
            && self.custom_format != CustomPixelFormat::Invalid
    }

    pub fn is_region_valid(&self, interval: SurfaceInterval) -> bool {
        self.invalid_regions.find(interval).is_none()
    }

    pub fn mark_valid(&mut self, interval: SurfaceInterval) {
        self.invalid_regions.erase(interval);
        self.modification_tick += 1;
    }

    pub fn mark_invalid(&mut self, interval: SurfaceInterval) {
        self.invalid_regions.insert(interval);
        self.modification_tick += 1;
    }

    pub fn is_fully_invalid(&self) -> bool {
        let interval = self.get_interval();
        self.invalid_regions.find(interval) == Some(interval)
    }

    /// Returns the fill buffer value starting from copy_addr
    fn make_fill_buffer(&self, copy_addr: u32) -> [u8; 4] {
        let fill_offset = (copy_addr - self.addr) % self.fill_size;
        let mut fill_buffer = [0u8; 4];

        for (fill_buff_pos, value) in (fill_offset..).zip(fill_buffer.iter_mut()) {
            *value = self.fill_data[(fill_buff_pos % self.fill_size) as usize];
        }

        fill_buffer
    }
}
