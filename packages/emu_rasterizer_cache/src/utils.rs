use emu_pica_types::math_util::Rectangle;

use crate::surface_params::SurfaceParams;
use crate::texture_codec::{
    LINEAR_DECODE_TABLE, LINEAR_DECODE_TABLE_CONVERTED, LINEAR_ENCODE_TABLE,
    LINEAR_ENCODE_TABLE_CONVERTED, SWIZZLE_TABLE, SWIZZLE_TABLE_CONVERTED, UNSWIZZLE_TABLE,
    UNSWIZZLE_TABLE_CONVERTED,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Offset {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extent {
    pub width: u32,
    pub height: u32,
}

impl Default for Extent {
    fn default() -> Self {
        Self {
            width: 1,
            height: 1,
        }
    }
}

/// The value a texture is cleared with: a color, or a depth and a stencil.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ClearValue {
    pub color: [f32; 4],
    pub depth: f32,
    pub stencil: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureClear {
    pub texture_level: u32,
    pub texture_rect: Rectangle<u32>,
    pub value: ClearValue,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureCopy {
    pub src_level: u32,
    pub dst_level: u32,
    pub src_layer: u32,
    pub dst_layer: u32,
    pub src_offset: Offset,
    pub dst_offset: Offset,
    pub extent: Extent,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureBlit {
    pub src_level: u32,
    pub dst_level: u32,
    pub src_layer: u32,
    pub dst_layer: u32,
    pub src_rect: Rectangle<u32>,
    pub dst_rect: Rectangle<u32>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BufferTextureCopy {
    pub buffer_offset: u32,
    pub buffer_size: u32,
    pub texture_rect: Rectangle<u32>,
    pub texture_level: u32,
}

/// A region of a staging buffer that the runtime maps for an upload or a download.
#[derive(Clone, Copy, Debug)]
pub struct StagingData {
    pub size: u32,
    pub offset: u32,
    pub mapped: *mut u8,
    pub mapped_size: usize,
}

impl StagingData {
    /// The mapped bytes of the staging buffer.
    ///
    /// # Safety
    /// The runtime keeps the mapping valid until the upload or the download that uses it is done.
    pub unsafe fn mapped<'a>(&self) -> &'a mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.mapped, self.mapped_size) }
    }
}

pub fn mip_levels(mut width: u32, mut height: u32, max_level: u32) -> u32 {
    let mut levels = 1;
    while width > 8 && height > 8 {
        levels += 1;
        width >>= 1;
        height >>= 1;
    }

    levels.min(max_level + 1)
}

/**
 * Encodes a linear texture to the expected linear or tiled format.
 *
 * @param surface_info Structure used to query the surface information.
 * @param start_addr The start address of the dest data. Used if tiled.
 * @param end_addr The end address of the dest data. Used if tiled.
 * @param source_tiled The source linear texture data.
 * @param dest_linear The output buffer where the encoded linear or tiled data will be written to.
 * @param convert Whether the pixel format needs to be converted.
 */
pub fn encode_texture(
    surface_info: &SurfaceParams,
    start_addr: u32,
    end_addr: u32,
    source: &mut [u8],
    dest: &mut [u8],
    convert: bool,
) {
    let format = surface_info.pixel_format;
    let func_index = format as u32;

    if surface_info.is_tiled {
        let swizzle_impl = if convert {
            SWIZZLE_TABLE_CONVERTED
        } else {
            SWIZZLE_TABLE
        }[func_index as usize];
        if let Some(swizzle_impl) = swizzle_impl {
            swizzle_impl(
                surface_info.width,
                surface_info.height,
                start_addr - surface_info.addr,
                end_addr - surface_info.addr,
                source,
                dest,
            );
            return;
        }
    } else {
        let linear_encode_impl = if convert {
            LINEAR_ENCODE_TABLE_CONVERTED
        } else {
            LINEAR_ENCODE_TABLE
        }[func_index as usize];
        if let Some(linear_encode_impl) = linear_encode_impl {
            linear_encode_impl(source, dest);
            return;
        }
    }

    log::error!(
        target: "HW_GPU",
        "Unimplemented texture encode function for pixel format = {}, tiled = {}",
        func_index,
        surface_info.is_tiled
    );
    log::error!(target: "Debug", "Unimplemented code!");
}

/**
 * Decodes a linear or tiled texture to the expected linear format.
 *
 * @param surface_info Structure used to query the surface information.
 * @param start_addr The start address of the source data. Used if tiled.
 * @param end_addr The end address of the source data. Used if tiled.
 * @param source_tiled The source linear or tiled texture data.
 * @param dest_linear The output buffer where the decoded linear data will be written to.
 * @param convert Whether the pixel format needs to be converted.
 */
pub fn decode_texture(
    surface_info: &SurfaceParams,
    start_addr: u32,
    end_addr: u32,
    source: &mut [u8],
    dest: &mut [u8],
    convert: bool,
) {
    let format = surface_info.pixel_format;
    let func_index = format as u32;

    if surface_info.is_tiled {
        let unswizzle_impl = if convert {
            UNSWIZZLE_TABLE_CONVERTED
        } else {
            UNSWIZZLE_TABLE
        }[func_index as usize];
        if let Some(unswizzle_impl) = unswizzle_impl {
            unswizzle_impl(
                surface_info.width,
                surface_info.height,
                start_addr - surface_info.addr,
                end_addr - surface_info.addr,
                dest,
                source,
            );
            return;
        }
    } else {
        let linear_decode_impl = if convert {
            LINEAR_DECODE_TABLE_CONVERTED
        } else {
            LINEAR_DECODE_TABLE
        }[func_index as usize];
        if let Some(linear_decode_impl) = linear_decode_impl {
            linear_decode_impl(source, dest);
            return;
        }
    }

    log::error!(
        target: "HW_GPU",
        "Unimplemented texture decode function for pixel format = {}, tiled = {}",
        func_index,
        surface_info.is_tiled
    );
    log::error!(target: "Debug", "Unimplemented code!");
}
