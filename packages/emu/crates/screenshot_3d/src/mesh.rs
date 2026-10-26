use crate::context::{
    AttributeFormat, DrawData, IndexFormat, Topology, VertexAttribute, VertexBuffer,
};
use crate::settings::MeshSettings;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub texture: Option<usize>,
    pub screen_width: u32,
}

impl MeshData {
    pub fn with_settings(draw: &DrawData, settings: &MeshSettings) -> Self {
        let position = draw
            .attributes
            .iter()
            .position(|attribute| attribute.components >= settings.position_min_components);
        let texcoord = position.and_then(|position| {
            draw.attributes
                .iter()
                .skip(position + 1)
                .position(|attribute| {
                    attribute.components == settings.texcoord_components
                        && attribute.format == AttributeFormat::Float
                })
                .map(|offset| position + 1 + offset)
        });

        let ids = vertex_ids(draw);
        let read = |attribute: Option<usize>, id: u32| {
            attribute
                .and_then(|attribute| draw.attributes.get(attribute))
                .and_then(|attribute| {
                    let buffer = draw.buffers.get(attribute.buffer)?;
                    read_attribute(buffer, attribute, id)
                })
                .unwrap_or_default()
        };

        let positions = ids
            .iter()
            .map(|&id| {
                let value = read(position, id);
                [value[0], value[1], value[2]]
            })
            .collect();
        let texcoords = texcoord
            .map(|texcoord| {
                ids.iter()
                    .map(|&id| {
                        let value = read(Some(texcoord), id);
                        [value[0], value[1]]
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self {
            positions,
            texcoords,
            indices: triangulate(draw.topology, ids.len() as u32),
            texture: draw.texture,
            screen_width: draw.screen_width,
        }
    }

    pub fn translate(&mut self, offset: [f32; 3]) {
        for position in &mut self.positions {
            for (axis, value) in position.iter_mut().enumerate() {
                *value += offset[axis];
            }
        }
    }
}

impl From<&DrawData> for MeshData {
    fn from(draw: &DrawData) -> Self {
        Self::with_settings(draw, &MeshSettings::default())
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

fn vertex_ids(draw: &DrawData) -> Vec<u32> {
    let Some(indices) = &draw.indices else {
        return (draw.vertex_offset..draw.vertex_offset + draw.vertex_count).collect();
    };
    let count = draw.vertex_count as usize;
    match indices.format {
        IndexFormat::Byte => indices.data.iter().take(count).map(|&i| u32::from(i)).collect(),
        IndexFormat::Short => indices
            .data
            .chunks_exact(2)
            .take(count)
            .map(|chunk| u32::from(u16::from_le_bytes([chunk[0], chunk[1]])))
            .collect(),
    }
}

fn read_attribute(buffer: &VertexBuffer, attribute: &VertexAttribute, id: u32) -> Option<[f32; 4]> {
    let size = attribute.format.size();
    let start = attribute.offset as usize + id as usize * buffer.stride as usize;
    let mut value = [0.0; 4];
    for (component, slot) in value
        .iter_mut()
        .enumerate()
        .take(usize::from(attribute.components))
    {
        let at = start + component * size;
        let bytes = buffer.data.get(at..at + size)?;
        *slot = match attribute.format {
            AttributeFormat::Byte => f32::from(bytes[0] as i8),
            AttributeFormat::UnsignedByte => f32::from(bytes[0]),
            AttributeFormat::Short => f32::from(i16::from_le_bytes([bytes[0], bytes[1]])),
            AttributeFormat::Float => {
                f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
            }
        };
    }
    Some(value)
}

fn triangulate(topology: Topology, count: u32) -> Vec<u32> {
    match topology {
        Topology::List => (0..count / 3 * 3).collect(),
        Topology::Strip => (0..count.saturating_sub(2))
            .flat_map(|i| {
                if i % 2 == 0 {
                    [i, i + 1, i + 2]
                } else {
                    [i + 1, i, i + 2]
                }
            })
            .collect(),
        Topology::Fan => (1..count.saturating_sub(1))
            .flat_map(|i| [0, i, i + 1])
            .collect(),
    }
}
