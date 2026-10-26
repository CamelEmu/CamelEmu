use crate::context::{AlphaMode, ClipVertex, DrawData, TextureData, TriangleMode, Viewport};
use crate::progress::Progress;
use crate::scene::SceneImage;
use crate::settings::ViewSettings;
use crate::view::Frustum;

#[derive(Debug, Clone, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub texture: Option<usize>,
    pub screen_width: u32,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
}

enum Attribute<'a> {
    Texture(&'a TextureData),
    Color,
    Patch,
}

struct Placement {
    flip_vertically: bool,
    viewport: Viewport,
    perspective: Option<Frustum>,
}

impl Placement {
    fn locate(&self, position: [f32; 4], screen_frustum: &Frustum) -> [f32; 3] {
        let [x, y, _, w] = position;
        if let Some(frustum) = &self.perspective {
            let right = if self.flip_vertically { y } else { -y };
            return [right * frustum.half_width, x * frustum.half_height, -w];
        }
        let viewport = &self.viewport;
        let raster_x = x * viewport.half_x + w * (viewport.half_x + viewport.corner_x);
        let raster_y = y * viewport.half_y + w * (viewport.half_y + viewport.corner_y);
        let up = 2.0 * raster_x / viewport.width - w;
        let right = if self.flip_vertically {
            2.0 * raster_y / viewport.height - w
        } else {
            w - 2.0 * raster_y / viewport.height
        };
        [
            right * screen_frustum.half_width,
            up * screen_frustum.half_height,
            -w,
        ]
    }
}

impl MeshData {
    pub fn split(
        draw: &DrawData,
        frustum: &Frustum,
        perspective: Option<Frustum>,
        texture: Option<&TextureData>,
        images: &mut Vec<SceneImage>,
        settings: &ViewSettings,
        progress: &mut Progress,
    ) -> Vec<MeshData> {
        let placement = Placement {
            flip_vertically: draw.flip_vertically,
            viewport: draw.viewport,
            perspective,
        };
        let textured = draw.texture.zip(texture);
        let mut texture_mesh = Self::empty(draw, textured.map(|(index, _)| index));
        let mut color_mesh = Self::empty(draw, None);
        let mut patch_meshes = Vec::new();
        let mut patches = draw.patches.iter();

        for (index, triangle) in draw.vertices.chunks_exact(3).enumerate() {
            progress.advance(1);
            let mut corners = [triangle[0], triangle[1], triangle[2]];
            match draw.modes.get(index).copied().unwrap_or(TriangleMode::Color) {
                TriangleMode::Texture => {
                    if let Some((_, texture)) = textured {
                        texture_mesh.push(
                            clip_w(corners, settings),
                            frustum,
                            &placement,
                            &Attribute::Texture(texture),
                        );
                    }
                }
                TriangleMode::Color => {
                    color_mesh.push(
                        clip_w(corners, settings),
                        frustum,
                        &placement,
                        &Attribute::Color,
                    );
                }
                TriangleMode::Patch => {
                    let Some(patch) = patches.next() else {
                        continue;
                    };
                    for (corner, coordinate) in
                        corners.iter_mut().zip([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])
                    {
                        corner.texcoord = coordinate;
                    }
                    let mut mesh = Self::empty(draw, Some(images.len()));
                    images.push(SceneImage::from(patch));
                    mesh.push(
                        clip_w(corners, settings),
                        frustum,
                        &placement,
                        &Attribute::Patch,
                    );
                    patch_meshes.push(mesh);
                }
            }
        }

        let mut meshes = Vec::new();
        for mesh in [texture_mesh, color_mesh] {
            if !mesh.positions.is_empty() {
                meshes.push(mesh);
            }
        }
        meshes.extend(patch_meshes);
        meshes
    }

    pub fn scale_xy(&mut self, factor: f32) {
        for position in &mut self.positions {
            position[0] *= factor;
            position[1] *= factor;
        }
    }

    pub fn translate(&mut self, offset: [f32; 3]) {
        for position in &mut self.positions {
            for (axis, value) in position.iter_mut().enumerate() {
                *value += offset[axis];
            }
        }
    }

    fn empty(draw: &DrawData, texture: Option<usize>) -> Self {
        Self {
            positions: Vec::new(),
            texcoords: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            texture,
            screen_width: draw.screen_width,
            alpha_mode: draw.alpha_mode,
            alpha_cutoff: draw.alpha_cutoff,
        }
    }

    fn push(
        &mut self,
        polygon: Vec<ClipVertex>,
        frustum: &Frustum,
        placement: &Placement,
        attribute: &Attribute,
    ) {
        for corner in 1..polygon.len().saturating_sub(1) {
            for vertex in [polygon[0], polygon[corner], polygon[corner + 1]] {
                self.indices.push(self.positions.len() as u32);
                self.positions
                    .push(placement.locate(vertex.position, frustum));
                match attribute {
                    Attribute::Texture(texture) => {
                        self.texcoords.push(texture.local_uv(vertex.texcoord))
                    }
                    Attribute::Color => self.colors.push(vertex.color),
                    Attribute::Patch => self.texcoords.push(vertex.texcoord),
                }
            }
        }
    }
}

impl TextureData {
    pub fn local_uv(&self, texcoord: [f32; 2]) -> [f32; 2] {
        [
            (texcoord[0] * self.source_width as f32 - self.origin_s as f32) / self.width as f32,
            (self.top_t as f32 - texcoord[1] * self.source_height as f32) / self.height as f32,
        ]
    }
}

impl From<&MeshData> for Vec<u8> {
    fn from(mesh: &MeshData) -> Self {
        let counts = [mesh.positions.len(), mesh.texcoords.len(), mesh.indices.len()];
        let mut out = Vec::new();
        for count in counts {
            out.extend_from_slice(&(count as u32).to_le_bytes());
        }
        for value in mesh.positions.iter().flatten() {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for value in mesh.texcoords.iter().flatten() {
            out.extend_from_slice(&value.to_le_bytes());
        }
        for index in &mesh.indices {
            out.extend_from_slice(&index.to_le_bytes());
        }
        out
    }
}

fn clip_w(triangle: [ClipVertex; 3], settings: &ViewSettings) -> Vec<ClipVertex> {
    let inside = |vertex: &ClipVertex| {
        vertex.position[3] + settings.clip_w_bias >= -settings.clip_tolerance
    };
    let intersection = |v0: &ClipVertex, v1: &ClipVertex| {
        let dp = v0.position[3] + settings.clip_w_bias;
        let dp_prev = v1.position[3] + settings.clip_w_bias;
        let factor = dp_prev / (dp_prev - dp);
        let mix = |a: f32, b: f32| a * factor + b * (1.0 - factor);
        ClipVertex {
            position: std::array::from_fn(|i| mix(v0.position[i], v1.position[i])),
            texcoord: std::array::from_fn(|i| mix(v0.texcoord[i], v1.texcoord[i])),
            color: std::array::from_fn(|i| mix(v0.color[i], v1.color[i])),
        }
    };

    let mut polygon = Vec::with_capacity(4);
    let mut reference = triangle[2];
    for vertex in triangle {
        if inside(&vertex) {
            if !inside(&reference) {
                polygon.push(intersection(&vertex, &reference));
            }
            polygon.push(vertex);
        } else if inside(&reference) {
            polygon.push(intersection(&vertex, &reference));
        }
        reference = vertex;
    }
    polygon
}
