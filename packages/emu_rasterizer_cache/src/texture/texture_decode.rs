use emu_pica_types::Vec4;
use emu_pica_types::color::{
    convert_4_to_8, decode_rg8, decode_rgb5a1, decode_rgb8, decode_rgb565, decode_rgba4,
    decode_rgba8,
};
use emu_pica_types::regs::texturing::{TextureConfig, TextureFormat};
use emu_pica_types::utils::morton_interleave;

use crate::texture::etc1::sample_etc1_subtile;

const TILE_SIZE: usize = 8 * 8;
const ETC1_SUBTILES: usize = 2 * 2;

/// Returns the byte size of a 8*8 tile of the specified texture format.
pub fn calculate_tile_size(format: TextureFormat) -> usize {
    match format {
        TextureFormat::RGBA8 => 4 * TILE_SIZE,

        TextureFormat::RGB8 => 3 * TILE_SIZE,

        TextureFormat::RGB5A1
        | TextureFormat::RGB565
        | TextureFormat::RGBA4
        | TextureFormat::IA8
        | TextureFormat::RG8 => 2 * TILE_SIZE,

        TextureFormat::I8 | TextureFormat::A8 | TextureFormat::IA4 => TILE_SIZE,

        TextureFormat::I4 | TextureFormat::A4 => TILE_SIZE / 2,

        TextureFormat::ETC1 => ETC1_SUBTILES * 8,

        TextureFormat::ETC1A4 => ETC1_SUBTILES * 16,

        // placeholder for yet unknown formats
        _ => {
            log::error!(target: "Debug", "Unimplemented code!");
            0
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureInfo {
    pub physical_address: u32,
    pub width: u32,
    pub height: u32,
    pub stride: isize,
    pub format: TextureFormat,
}

impl TextureInfo {
    pub fn from_pica_register(config: &TextureConfig, format: TextureFormat) -> Self {
        let mut info = TextureInfo {
            physical_address: config.get_physical_address(),
            width: config.width(),
            height: config.height(),
            stride: 0,
            format,
        };
        info.set_default_stride();
        info
    }

    /// Calculates stride from format and width, assuming that the entire texture is contiguous.
    pub fn set_default_stride(&mut self) {
        self.stride = (calculate_tile_size(self.format) * (self.width as usize / 8)) as isize;
    }
}

/**
 * Lookup texel located at the given coordinates and return an RGBA vector of its color.
 * @param source Source pointer to read data from
 * @param x,y Texture coordinates to read from
 * @param info TextureInfo object describing the texture setup
 * @param disable_alpha This is used for debug widgets which use this method to display textures
 * without providing a good way to visualize alpha by themselves. If true, this will return 255 for
 * the alpha component, and either drop the information entirely or store it in an "unused" color
 * channel.
 * @todo Eventually we should get rid of the disable_alpha parameter.
 */
pub fn lookup_texture(
    source: *const u8,
    x: u32,
    y: u32,
    info: &TextureInfo,
    disable_alpha: bool,
) -> Vec4<u8> {
    // Coordinate in tiles
    let coarse_x = x / 8;
    let coarse_y = y / 8;

    // Coordinate inside the tile
    let fine_x = x % 8;
    let fine_y = y % 8;

    let line = source.wrapping_offset(coarse_y as isize * info.stride);
    let tile = line.wrapping_add(coarse_x as usize * calculate_tile_size(info.format));
    lookup_texel_in_tile(tile, fine_x, fine_y, info, disable_alpha)
}

fn bytes<'a>(source: *const u8, length: usize) -> &'a [u8] {
    unsafe { std::slice::from_raw_parts(source, length) }
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Vec4<u8> {
    Vec4 {
        x: r,
        y: g,
        z: b,
        w: a,
    }
}

/**
 * Looks up a texel from a single 8x8 texture tile.
 *
 * @param source Pointer to the beginning of the tile.
 * @param x, y In-tile coordinates to read from. Must be < 8.
 * @param info TextureInfo describing the texture format.
 * @param disable_alpha Used for debugging. Sets the result alpha to 255 and either discards the
 *                      real alpha or inserts it in an otherwise unused channel.
 */
pub fn lookup_texel_in_tile(
    source: *const u8,
    mut x: u32,
    mut y: u32,
    info: &TextureInfo,
    disable_alpha: bool,
) -> Vec4<u8> {
    debug_assert!(x < 8);
    debug_assert!(y < 8);

    let morton = morton_interleave(x, y) as usize;
    match info.format {
        TextureFormat::RGBA8 => {
            let res = decode_rgba8(bytes(source.wrapping_add(morton * 4), 4));
            rgba(res.x, res.y, res.z, if disable_alpha { 255 } else { res.w })
        }

        TextureFormat::RGB8 => {
            let res = decode_rgb8(bytes(source.wrapping_add(morton * 3), 3));
            rgba(res.x, res.y, res.z, 255)
        }

        TextureFormat::RGB5A1 => {
            let res = decode_rgb5a1(bytes(source.wrapping_add(morton * 2), 2));
            rgba(res.x, res.y, res.z, if disable_alpha { 255 } else { res.w })
        }

        TextureFormat::RGB565 => {
            let res = decode_rgb565(bytes(source.wrapping_add(morton * 2), 2));
            rgba(res.x, res.y, res.z, 255)
        }

        TextureFormat::RGBA4 => {
            let res = decode_rgba4(bytes(source.wrapping_add(morton * 2), 2));
            rgba(res.x, res.y, res.z, if disable_alpha { 255 } else { res.w })
        }

        TextureFormat::IA8 => {
            let source_ptr = bytes(source.wrapping_add(morton * 2), 2);

            if disable_alpha {
                // Show intensity as red, alpha as green
                rgba(source_ptr[1], source_ptr[0], 0, 255)
            } else {
                rgba(source_ptr[1], source_ptr[1], source_ptr[1], source_ptr[0])
            }
        }

        TextureFormat::RG8 => {
            let res = decode_rg8(bytes(source.wrapping_add(morton * 2), 2));
            rgba(res.x, res.y, 0, 255)
        }

        TextureFormat::I8 => {
            let source_ptr = bytes(source.wrapping_add(morton), 1)[0];
            rgba(source_ptr, source_ptr, source_ptr, 255)
        }

        TextureFormat::A8 => {
            let source_ptr = bytes(source.wrapping_add(morton), 1)[0];

            if disable_alpha {
                rgba(source_ptr, source_ptr, source_ptr, 255)
            } else {
                rgba(0, 0, 0, source_ptr)
            }
        }

        TextureFormat::IA4 => {
            let source_ptr = bytes(source.wrapping_add(morton), 1)[0];

            let i = convert_4_to_8((source_ptr & 0xF0) >> 4);
            let a = convert_4_to_8(source_ptr & 0xF);

            if disable_alpha {
                // Show intensity as red, alpha as green
                rgba(i, a, 0, 255)
            } else {
                rgba(i, i, i, a)
            }
        }

        TextureFormat::I4 => {
            let source_ptr = bytes(source.wrapping_add(morton / 2), 1)[0];

            let i = if !morton.is_multiple_of(2) {
                (source_ptr & 0xF0) >> 4
            } else {
                source_ptr & 0xF
            };
            let i = convert_4_to_8(i);

            rgba(i, i, i, 255)
        }

        TextureFormat::A4 => {
            let source_ptr = bytes(source.wrapping_add(morton / 2), 1)[0];

            let a = if !morton.is_multiple_of(2) {
                (source_ptr & 0xF0) >> 4
            } else {
                source_ptr & 0xF
            };
            let a = convert_4_to_8(a);

            if disable_alpha {
                rgba(a, a, a, 255)
            } else {
                rgba(0, 0, 0, a)
            }
        }

        TextureFormat::ETC1 | TextureFormat::ETC1A4 => {
            let has_alpha = info.format == TextureFormat::ETC1A4;
            let subtile_size = if has_alpha { 16 } else { 8 };

            // ETC1 further subdivides each 8x8 tile into four 4x4 subtiles
            const SUBTILE_WIDTH: u32 = 4;
            const SUBTILE_HEIGHT: u32 = 4;

            let subtile_index = (x / SUBTILE_WIDTH) + 2 * (y / SUBTILE_HEIGHT);
            x %= SUBTILE_WIDTH;
            y %= SUBTILE_HEIGHT;

            let mut subtile_ptr = source.wrapping_add(subtile_index as usize * subtile_size);

            let mut alpha = 255u8;
            if has_alpha {
                let packed_alpha = u64::from_le_bytes(
                    bytes(subtile_ptr, 8)
                        .try_into()
                        .expect("the slice has eight bytes"),
                );
                subtile_ptr = subtile_ptr.wrapping_add(size_of::<u64>());

                alpha =
                    convert_4_to_8(((packed_alpha >> (4 * (x * SUBTILE_WIDTH + y))) & 0xF) as u8);
            }

            let subtile_data = u64::from_le_bytes(
                bytes(subtile_ptr, 8)
                    .try_into()
                    .expect("the slice has eight bytes"),
            );

            let rgb = sample_etc1_subtile(subtile_data, x, y);
            rgba(rgb.x, rgb.y, rgb.z, if disable_alpha { 255 } else { alpha })
        }

        _ => {
            log::error!(target: "HW_GPU", "Unknown texture format: {:x}", info.format.0);
            debug_assert!(false);
            Vec4::default()
        }
    }
}
