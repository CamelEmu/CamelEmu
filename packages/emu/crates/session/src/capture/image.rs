use crate::error::{AzaharError, SessionResult};
use crate::ffi::RawScreenshot;

use super::settings::CaptureSettings;

pub struct ScreenImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl ScreenImage {
    pub fn encode_png(&self) -> SessionResult<Vec<u8>> {
        let failed = |error: png::EncodingError| AzaharError::Capture(error.to_string());
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(failed)?;
        writer.write_image_data(&self.rgba).map_err(failed)?;
        writer.finish().map_err(failed)?;
        Ok(out)
    }
}

impl From<RawScreenshot<'_>> for ScreenImage {
    fn from(raw: RawScreenshot<'_>) -> Self {
        let settings = CaptureSettings::default();
        let row = raw.width as usize * settings.bytes_per_pixel;
        let mut rgba = Vec::with_capacity(raw.bgra.len());
        for line in raw.bgra.chunks_exact(row).rev() {
            for pixel in line.chunks_exact(settings.bytes_per_pixel) {
                rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], u8::MAX]);
            }
        }
        Self {
            width: raw.width,
            height: raw.height,
            rgba,
        }
    }
}

impl From<(ScreenImage, ScreenImage)> for ScreenImage {
    fn from((top, bottom): (ScreenImage, ScreenImage)) -> Self {
        let settings = CaptureSettings::default();
        let width = top.width.max(bottom.width);
        let height = top.height + bottom.height;
        let mut rgba = settings
            .background
            .repeat(width as usize * height as usize);
        let mut y = 0;
        for image in [&top, &bottom] {
            let left = (width - image.width) as usize / 2;
            let row = image.width as usize * settings.bytes_per_pixel;
            for line in image.rgba.chunks_exact(row) {
                let start = (y * width as usize + left) * settings.bytes_per_pixel;
                rgba[start..start + row].copy_from_slice(line);
                y += 1;
            }
        }
        Self {
            width,
            height,
            rgba,
        }
    }
}
