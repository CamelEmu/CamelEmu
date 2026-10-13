use crate::{Vec2, Vec4};

/// Convert a 1-bit color component to 8 bit
pub const fn convert_1_to_8(value: u8) -> u8 {
    value.wrapping_mul(255)
}

/// Convert a 4-bit color component to 8 bit
pub const fn convert_4_to_8(value: u8) -> u8 {
    (value << 4) | value
}

/// Convert a 5-bit color component to 8 bit
pub const fn convert_5_to_8(value: u8) -> u8 {
    (value << 3) | (value >> 2)
}

/// Convert a 6-bit color component to 8 bit
pub const fn convert_6_to_8(value: u8) -> u8 {
    (value << 2) | (value >> 4)
}

/// Convert a 8-bit color component to 1 bit
pub const fn convert_8_to_1(value: u8) -> u8 {
    value >> 7
}

/// Convert a 8-bit color component to 4 bit
pub const fn convert_8_to_4(value: u8) -> u8 {
    value >> 4
}

/// Convert a 8-bit color component to 5 bit
pub const fn convert_8_to_5(value: u8) -> u8 {
    value >> 3
}

/// Convert a 8-bit color component to 6 bit
pub const fn convert_8_to_6(value: u8) -> u8 {
    value >> 2
}

/// Averages the RGB components of a color
pub const fn average_rgb_components(color: &Vec4<u8>) -> u8 {
    ((color.x as u32 + color.y as u32 + color.z as u32) / 3) as u8
}

fn read_u16_le(bytes: &[u8]) -> u16 {
    u16::from_le_bytes([bytes[0], bytes[1]])
}

/**
 * Decode a color stored in RGBA8 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_rgba8(bytes: &[u8]) -> Vec4<u8> {
    Vec4 {
        x: bytes[3],
        y: bytes[2],
        z: bytes[1],
        w: bytes[0],
    }
}

/**
 * Decode a color stored in RGB8 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_rgb8(bytes: &[u8]) -> Vec4<u8> {
    Vec4 {
        x: bytes[2],
        y: bytes[1],
        z: bytes[0],
        w: 255,
    }
}

/**
 * Decode a color stored in RG8 (aka HILO8) format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_rg8(bytes: &[u8]) -> Vec4<u8> {
    Vec4 {
        x: bytes[1],
        y: bytes[0],
        z: 0,
        w: 255,
    }
}

/**
 * Decode a color stored in RGB565 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_rgb565(bytes: &[u8]) -> Vec4<u8> {
    let pixel = read_u16_le(bytes);
    Vec4 {
        x: convert_5_to_8(((pixel >> 11) & 0x1F) as u8),
        y: convert_6_to_8(((pixel >> 5) & 0x3F) as u8),
        z: convert_5_to_8((pixel & 0x1F) as u8),
        w: 255,
    }
}

/**
 * Decode a color stored in RGB5A1 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_rgb5a1(bytes: &[u8]) -> Vec4<u8> {
    let pixel = read_u16_le(bytes);
    Vec4 {
        x: convert_5_to_8(((pixel >> 11) & 0x1F) as u8),
        y: convert_5_to_8(((pixel >> 6) & 0x1F) as u8),
        z: convert_5_to_8(((pixel >> 1) & 0x1F) as u8),
        w: convert_1_to_8((pixel & 0x1) as u8),
    }
}

/**
 * Decode a color stored in RGBA4 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_rgba4(bytes: &[u8]) -> Vec4<u8> {
    let pixel = read_u16_le(bytes);
    Vec4 {
        x: convert_4_to_8(((pixel >> 12) & 0xF) as u8),
        y: convert_4_to_8(((pixel >> 8) & 0xF) as u8),
        z: convert_4_to_8(((pixel >> 4) & 0xF) as u8),
        w: convert_4_to_8((pixel & 0xF) as u8),
    }
}

/**
 * Decode a color stored in IA8 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_ia8(bytes: &[u8]) -> Vec4<u8> {
    Vec4 {
        x: bytes[1],
        y: bytes[1],
        z: bytes[1],
        w: bytes[0],
    }
}

/**
 * Decode a color stored in I8 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_i8(bytes: &[u8]) -> Vec4<u8> {
    Vec4 {
        x: bytes[0],
        y: bytes[0],
        z: bytes[0],
        w: 255,
    }
}

/**
 * Decode a color stored in A8 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_a8(bytes: &[u8]) -> Vec4<u8> {
    Vec4 {
        x: 0,
        y: 0,
        z: 0,
        w: bytes[0],
    }
}

/**
 * Decode a color stored in IA4 format
 * @param bytes Pointer to encoded source color
 * @return Result color decoded as Common::Vec4<u8>
 */
pub fn decode_ia4(bytes: &[u8]) -> Vec4<u8> {
    let i = convert_4_to_8((bytes[0] & 0xF0) >> 4);
    let a = convert_4_to_8(bytes[0] & 0x0F);
    Vec4 {
        x: i,
        y: i,
        z: i,
        w: a,
    }
}

/**
 * Decode a depth value stored in D16 format
 * @param bytes Pointer to encoded source value
 * @return Depth value as an u32
 */
pub fn decode_d16(bytes: &[u8]) -> u32 {
    u32::from(read_u16_le(bytes))
}

/**
 * Decode a depth value stored in D24 format
 * @param bytes Pointer to encoded source value
 * @return Depth value as an u32
 */
pub fn decode_d24(bytes: &[u8]) -> u32 {
    (u32::from(bytes[2]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[0])
}

/**
 * Decode a depth value and a stencil value stored in D24S8 format
 * @param bytes Pointer to encoded source values
 * @return Resulting values stored as a Common::Vec2
 */
pub fn decode_d24s8(bytes: &[u8]) -> Vec2<u32> {
    Vec2 {
        x: (u32::from(bytes[2]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[0]),
        y: u32::from(bytes[3]),
    }
}

/**
 * Encode a color as RGBA8 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_rgba8(color: &Vec4<u8>, bytes: &mut [u8]) {
    bytes[3] = color.x;
    bytes[2] = color.y;
    bytes[1] = color.z;
    bytes[0] = color.w;
}

/**
 * Encode a color as RGB8 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_rgb8(color: &Vec4<u8>, bytes: &mut [u8]) {
    bytes[2] = color.x;
    bytes[1] = color.y;
    bytes[0] = color.z;
}

/**
 * Encode a color as RG8 (aka HILO8) format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_rg8(color: &Vec4<u8>, bytes: &mut [u8]) {
    bytes[1] = color.x;
    bytes[0] = color.y;
}

/**
 * Encode a color as RGB565 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_rgb565(color: &Vec4<u8>, bytes: &mut [u8]) {
    let data: u16 = (u16::from(convert_8_to_5(color.x)) << 11)
        | (u16::from(convert_8_to_6(color.y)) << 5)
        | u16::from(convert_8_to_5(color.z));

    bytes[..2].copy_from_slice(&data.to_le_bytes());
}

/**
 * Encode a color as RGB5A1 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_rgb5a1(color: &Vec4<u8>, bytes: &mut [u8]) {
    let data: u16 = (u16::from(convert_8_to_5(color.x)) << 11)
        | (u16::from(convert_8_to_5(color.y)) << 6)
        | (u16::from(convert_8_to_5(color.z)) << 1)
        | u16::from(convert_8_to_1(color.w));

    bytes[..2].copy_from_slice(&data.to_le_bytes());
}

/**
 * Encode a color as RGBA4 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_rgba4(color: &Vec4<u8>, bytes: &mut [u8]) {
    let data: u16 = (u16::from(convert_8_to_4(color.x)) << 12)
        | (u16::from(convert_8_to_4(color.y)) << 8)
        | (u16::from(convert_8_to_4(color.z)) << 4)
        | u16::from(convert_8_to_4(color.w));

    bytes[..2].copy_from_slice(&data.to_ne_bytes());
}

/**
 * Encode a color as IA8 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_ia8(color: &Vec4<u8>, bytes: &mut [u8]) {
    bytes[1] = average_rgb_components(color);
    bytes[0] = color.w;
}

/**
 * Encode a color as I8 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_i8(color: &Vec4<u8>, bytes: &mut [u8]) {
    bytes[0] = average_rgb_components(color);
}

/**
 * Encode a color as A8 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_a8(color: &Vec4<u8>, bytes: &mut [u8]) {
    bytes[0] = color.w;
}

/**
 * Encode a color as IA4 format
 * @param color Source color to encode
 * @param bytes Destination pointer to store encoded color
 */
pub fn encode_ia4(color: &Vec4<u8>, bytes: &mut [u8]) {
    bytes[0] = (convert_8_to_4(average_rgb_components(color)) << 4) | convert_8_to_4(color.w);
}

/**
 * Encode a 16 bit depth value as D16 format
 * @param value 16 bit source depth value to encode
 * @param bytes Pointer where to store the encoded value
 */
pub fn encode_d16(value: u32, bytes: &mut [u8]) {
    let data = value as u16;
    bytes[..2].copy_from_slice(&data.to_le_bytes());
}

/**
 * Encode a 24 bit depth value as D24 format
 * @param value 24 bit source depth value to encode
 * @param bytes Pointer where to store the encoded value
 */
pub fn encode_d24(value: u32, bytes: &mut [u8]) {
    bytes[0] = (value & 0xFF) as u8;
    bytes[1] = ((value >> 8) & 0xFF) as u8;
    bytes[2] = ((value >> 16) & 0xFF) as u8;
}

/**
 * Encode a 24 bit depth and 8 bit stencil values as D24S8 format
 * @param depth 24 bit source depth value to encode
 * @param stencil 8 bit source stencil value to encode
 * @param bytes Pointer where to store the encoded value
 */
pub fn encode_d24s8(depth: u32, stencil: u8, bytes: &mut [u8]) {
    bytes[0] = (depth & 0xFF) as u8;
    bytes[1] = ((depth >> 8) & 0xFF) as u8;
    bytes[2] = ((depth >> 16) & 0xFF) as u8;
    bytes[3] = stencil;
}

/**
 * Encode a 24 bit depth value as D24X8 format (32 bits per pixel with 8 bits unused)
 * @param depth 24 bit source depth value to encode
 * @param bytes Pointer where to store the encoded value
 * @note unused bits will not be modified
 */
pub fn encode_d24x8(depth: u32, bytes: &mut [u8]) {
    bytes[0] = (depth & 0xFF) as u8;
    bytes[1] = ((depth >> 8) & 0xFF) as u8;
    bytes[2] = ((depth >> 16) & 0xFF) as u8;
}

/**
 * Encode an 8 bit stencil value as X24S8 format (32 bits per pixel with 24 bits unused)
 * @param stencil 8 bit source stencil value to encode
 * @param bytes Pointer where to store the encoded value
 * @note unused bits will not be modified
 */
pub fn encode_x24s8(stencil: u8, bytes: &mut [u8]) {
    bytes[3] = stencil;
}
