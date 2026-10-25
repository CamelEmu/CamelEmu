use crate::smdh::LARGE_ICON_SIZE;
use crate::source::read_whole_file;

pub fn rgb565_to_rgba(pixels: &[u16]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for pixel in pixels {
        let r5 = ((pixel >> 11) & 0x1F) as u8;
        let g6 = ((pixel >> 5) & 0x3F) as u8;
        let b5 = (pixel & 0x1F) as u8;
        rgba.extend_from_slice(&[
            (r5 << 3) | (r5 >> 2),
            (g6 << 2) | (g6 >> 4),
            (b5 << 3) | (b5 >> 2),
            255,
        ]);
    }
    rgba
}

pub fn encode_png(rgba: &[u8], size: usize) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut encoder = png::Encoder::new(&mut buffer, size as u32, size as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(rgba).ok()?;
    writer.finish().ok()?;
    Some(buffer)
}

fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

pub fn icon_path(icons_directory: &str, game_path: &str) -> String {
    format!(
        "{}/{:016x}.png",
        icons_directory.trim_end_matches('/'),
        fnv1a(game_path)
    )
}

pub fn write_icon(icons_directory: &str, game_path: &str, icon: &[u16]) -> Option<String> {
    if icon.len() != LARGE_ICON_SIZE * LARGE_ICON_SIZE {
        return None;
    }
    let png = encode_png(&rgb565_to_rgba(icon), LARGE_ICON_SIZE)?;
    let path = icon_path(icons_directory, game_path);
    let stored = on_device(&path);
    if read_whole_file(&stored).as_deref() == Some(&png) {
        return Some(path);
    }
    if !emu_storage::create_dir_all(&on_device(icons_directory)) {
        return None;
    }
    emu_storage::write_file(&stored, &png).then_some(path)
}

pub fn on_device(path: &str) -> String {
    format!("{}{path}", emu_storage::FILE_SCHEME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_565_pixels_to_full_range_rgba() {
        assert_eq!(rgb565_to_rgba(&[0xFFFF]), [255, 255, 255, 255]);
        assert_eq!(rgb565_to_rgba(&[0x0000]), [0, 0, 0, 255]);
        assert_eq!(rgb565_to_rgba(&[0xF800]), [255, 0, 0, 255]);
        assert_eq!(rgb565_to_rgba(&[0x07E0]), [0, 255, 0, 255]);
        assert_eq!(rgb565_to_rgba(&[0x001F]), [0, 0, 255, 255]);
    }

    #[test]
    fn icon_file_names_are_stable_per_game_path() {
        assert_eq!(icon_path("/icons/", "a"), icon_path("/icons", "a"));
        assert_ne!(icon_path("/icons", "a"), icon_path("/icons", "b"));
    }
}
