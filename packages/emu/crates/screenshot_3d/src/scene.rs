use crate::camera::CameraData;
use crate::context::{Screenshot3dContext, TextureData};
use crate::error::{Result, Screenshot3dError};
use crate::mesh::MeshData;
use crate::settings::ExportSettings;
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

impl From<&Screenshot3dContext> for Scene {
    fn from(context: &Screenshot3dContext) -> Self {
        let frustum = Frustum::from_camera(&context.camera);
        Self {
            meshes: context
                .vertices
                .draws
                .iter()
                .enumerate()
                .map(|(layer, draw)| {
                    let texture = draw.texture.and_then(|index| context.textures.get(index));
                    MeshData::from((draw, layer, frustum, texture))
                })
                .collect(),
            camera: context.camera.into(),
            images: context.textures.iter().map(SceneImage::from).collect(),
        }
    }
}

impl From<(Scene, Scene)> for Scene {
    fn from((top, mut bottom): (Scene, Scene)) -> Self {
        let gap = ExportSettings::default().stack_gap;
        if let (Some((top_min, top_max)), Some((bottom_min, bottom_max))) =
            (top.bounds(), bottom.bounds())
        {
            let offset = [
                (top_min[0] + top_max[0] - bottom_min[0] - bottom_max[0]) / 2.0,
                top_min[1] - gap - bottom_max[1],
                0.0,
            ];
            for mesh in &mut bottom.meshes {
                mesh.translate(offset);
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
