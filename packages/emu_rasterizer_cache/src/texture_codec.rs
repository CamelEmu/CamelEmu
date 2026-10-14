use emu_pica_types::Vec4;
use emu_pica_types::color::{
    average_rgb_components, convert_4_to_8, convert_8_to_4, decode_a8, decode_d24, decode_i8,
    decode_ia4, decode_ia8, decode_rg8, decode_rgb5a1, decode_rgb8, decode_rgb565, decode_rgba4,
    decode_rgba8, encode_a8, encode_d24, encode_i8, encode_ia4, encode_ia8, encode_rg8,
    encode_rgb5a1, encode_rgb8, encode_rgb565, encode_rgba4, encode_rgba8,
};
use emu_pica_types::utils::morton_interleave;

use crate::pixel_format::{FORMAT_MAP, PixelFormat};
use crate::texture::etc1::sample_etc1_subtile;

const RGBA8: u32 = PixelFormat::RGBA8 as u32;
const RGB8: u32 = PixelFormat::RGB8 as u32;
const RGB5A1: u32 = PixelFormat::RGB5A1 as u32;
const RGB565: u32 = PixelFormat::RGB565 as u32;
const RGBA4: u32 = PixelFormat::RGBA4 as u32;
const IA8: u32 = PixelFormat::IA8 as u32;
const RG8: u32 = PixelFormat::RG8 as u32;
const I8: u32 = PixelFormat::I8 as u32;
const A8: u32 = PixelFormat::A8 as u32;
const IA4: u32 = PixelFormat::IA4 as u32;
const I4: u32 = PixelFormat::I4 as u32;
const A4: u32 = PixelFormat::A4 as u32;
const ETC1: u32 = PixelFormat::ETC1 as u32;
const ETC1A4: u32 = PixelFormat::ETC1A4 as u32;
const D16: u32 = PixelFormat::D16 as u32;
const D24: u32 = PixelFormat::D24 as u32;
const D24S8: u32 = PixelFormat::D24S8 as u32;

const fn get_format_bpp(format: u32) -> u32 {
    FORMAT_MAP[format as usize].bits_per_block
}

const fn get_format_bytes_per_pixel(format: u32) -> u32 {
    FORMAT_MAP[format as usize].bytes_per_pixel
}

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

fn make_u32(bytes: &[u8]) -> u32 {
    u32::from_ne_bytes(bytes[..4].try_into().expect("the slice has four bytes"))
}

fn write_vec4(dest: &mut [u8], color: Vec4<u8>) {
    dest[..4].copy_from_slice(&[color.x, color.y, color.z, color.w]);
}

fn read_vec4(source: &[u8]) -> Vec4<u8> {
    Vec4 {
        x: source[0],
        y: source[1],
        z: source[2],
        w: source[3],
    }
}

fn decode_pixel<const FORMAT: u32, const CONVERTED: bool>(source: &[u8], dest: &mut [u8]) {
    let bytes_per_pixel = (get_format_bpp(FORMAT) / 8) as usize;

    if FORMAT == RGBA8 && CONVERTED {
        write_vec4(dest, decode_rgba8(source));
    } else if FORMAT == RGB8 && CONVERTED {
        write_vec4(dest, decode_rgb8(source));
    } else if FORMAT == RGB565 && CONVERTED {
        write_vec4(dest, decode_rgb565(source));
    } else if FORMAT == RGB5A1 && CONVERTED {
        write_vec4(dest, decode_rgb5a1(source));
    } else if FORMAT == RGBA4 && CONVERTED {
        write_vec4(dest, decode_rgba4(source));
    } else if FORMAT == IA8 {
        write_vec4(dest, decode_ia8(source));
    } else if FORMAT == RG8 {
        write_vec4(dest, decode_rg8(source));
    } else if FORMAT == I8 {
        write_vec4(dest, decode_i8(source));
    } else if FORMAT == A8 {
        write_vec4(dest, decode_a8(source));
    } else if FORMAT == IA4 {
        write_vec4(dest, decode_ia4(source));
    } else if FORMAT == D24 && CONVERTED {
        let d32 = decode_d24(source) as f32 / 16777215.0;
        dest[..4].copy_from_slice(&d32.to_ne_bytes());
    } else if FORMAT == D24S8 {
        let d24s8 = make_u32(source).rotate_left(8);
        dest[..4].copy_from_slice(&d24s8.to_ne_bytes());
    } else {
        dest[..bytes_per_pixel].copy_from_slice(&source[..bytes_per_pixel]);
    }
}

fn decode_pixel4<const FORMAT: u32>(x: u32, y: u32, source_tile: &[u8], dest_pixel: &mut [u8]) {
    let morton_offset = morton_interleave(x, y);
    let value = source_tile[(morton_offset >> 1) as usize];
    let pixel = convert_4_to_8(if !morton_offset.is_multiple_of(2) {
        value >> 4
    } else {
        value & 0xF
    });

    if FORMAT == I4 {
        dest_pixel[..3].fill(pixel);
        dest_pixel[3] = 255;
    } else {
        dest_pixel[..3].fill(0);
        dest_pixel[3] = pixel;
    }
}

fn decode_pixel_etc1<const FORMAT: u32>(
    mut x: u32,
    mut y: u32,
    source_tile: &[u8],
    dest_pixel: &mut [u8],
) {
    const SUBTILE_WIDTH: u32 = 4;
    const SUBTILE_HEIGHT: u32 = 4;
    let has_alpha = FORMAT == ETC1A4;
    let subtile_size: usize = if has_alpha { 16 } else { 8 };

    let subtile_index = (x / SUBTILE_WIDTH) + 2 * (y / SUBTILE_HEIGHT);
    x %= SUBTILE_WIDTH;
    y %= SUBTILE_HEIGHT;

    let mut subtile_ptr = subtile_index as usize * subtile_size;

    let mut alpha = 255u8;
    if has_alpha {
        let packed_alpha = u64::from_le_bytes(
            source_tile[subtile_ptr..subtile_ptr + 8]
                .try_into()
                .expect("the slice has eight bytes"),
        );
        subtile_ptr += size_of::<u64>();

        alpha = convert_4_to_8(((packed_alpha >> (4 * (x * SUBTILE_WIDTH + y))) & 0xF) as u8);
    }

    let subtile_data = u64::from_le_bytes(
        source_tile[subtile_ptr..subtile_ptr + 8]
            .try_into()
            .expect("the slice has eight bytes"),
    );
    let rgb = sample_etc1_subtile(subtile_data, x, y);

    // Copy the uncompressed pixel to the destination
    dest_pixel[..3].copy_from_slice(&[rgb.x, rgb.y, rgb.z]);
    dest_pixel[3] = alpha;
}

fn encode_pixel<const FORMAT: u32, const CONVERTED: bool>(source: &[u8], dest: &mut [u8]) {
    let bytes_per_pixel = (get_format_bpp(FORMAT) / 8) as usize;

    if FORMAT == RGBA8 && CONVERTED {
        encode_rgba8(&read_vec4(source), dest);
    } else if FORMAT == RGB8 && CONVERTED {
        encode_rgb8(&read_vec4(source), dest);
    } else if FORMAT == RGB565 && CONVERTED {
        encode_rgb565(&read_vec4(source), dest);
    } else if FORMAT == RGB5A1 && CONVERTED {
        encode_rgb5a1(&read_vec4(source), dest);
    } else if FORMAT == RGBA4 && CONVERTED {
        encode_rgba4(&read_vec4(source), dest);
    } else if FORMAT == IA8 {
        encode_ia8(&read_vec4(source), dest);
    } else if FORMAT == RG8 {
        encode_rg8(&read_vec4(source), dest);
    } else if FORMAT == I8 {
        encode_i8(&read_vec4(source), dest);
    } else if FORMAT == A8 {
        encode_a8(&read_vec4(source), dest);
    } else if FORMAT == IA4 {
        encode_ia4(&read_vec4(source), dest);
    } else if FORMAT == D24 && CONVERTED {
        let d32 = f32::from_ne_bytes(source[..4].try_into().expect("the slice has four bytes"));
        encode_d24((d32 * 0xFFFFFF as f32) as u32, dest);
    } else if FORMAT == D24S8 {
        let s8d24 = make_u32(source).rotate_right(8);
        dest[..4].copy_from_slice(&s8d24.to_ne_bytes());
    } else {
        dest[..bytes_per_pixel].copy_from_slice(&source[..bytes_per_pixel]);
    }
}

fn encode_pixel4<const FORMAT: u32>(
    x: u32,
    y: u32,
    source_pixel: &[u8],
    dest_tile_buffer: &mut [u8],
) {
    let rgba = read_vec4(source_pixel);

    let pixel = if FORMAT == I4 {
        average_rgb_components(&rgba)
    } else {
        rgba.w
    };

    let morton_offset = morton_interleave(x, y);
    let byte_offset = (morton_offset >> 1) as usize;

    let current_values = dest_tile_buffer[byte_offset];
    let new_value = convert_8_to_4(pixel);

    if !morton_offset.is_multiple_of(2) {
        dest_tile_buffer[byte_offset] = (new_value << 4) | (current_values & 0x0F);
    } else {
        dest_tile_buffer[byte_offset] = (current_values & 0xF0) | new_value;
    }
}

fn morton_copy_tile<const MORTON_TO_LINEAR: bool, const FORMAT: u32, const CONVERTED: bool>(
    stride: u32,
    tile_buffer: &mut [u8],
    linear_buffer: &mut [u8],
) {
    let bytes_per_pixel = get_format_bpp(FORMAT) / 8;
    let linear_bytes_per_pixel = if CONVERTED {
        4
    } else {
        get_format_bytes_per_pixel(FORMAT)
    };
    let is_compressed = FORMAT == ETC1 || FORMAT == ETC1A4;
    let is_4bit = FORMAT == I4 || FORMAT == A4;

    for y in 0..8 {
        for x in 0..8 {
            let tiled_offset = (morton_interleave(x, y) * bytes_per_pixel) as usize;
            let tiled_end = tiled_offset + bytes_per_pixel as usize;
            let linear_offset = (((7 - y) * stride + x) * linear_bytes_per_pixel) as usize;
            let linear_pixel =
                &mut linear_buffer[linear_offset..linear_offset + linear_bytes_per_pixel as usize];
            if MORTON_TO_LINEAR {
                if is_compressed {
                    decode_pixel_etc1::<FORMAT>(x, y, tile_buffer, linear_pixel);
                } else if is_4bit {
                    decode_pixel4::<FORMAT>(x, y, tile_buffer, linear_pixel);
                } else {
                    decode_pixel::<FORMAT, CONVERTED>(
                        &tile_buffer[tiled_offset..tiled_end],
                        linear_pixel,
                    );
                }
            } else if is_4bit {
                encode_pixel4::<FORMAT>(x, y, linear_pixel, tile_buffer);
            } else {
                encode_pixel::<FORMAT, CONVERTED>(
                    linear_pixel,
                    &mut tile_buffer[tiled_offset..tiled_end],
                );
            }
        }
    }
}

/**
 * @brief Performs morton to/from linear convertions on the provided pixel data
 * @param converted If true performs RGBA8 to/from convertion to all color formats
 * @param width, height The dimentions of the rectangular region of pixels in linear_buffer
 * @param start_offset The number of bytes from the start of the first tile to the start of
 * tiled_buffer
 * @param end_offset The number of bytes from the start of the first tile to the end of tiled_buffer
 * @param linear_buffer The linear pixel data
 * @param tiled_buffer The tiled pixel data
 *
 * The MortonCopy is at the heart of the PICA texture implementation, as it's responsible for
 * converting between linear and morton tiled layouts. The function handles both convertions but
 * there are slightly different paths and inputs for each:
 *
 * Morton to Linear:
 * During uploads, tiled_buffer is always aligned to the tile or scanline boundary depending if the
 * linear rectangle spans multiple vertical tiles. linear_buffer does not reference the entire
 * texture area, but rather the specific rectangle affected by the upload.
 *
 * Linear to Morton:
 * This is similar to the other convertion but with some differences. In this case tiled_buffer is
 * not required to be aligned to any specific boundary which requires special care.
 * start_offset/end_offset are useful here as they tell us exactly where the data should be placed
 * in the linear_buffer.
 */
fn morton_copy<const MORTON_TO_LINEAR: bool, const FORMAT: u32, const CONVERTED: bool>(
    width: u32,
    height: u32,
    start_offset: u32,
    end_offset: u32,
    linear_buffer: &mut [u8],
    tiled_buffer: &mut [u8],
) {
    let bytes_per_pixel = get_format_bpp(FORMAT) / 8;
    let aligned_bytes_per_pixel = if CONVERTED {
        4
    } else {
        get_format_bytes_per_pixel(FORMAT)
    };
    let tile_size = get_format_bpp(FORMAT) * 64 / 8;
    debug_assert!(aligned_bytes_per_pixel >= bytes_per_pixel);

    let linear_tile_stride = ((7 * width + 8) * aligned_bytes_per_pixel) as usize;
    let aligned_down_start_offset = align_down(start_offset, tile_size);
    let aligned_start_offset = align_up(start_offset, tile_size);
    let aligned_end_offset = align_down(end_offset, tile_size);
    let begin_pixel_index = aligned_down_start_offset * 8 / get_format_bpp(FORMAT);

    assert!(
        !MORTON_TO_LINEAR
            || (aligned_start_offset == start_offset && aligned_end_offset == end_offset),
        "Assertion Failed!"
    );

    // In OpenGL the texture origin is in the bottom left corner as opposed to other
    // APIs that have it at the top left. To avoid flipping texture coordinates in
    // the shader we read/write the linear buffer from the bottom up
    let mut x = (begin_pixel_index % (width * 8)) / 8;
    let mut y = (begin_pixel_index / (width * 8)) * 8;
    let mut linear_offset = (height.wrapping_sub(8).wrapping_sub(y))
        .wrapping_mul(width)
        .wrapping_add(x)
        .wrapping_mul(aligned_bytes_per_pixel);
    let mut tiled_offset = 0u32;

    let linear_next_tile = |x: &mut u32, y: &mut u32, linear_offset: &mut u32| {
        *x = (*x + 8) % width;
        *linear_offset = linear_offset.wrapping_add(8 * aligned_bytes_per_pixel);
        if *x == 0 {
            *y = (*y + 8) % height;
            if *y == 0 {
                return;
            }

            *linear_offset = linear_offset.wrapping_sub(width * 9 * aligned_bytes_per_pixel);
        }
    };

    let mut tmp_buf = [0u8; 4 * 64];
    let tmp_buf = &mut tmp_buf[..tile_size as usize];

    // If during a texture download the start coordinate is not tile aligned, swizzle
    // the tile affected to a temporary buffer and copy the part we are interested in
    if start_offset < aligned_start_offset && !MORTON_TO_LINEAR {
        let linear_data =
            &mut linear_buffer[linear_offset as usize..linear_offset as usize + linear_tile_stride];
        morton_copy_tile::<MORTON_TO_LINEAR, FORMAT, CONVERTED>(width, tmp_buf, linear_data);

        let copy_start = (start_offset - aligned_down_start_offset) as usize;
        let copy_size = (aligned_start_offset.min(end_offset) - start_offset) as usize;
        tiled_buffer[..copy_size].copy_from_slice(&tmp_buf[copy_start..copy_start + copy_size]);

        tiled_offset += aligned_start_offset - start_offset;
        linear_next_tile(&mut x, &mut y, &mut linear_offset);
    }

    // If the copy spans multiple tiles, copy the fully aligned tiles in between.
    if aligned_start_offset < aligned_end_offset {
        let tile_buffer_size = tiled_buffer.len() as u32;
        let buffer_end =
            (tiled_offset + aligned_end_offset - aligned_start_offset).min(tile_buffer_size);
        while tiled_offset < buffer_end {
            let linear_data = &mut linear_buffer
                [linear_offset as usize..linear_offset as usize + linear_tile_stride];
            let tiled_data =
                &mut tiled_buffer[tiled_offset as usize..(tiled_offset + tile_size) as usize];
            morton_copy_tile::<MORTON_TO_LINEAR, FORMAT, CONVERTED>(width, tiled_data, linear_data);
            tiled_offset += tile_size;
            linear_next_tile(&mut x, &mut y, &mut linear_offset);
        }
    }

    // If during a texture download the end coordinate is not tile aligned, swizzle
    // the tile affected to a temporary buffer and copy the part we are interested in
    if end_offset > aligned_start_offset.max(aligned_end_offset) && !MORTON_TO_LINEAR {
        let linear_data =
            &mut linear_buffer[linear_offset as usize..linear_offset as usize + linear_tile_stride];
        morton_copy_tile::<MORTON_TO_LINEAR, FORMAT, CONVERTED>(width, tmp_buf, linear_data);
        let copy_size = (end_offset - aligned_end_offset) as usize;
        tiled_buffer[tiled_offset as usize..tiled_offset as usize + copy_size]
            .copy_from_slice(&tmp_buf[..copy_size]);
    }
}

/**
 * Performs a linear copy, converting pixel formats if required.
 * @tparam decode If true, decodes the texture if needed. Otherwise, encodes if needed.
 * @tparam format Pixel format to copy.
 * @tparam converted If true, converts the texture to/from the appropriate format.
 * @param src_buffer The source pixel data
 * @param dst_buffer The destination pixel data
 * @return
 */
fn linear_copy<const DECODE: bool, const FORMAT: u32, const CONVERTED: bool>(
    src_buffer: &mut [u8],
    dst_buffer: &mut [u8],
) {
    let mut src_size = src_buffer.len();
    let mut dst_size = dst_buffer.len();

    if CONVERTED {
        let encoded_bytes_per_pixel = (get_format_bpp(FORMAT) / 8) as usize;
        let decoded_bytes_per_pixel = 4;
        let src_bytes_per_pixel = if DECODE {
            encoded_bytes_per_pixel
        } else {
            decoded_bytes_per_pixel
        };
        let dst_bytes_per_pixel = if DECODE {
            decoded_bytes_per_pixel
        } else {
            encoded_bytes_per_pixel
        };

        src_size -= src_size % src_bytes_per_pixel;
        dst_size -= dst_size % dst_bytes_per_pixel;

        let mut src_index = 0;
        let mut dst_index = 0;
        while src_index < src_size && dst_index < dst_size {
            let src_pixel = &src_buffer[src_index..src_index + src_bytes_per_pixel];
            let dst_pixel = &mut dst_buffer[dst_index..dst_index + dst_bytes_per_pixel];
            if DECODE {
                decode_pixel::<FORMAT, CONVERTED>(src_pixel, dst_pixel);
            } else {
                encode_pixel::<FORMAT, CONVERTED>(src_pixel, dst_pixel);
            }
            src_index += src_bytes_per_pixel;
            dst_index += dst_bytes_per_pixel;
        }
    } else {
        let size = src_size.min(dst_size);
        dst_buffer[..size].copy_from_slice(&src_buffer[..size]);
    }
}

pub type MortonFunc = fn(u32, u32, u32, u32, &mut [u8], &mut [u8]);

pub static UNSWIZZLE_TABLE: [Option<MortonFunc>; 18] = [
    Some(morton_copy::<true, RGBA8, false>),  // 0
    Some(morton_copy::<true, RGB8, false>),   // 1
    Some(morton_copy::<true, RGB5A1, false>), // 2
    Some(morton_copy::<true, RGB565, false>), // 3
    Some(morton_copy::<true, RGBA4, false>),  // 4
    Some(morton_copy::<true, IA8, false>),    // 5
    Some(morton_copy::<true, RG8, false>),    // 6
    Some(morton_copy::<true, I8, false>),     // 7
    Some(morton_copy::<true, A8, false>),     // 8
    Some(morton_copy::<true, IA4, false>),    // 9
    Some(morton_copy::<true, I4, false>),     // 10
    Some(morton_copy::<true, A4, false>),     // 11
    Some(morton_copy::<true, ETC1, false>),   // 12
    Some(morton_copy::<true, ETC1A4, false>), // 13
    Some(morton_copy::<true, D16, false>),    // 14
    None,                                     // 15
    Some(morton_copy::<true, D24, false>),    // 16
    Some(morton_copy::<true, D24S8, false>),  // 17
];

pub static UNSWIZZLE_TABLE_CONVERTED: [Option<MortonFunc>; 18] = [
    Some(morton_copy::<true, RGBA8, true>),  // 0
    Some(morton_copy::<true, RGB8, true>),   // 1
    Some(morton_copy::<true, RGB5A1, true>), // 2
    Some(morton_copy::<true, RGB565, true>), // 3
    Some(morton_copy::<true, RGBA4, true>),  // 4
    // The following formats are implicitly converted to RGBA regardless, so ignore them.
    None,                                 // 5
    None,                                 // 6
    None,                                 // 7
    None,                                 // 8
    None,                                 // 9
    None,                                 // 10
    None,                                 // 11
    None,                                 // 12
    None,                                 // 13
    Some(morton_copy::<true, D16, true>), // 14
    None,                                 // 15
    Some(morton_copy::<true, D24, true>), // 16
    // No conversion here as we need to do a special deinterleaving conversion elsewhere.
    None, // 17
];

pub static SWIZZLE_TABLE: [Option<MortonFunc>; 18] = [
    Some(morton_copy::<false, RGBA8, false>),  // 0
    Some(morton_copy::<false, RGB8, false>),   // 1
    Some(morton_copy::<false, RGB5A1, false>), // 2
    Some(morton_copy::<false, RGB565, false>), // 3
    Some(morton_copy::<false, RGBA4, false>),  // 4
    Some(morton_copy::<false, IA8, false>),    // 5
    Some(morton_copy::<false, RG8, false>),    // 6
    Some(morton_copy::<false, I8, false>),     // 7
    Some(morton_copy::<false, A8, false>),     // 8
    Some(morton_copy::<false, IA4, false>),    // 9
    Some(morton_copy::<false, I4, false>),     // 10
    Some(morton_copy::<false, A4, false>),     // 11
    None,                                      // 12
    None,                                      // 13
    Some(morton_copy::<false, D16, false>),    // 14
    None,                                      // 15
    Some(morton_copy::<false, D24, false>),    // 16
    Some(morton_copy::<false, D24S8, false>),  // 17
];

pub static SWIZZLE_TABLE_CONVERTED: [Option<MortonFunc>; 18] = [
    Some(morton_copy::<false, RGBA8, true>),  // 0
    Some(morton_copy::<false, RGB8, true>),   // 1
    Some(morton_copy::<false, RGB5A1, true>), // 2
    Some(morton_copy::<false, RGB565, true>), // 3
    Some(morton_copy::<false, RGBA4, true>),  // 4
    // The following formats are implicitly converted from RGBA regardless, so ignore them.
    None,                                  // 5
    None,                                  // 6
    None,                                  // 7
    None,                                  // 8
    None,                                  // 9
    None,                                  // 10
    None,                                  // 11
    None,                                  // 12
    None,                                  // 13
    Some(morton_copy::<false, D16, true>), // 14
    None,                                  // 15
    Some(morton_copy::<false, D24, true>), // 16
    // No conversion here as we need to do a special interleaving conversion elsewhere.
    None, // 17
];

pub type LinearFunc = fn(&mut [u8], &mut [u8]);

pub static LINEAR_DECODE_TABLE: [Option<LinearFunc>; 18] = [
    Some(linear_copy::<true, RGBA8, false>),  // 0
    Some(linear_copy::<true, RGB8, false>),   // 1
    Some(linear_copy::<true, RGB5A1, false>), // 2
    Some(linear_copy::<true, RGB565, false>), // 3
    Some(linear_copy::<true, RGBA4, false>),  // 4
    // These formats cannot be used linearly and can be ignored.
    None,                                    // 5
    None,                                    // 6
    None,                                    // 7
    None,                                    // 8
    None,                                    // 9
    None,                                    // 10
    None,                                    // 11
    None,                                    // 12
    None,                                    // 13
    Some(linear_copy::<true, D16, false>),   // 14
    None,                                    // 15
    Some(linear_copy::<true, D24, false>),   // 16
    Some(linear_copy::<true, D24S8, false>), // 17
];

pub static LINEAR_DECODE_TABLE_CONVERTED: [Option<LinearFunc>; 18] = [
    Some(linear_copy::<true, RGBA8, true>),  // 0
    Some(linear_copy::<true, RGB8, true>),   // 1
    Some(linear_copy::<true, RGB5A1, true>), // 2
    Some(linear_copy::<true, RGB565, true>), // 3
    Some(linear_copy::<true, RGBA4, true>),  // 4
    // These formats cannot be used linearly and can be ignored.
    None,                                 // 5
    None,                                 // 6
    None,                                 // 7
    None,                                 // 8
    None,                                 // 9
    None,                                 // 10
    None,                                 // 11
    None,                                 // 12
    None,                                 // 13
    Some(linear_copy::<true, D16, true>), // 14
    None,                                 // 15
    Some(linear_copy::<true, D24, true>), // 16
    // No conversion here as we need to do a special deinterleaving conversion elsewhere.
    None, // 17
];

pub static LINEAR_ENCODE_TABLE: [Option<LinearFunc>; 18] = [
    Some(linear_copy::<false, RGBA8, false>),  // 0
    Some(linear_copy::<false, RGB8, false>),   // 1
    Some(linear_copy::<false, RGB5A1, false>), // 2
    Some(linear_copy::<false, RGB565, false>), // 3
    Some(linear_copy::<false, RGBA4, false>),  // 4
    // These formats cannot be used linearly and can be ignored.
    None,                                     // 5
    None,                                     // 6
    None,                                     // 7
    None,                                     // 8
    None,                                     // 9
    None,                                     // 10
    None,                                     // 11
    None,                                     // 12
    None,                                     // 13
    Some(linear_copy::<false, D16, false>),   // 14
    None,                                     // 15
    Some(linear_copy::<false, D24, false>),   // 16
    Some(linear_copy::<false, D24S8, false>), // 17
];

pub static LINEAR_ENCODE_TABLE_CONVERTED: [Option<LinearFunc>; 18] = [
    Some(linear_copy::<false, RGBA8, true>),  // 0
    Some(linear_copy::<false, RGB8, true>),   // 1
    Some(linear_copy::<false, RGB5A1, true>), // 2
    Some(linear_copy::<false, RGB565, true>), // 3
    Some(linear_copy::<false, RGBA4, true>),  // 4
    // These formats cannot be used linearly and can be ignored.
    None,                                  // 5
    None,                                  // 6
    None,                                  // 7
    None,                                  // 8
    None,                                  // 9
    None,                                  // 10
    None,                                  // 11
    None,                                  // 12
    None,                                  // 13
    Some(linear_copy::<false, D16, true>), // 14
    None,                                  // 15
    Some(linear_copy::<false, D24, true>), // 16
    // No conversion here as we need to do a special interleaving conversion elsewhere.
    None, // 17
];
