use emu_pica_types::regs::framebuffer::{ColorFormat, DepthFormat};
use emu_pica_types::regs::texturing::TextureFormat;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PixelFormat {
    RGBA8 = 0,
    RGB8 = 1,
    RGB5A1 = 2,
    RGB565 = 3,
    RGBA4 = 4,
    IA8 = 5,
    RG8 = 6,
    I8 = 7,
    A8 = 8,
    IA4 = 9,
    I4 = 10,
    A4 = 11,
    ETC1 = 12,
    ETC1A4 = 13,
    D16 = 14,
    D24 = 16,
    D24S8 = 17,
    MaxPixelFormat = 18,
    #[default]
    Invalid = u32::MAX,
}

pub const PIXEL_FORMAT_COUNT: usize = PixelFormat::MaxPixelFormat as usize;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SurfaceType {
    Color = 0,
    Texture = 1,
    Depth = 2,
    DepthStencil = 3,
    Fill = 4,
    #[default]
    Invalid = 5,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextureType {
    #[default]
    Texture2D = 0,
    CubeMap = 1,
}

#[derive(Clone, Copy, Debug)]
pub struct PixelFormatInfo {
    pub ty: SurfaceType,
    pub bits_per_block: u32,
    pub bytes_per_pixel: u32,
}

const fn info(ty: SurfaceType, bits_per_block: u32, bytes_per_pixel: u32) -> PixelFormatInfo {
    PixelFormatInfo {
        ty,
        bits_per_block,
        bytes_per_pixel,
    }
}

/**
 * Lookup table for querying pixel format properties (type, name, etc)
 * @note Modern GPUs require 4 byte alignment for D24
 * @note Texture formats are automatically converted to RGBA8
 **/
pub const FORMAT_MAP: [PixelFormatInfo; PIXEL_FORMAT_COUNT] = [
    info(SurfaceType::Color, 32, 4),
    info(SurfaceType::Color, 24, 3),
    info(SurfaceType::Color, 16, 2),
    info(SurfaceType::Color, 16, 2),
    info(SurfaceType::Color, 16, 2),
    info(SurfaceType::Texture, 16, 4),
    info(SurfaceType::Texture, 16, 4),
    info(SurfaceType::Texture, 8, 4),
    info(SurfaceType::Texture, 8, 4),
    info(SurfaceType::Texture, 8, 4),
    info(SurfaceType::Texture, 4, 4),
    info(SurfaceType::Texture, 4, 4),
    info(SurfaceType::Texture, 4, 4),
    info(SurfaceType::Texture, 8, 4),
    info(SurfaceType::Depth, 16, 2),
    info(SurfaceType::Invalid, 0, 0),
    info(SurfaceType::Depth, 24, 4),
    info(SurfaceType::DepthStencil, 32, 4),
];

pub const fn get_format_bpp(format: PixelFormat) -> u32 {
    let index = format as usize;
    assert!(index < FORMAT_MAP.len());
    FORMAT_MAP[index].bits_per_block
}

pub const fn get_format_bytes_per_pixel(format: PixelFormat) -> u32 {
    let index = format as usize;
    assert!(index < FORMAT_MAP.len());
    FORMAT_MAP[index].bytes_per_pixel
}

pub const fn get_format_type(format: PixelFormat) -> SurfaceType {
    let index = format as usize;
    assert!(index < FORMAT_MAP.len());
    FORMAT_MAP[index].ty
}

pub fn pixel_format_as_string(format: PixelFormat) -> &'static str {
    match format {
        PixelFormat::RGBA8 => "RGBA8",
        PixelFormat::RGB8 => "RGB8",
        PixelFormat::RGB5A1 => "RGB5A1",
        PixelFormat::RGB565 => "RGB565",
        PixelFormat::RGBA4 => "RGBA4",
        PixelFormat::IA8 => "IA8",
        PixelFormat::RG8 => "RG8",
        PixelFormat::I8 => "I8",
        PixelFormat::A8 => "A8",
        PixelFormat::IA4 => "IA4",
        PixelFormat::I4 => "I4",
        PixelFormat::A4 => "A4",
        PixelFormat::ETC1 => "ETC1",
        PixelFormat::ETC1A4 => "ETC1A4",
        PixelFormat::D16 => "D16",
        PixelFormat::D24 => "D24",
        PixelFormat::D24S8 => "D24S8",
        _ => "NotReal",
    }
}

pub fn check_formats_blittable(source_format: PixelFormat, dest_format: PixelFormat) -> bool {
    let source_type = get_format_type(source_format);
    let dest_type = get_format_type(dest_format);

    if (source_type == SurfaceType::Color || source_type == SurfaceType::Texture)
        && (dest_type == SurfaceType::Color || dest_type == SurfaceType::Texture)
    {
        return true;
    }

    if source_type == SurfaceType::Depth && dest_type == SurfaceType::Depth {
        return true;
    }

    if source_type == SurfaceType::DepthStencil && dest_type == SurfaceType::DepthStencil {
        return true;
    }

    log::warn!(
        target: "HW_GPU",
        "Unblittable format pair detected {} and {}",
        pixel_format_as_string(source_format),
        pixel_format_as_string(dest_format)
    );
    false
}

pub fn pixel_format_from_texture_format(format: TextureFormat) -> PixelFormat {
    match format {
        TextureFormat::RGBA8 => PixelFormat::RGBA8,
        TextureFormat::RGB8 => PixelFormat::RGB8,
        TextureFormat::RGB5A1 => PixelFormat::RGB5A1,
        TextureFormat::RGB565 => PixelFormat::RGB565,
        TextureFormat::RGBA4 => PixelFormat::RGBA4,
        TextureFormat::IA8 => PixelFormat::IA8,
        TextureFormat::RG8 => PixelFormat::RG8,
        TextureFormat::I8 => PixelFormat::I8,
        TextureFormat::A8 => PixelFormat::A8,
        TextureFormat::IA4 => PixelFormat::IA4,
        TextureFormat::I4 => PixelFormat::I4,
        TextureFormat::A4 => PixelFormat::A4,
        TextureFormat::ETC1 => PixelFormat::ETC1,
        TextureFormat::ETC1A4 => PixelFormat::ETC1A4,
        _ => PixelFormat::Invalid,
    }
}

pub fn pixel_format_from_color_format(format: ColorFormat) -> PixelFormat {
    match format {
        ColorFormat::RGBA8 => PixelFormat::RGBA8,
        ColorFormat::RGB8 => PixelFormat::RGB8,
        ColorFormat::RGB5A1 => PixelFormat::RGB5A1,
        ColorFormat::RGB565 => PixelFormat::RGB565,
        ColorFormat::RGBA4 => PixelFormat::RGBA4,
        _ => PixelFormat::Invalid,
    }
}

pub fn pixel_format_from_depth_format(format: DepthFormat) -> PixelFormat {
    match format {
        DepthFormat::D16 => PixelFormat::D16,
        DepthFormat::D24 => PixelFormat::D24,
        DepthFormat::D24S8 => PixelFormat::D24S8,
        _ => PixelFormat::Invalid,
    }
}

pub fn pixel_format_from_gpu_pixel_format(format: emu_pica_types::PixelFormat) -> PixelFormat {
    match format {
        emu_pica_types::PixelFormat::RGBA8 => PixelFormat::RGBA8,
        emu_pica_types::PixelFormat::RGB8 => PixelFormat::RGB8,
        emu_pica_types::PixelFormat::RGB565 => PixelFormat::RGB565,
        emu_pica_types::PixelFormat::RGB5A1 => PixelFormat::RGB5A1,
        emu_pica_types::PixelFormat::RGBA4 => PixelFormat::RGBA4,
        _ => PixelFormat::Invalid,
    }
}
