use emu_pica_types::math_util::Rectangle;

use crate::custom_textures::custom_format::CustomPixelFormat;
use crate::interval::Interval;
use crate::pixel_format::{
    PixelFormat, SurfaceType, TextureType, get_format_bpp, get_format_type, pixel_format_as_string,
};

pub type SurfaceInterval = Interval;

pub const MAX_PICA_LEVELS: usize = 8;

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

#[derive(Clone, Copy, Debug)]
pub struct SurfaceParams {
    pub addr: u32,
    pub end: u32,
    pub size: u32,

    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub levels: u32,
    pub res_scale: u32,

    pub is_tiled: bool,
    pub texture_type: TextureType,
    pub pixel_format: PixelFormat,
    pub custom_format: CustomPixelFormat,
    pub ty: SurfaceType,

    pub mipmap_offsets: [u32; MAX_PICA_LEVELS],
}

impl Default for SurfaceParams {
    fn default() -> Self {
        Self {
            addr: 0,
            end: 0,
            size: 0,
            width: 0,
            height: 0,
            stride: 0,
            levels: 1,
            res_scale: 1,
            is_tiled: false,
            texture_type: TextureType::Texture2D,
            pixel_format: PixelFormat::Invalid,
            custom_format: CustomPixelFormat::Invalid,
            ty: SurfaceType::Invalid,
            mipmap_offsets: [0; MAX_PICA_LEVELS],
        }
    }
}

impl PartialEq for SurfaceParams {
    fn eq(&self, other: &Self) -> bool {
        (
            self.addr,
            self.end,
            self.width,
            self.height,
            self.stride,
            self.levels,
            self.is_tiled,
            self.texture_type,
            self.pixel_format,
            self.custom_format,
        ) == (
            other.addr,
            other.end,
            other.width,
            other.height,
            other.stride,
            other.levels,
            other.is_tiled,
            other.texture_type,
            other.pixel_format,
            other.custom_format,
        )
    }
}

impl SurfaceParams {
    /// Returns true if other_surface matches exactly params
    pub fn exact_match(&self, other_surface: &SurfaceParams) -> bool {
        (
            other_surface.addr,
            other_surface.width,
            other_surface.height,
            other_surface.stride,
            other_surface.pixel_format,
            other_surface.is_tiled,
        ) == (
            self.addr,
            self.width,
            self.height,
            self.stride,
            self.pixel_format,
            self.is_tiled,
        ) && self.pixel_format != PixelFormat::Invalid
            && self.levels >= other_surface.levels
    }

    /// Returns true if sub_surface is a subrect of params
    pub fn can_sub_rect(&self, sub_surface: &SurfaceParams) -> bool {
        let level = self.level_of(sub_surface.addr);
        sub_surface.addr >= self.addr
            && sub_surface.end <= self.end
            && sub_surface.pixel_format == self.pixel_format
            && self.pixel_format != PixelFormat::Invalid
            && sub_surface.is_tiled == self.is_tiled
            && sub_surface
                .addr
                .wrapping_sub(self.mipmap_offsets[level as usize])
                .is_multiple_of(self.bytes_in_pixels(if self.is_tiled { 64 } else { 1 }))
            && (sub_surface.stride == (self.stride >> level)
                || sub_surface.height <= if self.is_tiled { 8 } else { 1 })
            && self.get_sub_rect(sub_surface).right <= self.stride
    }

    /// Returns true if other_surface can be used for reinterpretion.
    pub fn can_reinterpret(&self, other_surface: &SurfaceParams) -> bool {
        other_surface.addr >= self.addr
            && other_surface.end <= self.end
            && self.pixel_format != PixelFormat::Invalid
            && self.pixel_format != other_surface.pixel_format
            && self.get_format_bpp() == other_surface.get_format_bpp()
            && other_surface.is_tiled == self.is_tiled
            && other_surface.stride == self.stride
            && (other_surface.addr - self.addr)
                .is_multiple_of(self.bytes_in_pixels(if self.is_tiled { 64 } else { 1 }))
            && self.get_sub_rect(other_surface).right <= self.stride
    }

    /// Returns true if params can be used for texcopy
    pub fn can_tex_copy(&self, texcopy_params: &SurfaceParams) -> bool {
        let copy_interval = texcopy_params.get_interval();
        if self.pixel_format == PixelFormat::Invalid
            || self.addr > texcopy_params.addr
            || self.end < texcopy_params.end
        {
            return false;
        }

        if texcopy_params.width != texcopy_params.stride {
            let tile_stride = self.bytes_in_pixels(self.stride * if self.is_tiled { 8 } else { 1 });
            let tile_bytes = self.bytes_in_pixels(if self.is_tiled { 64 } else { 1 });
            return (texcopy_params.addr - self.addr).is_multiple_of(tile_bytes)
                && texcopy_params.width.is_multiple_of(tile_bytes)
                && (texcopy_params.height == 1 || texcopy_params.stride == tile_stride)
                && ((texcopy_params.addr - self.addr) % tile_stride) + texcopy_params.width
                    <= tile_stride;
        }

        let target_level = self.level_of(texcopy_params.addr);
        if self.level_interval(target_level).intersect(&copy_interval) != copy_interval {
            return false;
        }

        self.from_interval(copy_interval).get_interval() == copy_interval
    }

    /// Updates remaining members from the already set addr, width, height and pixel_format
    pub fn update_params(&mut self) {
        if self.stride == 0 {
            self.stride = self.width;
        }

        self.ty = get_format_type(self.pixel_format);
        if self.levels != 1 {
            assert!(self.stride == self.width, "Assertion Failed!");
            self.calculate_mip_level_offsets();
            self.size = self.calculate_surface_size();
        } else {
            self.mipmap_offsets[0] = self.addr;
            self.size = if !self.is_tiled {
                self.bytes_in_pixels(
                    self.stride
                        .wrapping_mul(self.height.wrapping_sub(1))
                        .wrapping_add(self.width),
                )
            } else {
                self.bytes_in_pixels(
                    self.stride
                        .wrapping_mul(8)
                        .wrapping_mul((self.height / 8).wrapping_sub(1))
                        .wrapping_add(self.width * 8),
                )
            };
        }

        self.end = self.addr.wrapping_add(self.size);
    }

    /// Returns the unscaled rectangle referenced by sub_surface
    pub fn get_sub_rect(&self, sub_surface: &SurfaceParams) -> Rectangle<u32> {
        let level = self.level_of(sub_surface.addr);
        let begin_pixel_index =
            self.pixels_in_bytes(sub_surface.addr - self.mipmap_offsets[level as usize]);
        assert!(self.stride == self.width || level == 0, "Assertion Failed!");

        let stride_lod = self.stride >> level;
        if self.is_tiled {
            let x0 = (begin_pixel_index % (stride_lod * 8)) / 8;
            let y0 = (begin_pixel_index / (stride_lod * 8)) * 8;
            let height_lod = self.height >> level;
            // Top to bottom
            return Rectangle::new(
                x0,
                height_lod.wrapping_sub(y0),
                x0 + sub_surface.width,
                height_lod.wrapping_sub(y0 + sub_surface.height),
            );
        }

        let x0 = begin_pixel_index % stride_lod;
        let y0 = begin_pixel_index / stride_lod;
        // Bottom to top
        Rectangle::new(x0, y0 + sub_surface.height, x0 + sub_surface.width, y0)
    }

    /// Returns the scaled rectangle referenced by sub_surface
    pub fn get_scaled_sub_rect(&self, sub_surface: &SurfaceParams) -> Rectangle<u32> {
        self.get_sub_rect(sub_surface).mul(self.res_scale)
    }

    /// Returns the outer rectangle containing interval
    pub fn from_interval(&self, interval: SurfaceInterval) -> SurfaceParams {
        let mut params = *self;
        let level = self.level_of(interval.lower);
        let end_addr = interval.upper;

        // Ensure provided interval is contained in a single level
        assert!(
            level == self.level_of(end_addr)
                || end_addr == self.end
                || end_addr == self.mipmap_offsets[level as usize + 1],
            "Assertion Failed!"
        );

        params.width >>= level;
        params.stride >>= level;

        let tiled_size = if self.is_tiled { 8 } else { 1 };
        let stride_tiled_bytes = self.bytes_in_pixels(params.stride * tiled_size);
        assert!(self.stride == self.width || level == 0, "Assertion Failed!");

        let start = self.mipmap_offsets[level as usize];
        let mut aligned_start = start + align_down(interval.first() - start, stride_tiled_bytes);
        let mut aligned_end = start + align_up(interval.last_next() - start, stride_tiled_bytes);

        if aligned_end - aligned_start > stride_tiled_bytes {
            params.addr = aligned_start;
            params.height = (aligned_end - aligned_start) / self.bytes_in_pixels(params.stride);
        } else {
            // 1 row
            assert!(
                aligned_end - aligned_start == stride_tiled_bytes,
                "Assertion Failed!"
            );
            let tiled_alignment = self.bytes_in_pixels(if self.is_tiled { 8 * 8 } else { 1 });

            aligned_start = start + align_down(interval.first() - start, tiled_alignment);
            aligned_end = start + align_up(interval.last_next() - start, tiled_alignment);

            params.addr = aligned_start;
            params.width = self.pixels_in_bytes(aligned_end - aligned_start) / tiled_size;
            params.stride = params.width;
            params.height = tiled_size;
        }

        params.levels = 1;
        params.update_params();
        params
    }

    /// Returns the address interval referenced by unscaled_rect
    pub fn get_sub_rect_interval(
        &self,
        mut unscaled_rect: Rectangle<u32>,
        level: u32,
    ) -> SurfaceInterval {
        if unscaled_rect.get_height() == 0 || unscaled_rect.get_width() == 0 {
            return SurfaceInterval::default();
        }

        if self.is_tiled {
            unscaled_rect.left = align_down(unscaled_rect.left, 8) * 8;
            unscaled_rect.bottom = align_down(unscaled_rect.bottom, 8) / 8;
            unscaled_rect.right = align_up(unscaled_rect.right, 8) * 8;
            unscaled_rect.top = align_up(unscaled_rect.top, 8) / 8;
        }

        let stride_tiled = (if !self.is_tiled {
            self.stride
        } else {
            self.stride * 8
        }) >> level;
        let pixels = (unscaled_rect.get_height() - 1)
            .wrapping_mul(stride_tiled)
            .wrapping_add(unscaled_rect.get_width());
        let pixel_offset = stride_tiled
            .wrapping_mul(if !self.is_tiled {
                unscaled_rect.bottom
            } else {
                (self.height / 8).wrapping_sub(unscaled_rect.top)
            })
            .wrapping_add(unscaled_rect.left);

        let start = self.mipmap_offsets[level as usize];
        SurfaceInterval::new(
            start.wrapping_add(self.bytes_in_pixels(pixel_offset)),
            start.wrapping_add(self.bytes_in_pixels(pixel_offset.wrapping_add(pixels))),
        )
    }

    /// Computes the offset of each mipmap level
    fn calculate_mip_level_offsets(&mut self) {
        assert!(
            self.levels as usize <= MAX_PICA_LEVELS && self.stride == self.width,
            "Assertion Failed!"
        );

        let mut level_width = self.width;
        let mut level_height = self.height;
        let mut offset = self.addr;

        for level in 0..self.levels as usize {
            self.mipmap_offsets[level] = offset;
            offset = offset.wrapping_add(self.bytes_in_pixels(level_width * level_height));

            level_width >>= 1;
            level_height >>= 1;
        }
    }

    /// Calculates total surface size taking mipmaps into account
    fn calculate_surface_size(&self) -> u32 {
        assert!(
            self.levels as usize <= MAX_PICA_LEVELS && self.stride == self.width,
            "Assertion Failed!"
        );

        let mut level_width = self.width;
        let mut level_height = self.height;
        let mut size = 0u32;

        for _ in 0..self.levels {
            size = size.wrapping_add(self.bytes_in_pixels(level_width * level_height));
            level_width >>= 1;
            level_height >>= 1;
        }
        size
    }

    /// Return the address interval of the provided level
    pub fn level_interval(&self, level: u32) -> SurfaceInterval {
        assert!(self.levels > level, "Assertion Failed!");
        let start_addr = self.mipmap_offsets[level as usize];
        let end_addr = if level == self.levels - 1 {
            self.end
        } else {
            self.mipmap_offsets[level as usize + 1]
        };
        SurfaceInterval::new(start_addr, end_addr)
    }

    /// Returns the level of the provided address
    pub fn level_of(&self, level_addr: u32) -> u32 {
        if level_addr < self.addr || level_addr > self.end {
            return 0;
        }

        let mut level = self.levels - 1;
        while self.mipmap_offsets[level as usize] > level_addr {
            level -= 1;
        }
        level
    }

    /// Returns a string identifier of the params object
    pub fn debug_name(&self, scaled: bool, custom: bool) -> String {
        let scaled_width = if scaled {
            self.get_scaled_width()
        } else {
            self.width
        };
        let scaled_height = if scaled {
            self.get_scaled_height()
        } else {
            self.height
        };
        format!(
            "Surface: {}x{} {} {} levels from {:#x} to {:#x} ({}{})",
            scaled_width,
            scaled_height,
            pixel_format_as_string(self.pixel_format),
            self.levels,
            self.addr,
            self.end,
            if custom { "custom," } else { "" },
            if scaled { "scaled" } else { "unscaled" }
        )
    }

    pub fn get_interval(&self) -> SurfaceInterval {
        SurfaceInterval::new(self.addr, self.end)
    }

    pub fn get_format_bpp(&self) -> u32 {
        get_format_bpp(self.pixel_format)
    }

    pub fn get_scaled_width(&self) -> u32 {
        self.width * self.res_scale
    }

    pub fn get_scaled_height(&self) -> u32 {
        self.height * self.res_scale
    }

    pub fn get_rect(&self, level: u32) -> Rectangle<u32> {
        Rectangle::new(0, self.height >> level, self.width >> level, 0)
    }

    pub fn get_scaled_rect(&self, level: u32) -> Rectangle<u32> {
        Rectangle::new(
            0,
            self.get_scaled_height() >> level,
            self.get_scaled_width() >> level,
            0,
        )
    }

    pub fn pixels_in_bytes(&self, size: u32) -> u32 {
        size.wrapping_mul(8) / self.get_format_bpp()
    }

    pub fn bytes_in_pixels(&self, pixels: u32) -> u32 {
        pixels.wrapping_mul(self.get_format_bpp()) / 8
    }
}
