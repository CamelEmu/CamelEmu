use serde_json::{Value, json};

use crate::error::Result;
use crate::scene::{Scene, SceneImage};
use crate::settings::ExportSettings;

struct BinaryBuilder<'a> {
    settings: &'a ExportSettings,
    data: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl<'a> BinaryBuilder<'a> {
    fn new(settings: &'a ExportSettings) -> Self {
        Self {
            settings,
            data: Vec::new(),
            views: Vec::new(),
            accessors: Vec::new(),
        }
    }

    fn push_view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        pad(&mut self.data, self.settings.chunk_alignment, 0);
        let mut view = json!({
            "buffer": 0,
            "byteOffset": self.data.len(),
            "byteLength": bytes.len(),
        });
        if let Some(target) = target {
            view["target"] = json!(target);
        }
        self.data.extend_from_slice(bytes);
        self.views.push(view);
        self.views.len() - 1
    }

    fn push_accessor(&mut self, view: usize, component_type: u32, count: usize, kind: &str) -> usize {
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": component_type,
            "count": count,
            "type": kind,
        }));
        self.accessors.len() - 1
    }
}

impl Scene {
    pub fn to_glb(&self, settings: &ExportSettings) -> Result<Vec<u8>> {
        let mut binary = BinaryBuilder::new(settings);

        let mut images = Vec::new();
        let mut samplers = Vec::new();
        let mut textures = Vec::new();
        let mut materials = Vec::new();
        for (index, image) in self.images.iter().enumerate() {
            let png = image.encode_png(index)?;
            let view = binary.push_view(&png, None);
            images.push(json!({ "bufferView": view, "mimeType": settings.png_mime_type }));
            samplers.push(sampler(image, settings));
            textures.push(json!({ "sampler": index, "source": index }));
            materials.push(json!({
                "name": format!("{}{index}", settings.material_name_prefix),
                "pbrMetallicRoughness": {
                    "baseColorTexture": { "index": index },
                    "metallicFactor": 0.0,
                },
                "alphaMode": "BLEND",
                "doubleSided": true,
            }));
        }
        let default_material = materials.len();
        materials.push(json!({
            "name": settings.material_name_prefix,
            "pbrMetallicRoughness": { "metallicFactor": 0.0 },
            "doubleSided": true,
        }));

        let mut meshes = Vec::new();
        for (index, mesh) in self.meshes.iter().enumerate() {
            if mesh.indices.is_empty() || mesh.positions.is_empty() {
                continue;
            }
            let position_bytes: Vec<u8> = mesh
                .positions
                .iter()
                .flatten()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            let view = binary.push_view(&position_bytes, Some(settings.target_array_buffer));
            let position = binary.push_accessor(
                view,
                settings.component_float,
                mesh.positions.len(),
                "VEC3",
            );
            let (min, max) = bounds(&mesh.positions);
            binary.accessors[position]["min"] = json!(min);
            binary.accessors[position]["max"] = json!(max);

            let mut attributes = json!({ "POSITION": position });
            if mesh.texcoords.len() == mesh.positions.len() {
                let bytes: Vec<u8> = mesh
                    .texcoords
                    .iter()
                    .flatten()
                    .flat_map(|value| value.to_le_bytes())
                    .collect();
                let view = binary.push_view(&bytes, Some(settings.target_array_buffer));
                attributes["TEXCOORD_0"] = json!(binary.push_accessor(
                    view,
                    settings.component_float,
                    mesh.texcoords.len(),
                    "VEC2",
                ));
            }

            let index_bytes: Vec<u8> = mesh
                .indices
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect();
            let view = binary.push_view(&index_bytes, Some(settings.target_element_array_buffer));
            let indices = binary.push_accessor(
                view,
                settings.component_unsigned_int,
                mesh.indices.len(),
                "SCALAR",
            );

            meshes.push(json!({
                "name": format!("{}{index}", settings.mesh_name_prefix),
                "primitives": [{
                    "attributes": attributes,
                    "indices": indices,
                    "material": mesh.texture.unwrap_or(default_material),
                    "mode": settings.mode_triangles,
                }],
            }));
        }

        let mut nodes: Vec<Value> = (0..meshes.len()).map(|mesh| json!({ "mesh": mesh })).collect();
        nodes.push(json!({
            "name": "camera",
            "camera": 0,
            "translation": self.camera.position,
            "rotation": self.camera.rotation,
        }));

        let mut root = json!({
            "asset": { "version": "2.0", "generator": settings.generator },
            "scene": 0,
            "scenes": [{ "nodes": (0..nodes.len()).collect::<Vec<_>>() }],
            "nodes": nodes,
            "cameras": [{
                "type": "perspective",
                "perspective": {
                    "yfov": self.camera.yfov,
                    "aspectRatio": self.camera.aspect,
                    "znear": self.camera.near,
                    "zfar": self.camera.far,
                },
            }],
            "meshes": meshes,
            "materials": materials,
            "accessors": binary.accessors,
            "bufferViews": binary.views,
            "buffers": [{ "byteLength": binary.data.len() }],
        });
        if !images.is_empty() {
            root["images"] = json!(images);
            root["samplers"] = json!(samplers);
            root["textures"] = json!(textures);
        }

        let mut json_chunk = root.to_string().into_bytes();
        pad(&mut json_chunk, settings.chunk_alignment, settings.json_padding);
        let mut bin_chunk = binary.data;
        pad(&mut bin_chunk, settings.chunk_alignment, 0);

        let header_len = 12;
        let chunk_header_len = 8;
        let total = header_len + 2 * chunk_header_len + json_chunk.len() + bin_chunk.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(&settings.glb_magic.to_le_bytes());
        out.extend_from_slice(&settings.glb_version.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        out.extend_from_slice(&settings.json_chunk_type.to_le_bytes());
        out.extend_from_slice(&json_chunk);
        out.extend_from_slice(&(bin_chunk.len() as u32).to_le_bytes());
        out.extend_from_slice(&settings.bin_chunk_type.to_le_bytes());
        out.extend_from_slice(&bin_chunk);
        Ok(out)
    }
}

fn sampler(image: &SceneImage, settings: &ExportSettings) -> Value {
    let filter = |linear: bool| {
        if linear {
            settings.filter_linear
        } else {
            settings.filter_nearest
        }
    };
    let wrap = |mode: u8| {
        if mode == settings.pica_wrap_repeat {
            settings.wrap_repeat
        } else if mode == settings.pica_wrap_mirrored_repeat {
            settings.wrap_mirrored_repeat
        } else {
            settings.wrap_clamp_to_edge
        }
    };
    json!({
        "magFilter": filter(image.magnify_linear),
        "minFilter": filter(image.minify_linear),
        "wrapS": wrap(image.wrap_s),
        "wrapT": wrap(image.wrap_t),
    })
}

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    positions.iter().fold(
        ([f32::MAX; 3], [f32::MIN; 3]),
        |(mut min, mut max), position| {
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
            (min, max)
        },
    )
}

fn pad(data: &mut Vec<u8>, alignment: usize, fill: u8) {
    while data.len() % alignment != 0 {
        data.push(fill);
    }
}
