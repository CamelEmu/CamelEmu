use std::time::Duration;

pub struct CaptureSettings {
    pub timeout: Duration,
    pub timeout_3d: Duration,
    pub screenshot_prefix: &'static str,
    pub screenshot_3d_prefix: &'static str,
    pub png_extension: &'static str,
    pub png_mime_type: &'static str,
    pub glb_extension: &'static str,
    pub glb_mime_type: &'static str,
    pub bytes_per_pixel: usize,
    pub background: [u8; 4],
}

impl Default for CaptureSettings {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(5),
            timeout_3d: Duration::from_secs(120),
            screenshot_prefix: "screenshot_",
            screenshot_3d_prefix: "screenshot_3d_",
            png_extension: "png",
            png_mime_type: "image/png",
            glb_extension: "glb",
            glb_mime_type: "model/gltf-binary",
            bytes_per_pixel: 4,
            background: [0, 0, 0, 255],
        }
    }
}
