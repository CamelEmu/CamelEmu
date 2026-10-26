use crate::camera::CameraData;
use crate::context::{AlphaMode, PatchData, Screenshot3dContext, TextureData};
use crate::error::{Result, Screenshot3dError};
use crate::mesh::MeshData;
use crate::settings::{ExportSettings, ViewSettings};
use crate::view::Frustum;

#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub meshes: Vec<MeshData>,
    pub camera: SceneCamera,
    pub images: Vec<SceneImage>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SceneCamera {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub yfov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Debug, Clone)]
pub struct SceneImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub wrap_s: u8,
    pub wrap_t: u8,
    pub magnify_linear: bool,
    pub minify_linear: bool,
}

impl SceneImage {
    pub fn encode_png(&self, index: usize) -> Result<Vec<u8>> {
        let invalid = || Screenshot3dError::InvalidTexture {
            index,
            width: self.width,
            height: self.height,
        };
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|_| invalid())?;
        writer.write_image_data(&self.rgba).map_err(|_| invalid())?;
        writer.finish().map_err(|_| invalid())?;
        Ok(out)
    }
}

impl Scene {
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut positions = self.meshes.iter().flat_map(|mesh| mesh.positions.iter());
        let first = *positions.next()?;
        Some(positions.fold((first, first), |(mut min, mut max), position| {
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
            (min, max)
        }))
    }
}

impl From<CameraData> for SceneCamera {
    fn from(camera: CameraData) -> Self {
        Self {
            position: camera.position,
            rotation: camera.rotation,
            yfov: camera.projection.yfov,
            aspect: camera.projection.aspect,
            near: camera.projection.near,
            far: camera.projection.far,
        }
    }
}

impl From<&TextureData> for SceneImage {
    fn from(texture: &TextureData) -> Self {
        Self {
            width: texture.width,
            height: texture.height,
            rgba: texture.rgba.clone(),
            wrap_s: texture.wrap_s,
            wrap_t: texture.wrap_t,
            magnify_linear: texture.magnify_linear,
            minify_linear: texture.minify_linear,
        }
    }
}

impl From<&PatchData> for SceneImage {
    fn from(patch: &PatchData) -> Self {
        let settings = ExportSettings::default();
        Self {
            width: patch.size,
            height: patch.size,
            rgba: patch.rgba.clone(),
            wrap_s: settings.pica_clamp_to_edge,
            wrap_t: settings.pica_clamp_to_edge,
            magnify_linear: false,
            minify_linear: false,
        }
    }
}

impl From<&Screenshot3dContext> for Scene {
    fn from(context: &Screenshot3dContext) -> Self {
        let settings = ViewSettings::default();
        let frustum = Frustum::from_camera(&context.camera);
        let mut images: Vec<SceneImage> = context.textures.iter().map(SceneImage::from).collect();
        let mut meshes = Vec::new();
        for draw in &context.vertices.draws {
            let texture = draw.texture.and_then(|index| context.textures.get(index));
            let perspective = Frustum::probe(draw, &context.shaders, &settings);
            meshes.extend(MeshData::split(
                draw,
                &frustum,
                perspective,
                texture,
                &mut images,
                &settings,
            ));
        }
        let mut scene = Self {
            meshes,
            camera: context.camera.into(),
            images,
        };
        scene.discard_opaque_alpha();
        scene
    }
}

impl Scene {
    fn discard_opaque_alpha(&mut self) {
        let mut translucent = vec![false; self.images.len()];
        for mesh in &self.meshes {
            if mesh.alpha_mode != AlphaMode::Opaque {
                if let Some(slot) = mesh.texture.and_then(|texture| translucent.get_mut(texture)) {
                    *slot = true;
                }
            }
        }
        for mesh in &mut self.meshes {
            if mesh.alpha_mode == AlphaMode::Opaque {
                for color in &mut mesh.colors {
                    color[3] = 1.0;
                }
            }
        }
        for (image, translucent) in self.images.iter_mut().zip(translucent) {
            if !translucent {
                for pixel in image.rgba.chunks_exact_mut(4) {
                    pixel[3] = u8::MAX;
                }
            }
        }
    }
}

impl From<(Scene, Scene)> for Scene {
    fn from((top, mut bottom): (Scene, Scene)) -> Self {
        let settings = ExportSettings::default();
        let top_half = (top.camera.yfov / 2.0).tan();
        let bottom_half = (bottom.camera.yfov / 2.0).tan();
        let scale = top_half / bottom_half;
        if scale.is_finite() && scale > 0.0 {
            let offset = -(2.0 * top_half * settings.screen_plane_depth + settings.stack_gap);
            for mesh in &mut bottom.meshes {
                mesh.scale_xy(scale);
                mesh.translate([0.0, offset, 0.0]);
            }
        }

        let image_offset = top.images.len();
        for mesh in &mut bottom.meshes {
            mesh.texture = mesh.texture.map(|texture| texture + image_offset);
        }
        let mut meshes = top.meshes;
        meshes.extend(bottom.meshes);
        let mut images = top.images;
        images.extend(bottom.images);
        Self {
            meshes,
            camera: top.camera,
            images,
        }
    }
}
