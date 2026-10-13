// 8x8 Z-Order coordinate from 2D coordinates
pub const fn morton_interleave(x: u32, y: u32) -> u32 {
    const XLUT: [u32; 8] = [0x00, 0x01, 0x04, 0x05, 0x10, 0x11, 0x14, 0x15];
    const YLUT: [u32; 8] = [0x00, 0x02, 0x08, 0x0a, 0x20, 0x22, 0x28, 0x2a];
    XLUT[(x % 8) as usize] + YLUT[(y % 8) as usize]
}

/**
 * Calculates the offset of the position of the pixel in Morton order
 */
pub const fn get_morton_offset(x: u32, y: u32, bytes_per_pixel: u32) -> u32 {
    // Images are split into 8x8 tiles. Each tile is composed of four 4x4 subtiles each
    // of which is composed of four 2x2 subtiles each of which is composed of four texels.
    // Each structure is embedded into the next-bigger one in a diagonal pattern, e.g.
    // texels are laid out in a 2x2 subtile like this:
    // 2 3
    // 0 1
    //
    // The full 8x8 tile has the texels arranged like this:
    //
    // 42 43 46 47 58 59 62 63
    // 40 41 44 45 56 57 60 61
    // 34 35 38 39 50 51 54 55
    // 32 33 36 37 48 49 52 53
    // 10 11 14 15 26 27 30 31
    // 08 09 12 13 24 25 28 29
    // 02 03 06 07 18 19 22 23
    // 00 01 04 05 16 17 20 21
    //
    // This pattern is what's called Z-order curve, or Morton order.

    let block_height = 8;
    let coarse_x = x & !7;

    let i = morton_interleave(x, y);

    let offset = coarse_x.wrapping_mul(block_height);

    i.wrapping_add(offset).wrapping_mul(bytes_per_pixel)
}
