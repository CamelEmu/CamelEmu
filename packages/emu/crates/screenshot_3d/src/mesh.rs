use crate::context::{AlphaMode, ClipVertex, DrawData, TextureData};
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

impl MeshData {
    pub fn with_settings(
        draw: &DrawData,
        layer: usize,
        frustum: &Frustum,
        texture: Option<&TextureData>,
        settings: &ViewSettings,
    ) -> Self {
        let depth_scale = 1.0 + layer as f32 * settings.layer_offset;
        let horizontal = if draw.flip_vertically { 1.0 } else { -1.0 };
        let textured = draw.texture.zip(texture);

        let mut mesh = Self {
            positions: Vec::new(),
            texcoords: Vec::new(),
            colors: Vec::new(),
            indices: Vec::new(),
            texture: textured.map(|(index, _)| index),
            screen_width: draw.screen_width,
            alpha_mode: draw.alpha_mode,
            alpha_cutoff: draw.alpha_cutoff,
        };
        for triangle in draw.vertices.chunks_exact(3) {
            let polygon = clip_w([triangle[0], triangle[1], triangle[2]], settings);
            for corner in 1..polygon.len().saturating_sub(1) {
                for vertex in [polygon[0], polygon[corner], polygon[corner + 1]] {
                    let [x, y, _, w] = vertex.position;
                    mesh.indices.push(mesh.positions.len() as u32);
                    mesh.positions.push([
                        horizontal * y * frustum.half_width * depth_scale,
                        x * frustum.half_height * depth_scale,
                        -w * depth_scale,
                    ]);
                    match textured {
                        Some((_, texture)) => mesh.texcoords.push(texture.local_uv(vertex.texcoord)),
                        None => mesh.colors.push(vertex.color),
                    }
                }
            }
        }
        mesh
    }

    pub fn translate(&mut self, offset: [f32; 3]) {
        for position in &mut self.positions {
            for (axis, value) in position.iter_mut().enumerate() {
                *value += offset[axis];
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

impl From<(&DrawData, usize, Frustum, Option<&TextureData>)> for MeshData {
    fn from(
        (draw, layer, frustum, texture): (&DrawData, usize, Frustum, Option<&TextureData>),
    ) -> Self {
        Self::with_settings(draw, layer, &frustum, texture, &ViewSettings::default())
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
