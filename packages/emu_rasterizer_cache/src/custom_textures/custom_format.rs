#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CustomPixelFormat {
    RGBA8 = 0,
    BC1 = 1,
    BC3 = 2,
    BC5 = 3,
    BC7 = 4,
    ASTC4 = 5,
    ASTC6 = 6,
    ASTC8 = 7,
    #[default]
    Invalid = u32::MAX,
}

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CustomFileFormat {
    #[default]
    None = 0,
    PNG = 1,
    DDS = 2,
    KTX = 3,
}

pub fn custom_pixel_format_as_string(format: CustomPixelFormat) -> &'static str {
    match format {
        CustomPixelFormat::RGBA8 => "RGBA8",
        CustomPixelFormat::BC1 => "BC1",
        CustomPixelFormat::BC3 => "BC3",
        CustomPixelFormat::BC5 => "BC5",
        CustomPixelFormat::BC7 => "BC7",
        CustomPixelFormat::ASTC4 => "ASTC4",
        CustomPixelFormat::ASTC6 => "ASTC6",
        CustomPixelFormat::ASTC8 => "ASTC8",
        _ => "NotReal",
    }
}

pub fn is_custom_format_compressed(format: CustomPixelFormat) -> bool {
    format != CustomPixelFormat::RGBA8 && format != CustomPixelFormat::Invalid
}
