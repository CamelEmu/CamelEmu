use crate::bits::{bits, pica_enum, sbits};

pica_enum!(TextureType {
    TEXTURE_2D = 0,
    TEXTURE_CUBE = 1,
    SHADOW_2D = 2,
    PROJECTION_2D = 3,
    SHADOW_CUBE = 4,
    DISABLED = 5,
});

pica_enum!(WrapMode {
    CLAMP_TO_EDGE = 0,
    CLAMP_TO_BORDER = 1,
    REPEAT = 2,
    MIRRORED_REPEAT = 3,
    // Mode 4-7 produces some weird result and may be just invalid:
    CLAMP_TO_EDGE2 = 4,   // Positive coord: clamp to edge; negative coord: repeat
    CLAMP_TO_BORDER2 = 5, // Positive coord: clamp to border; negative coord: repeat
    REPEAT2 = 6,          // Same as Repeat
    REPEAT3 = 7,          // Same as Repeat
});

pica_enum!(TextureFilter {
    NEAREST = 0,
    LINEAR = 1,
});

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TextureConfig {
    pub border_color: u32,
    pub size: u32,
    pub params: u32,
    pub lod: u32,
    pub address_raw: u32,
    // texture1 and texture2 store the texture format directly after the address
    // whereas texture0 inserts some additional flags inbetween.
    // Hence, we store the format separately so that all other parameters can be described
    // in a single structure.
}

impl TextureConfig {
    pub fn border_color_r(&self) -> u32 {
        bits(self.border_color, 0, 8)
    }

    pub fn border_color_g(&self) -> u32 {
        bits(self.border_color, 8, 8)
    }

    pub fn border_color_b(&self) -> u32 {
        bits(self.border_color, 16, 8)
    }

    pub fn border_color_a(&self) -> u32 {
        bits(self.border_color, 24, 8)
    }

    pub fn height(&self) -> u32 {
        bits(self.size, 0, 11)
    }

    pub fn width(&self) -> u32 {
        bits(self.size, 16, 11)
    }

    pub fn mag_filter(&self) -> TextureFilter {
        TextureFilter(bits(self.params, 1, 1))
    }

    pub fn min_filter(&self) -> TextureFilter {
        TextureFilter(bits(self.params, 2, 1))
    }

    pub fn wrap_t(&self) -> WrapMode {
        WrapMode(bits(self.params, 8, 3))
    }

    pub fn wrap_s(&self) -> WrapMode {
        WrapMode(bits(self.params, 12, 3))
    }

    pub fn mip_filter(&self) -> TextureFilter {
        TextureFilter(bits(self.params, 24, 1))
    }

    /// @note Only valid for texture 0 according to 3DBrew.
    pub fn texture_type(&self) -> TextureType {
        TextureType(bits(self.params, 28, 3))
    }

    pub fn lod_bias(&self) -> i32 {
        sbits(self.lod, 0, 13) // fixed1.4.8
    }

    pub fn lod_max_level(&self) -> u32 {
        bits(self.lod, 16, 4)
    }

    pub fn lod_min_level(&self) -> u32 {
        bits(self.lod, 24, 4)
    }

    pub fn address(&self) -> u32 {
        bits(self.address_raw, 0, 28)
    }

    pub fn get_physical_address(&self) -> u32 {
        self.address().wrapping_mul(8)
    }
}

pica_enum!(TextureFormat {
    RGBA8 = 0,
    RGB8 = 1,
    RGB5A1 = 2,
    RGB565 = 3,
    RGBA4 = 4,
    IA8 = 5,
    RG8 = 6, // @note Also called HILO8 in 3DBrew.
    I8 = 7,
    A8 = 8,
    IA4 = 9,
    I4 = 10,
    A4 = 11,
    ETC1 = 12,   // compressed
    ETC1A4 = 13, // compressed
});

impl TextureFormat {
    pub fn nibbles_per_pixel(self) -> u32 {
        match self {
            Self::RGBA8 => 8,
            Self::RGB8 => 6,
            Self::RGB5A1 | Self::RGB565 | Self::RGBA4 | Self::IA8 | Self::RG8 => 4,
            Self::I4 | Self::A4 => 1,
            Self::I8 | Self::A8 | Self::IA4 => 2,
            // placeholder for yet unknown formats
            _ => {
                log::error!(target: "Debug", "Unimplemented code!");
                0
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CubeFace {
    PositiveX = 0,
    NegativeX = 1,
    PositiveY = 2,
    NegativeY = 3,
    PositiveZ = 4,
    NegativeZ = 5,
}

#[derive(Clone, Copy, Debug)]
pub struct FullTextureConfig {
    pub enabled: u32,
    pub config: TextureConfig,
    pub format: TextureFormat,
}
