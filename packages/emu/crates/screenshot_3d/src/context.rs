use crate::camera::{CameraData, CameraSettings};

#[derive(Debug, Clone, Default)]
pub struct Screenshot3dContext {
    pub vertices: VertexData,
    pub shaders: ShaderData,
    pub camera: CameraData,
    pub textures: Vec<TextureData>,
}

impl Screenshot3dContext {
    pub fn new(vertices: VertexData, shaders: ShaderData, textures: Vec<TextureData>) -> Self {
        let settings = CameraSettings::default();
        let camera = vertices
            .draws
            .iter()
            .max_by_key(|draw| draw.vertex_count)
            .and_then(|draw| shaders.programs.get(draw.shader))
            .and_then(|program| CameraData::estimate(program, &settings))
            .unwrap_or_default();
        Self {
            vertices,
            shaders,
            camera,
            textures,
        }
    }

    pub fn retain_draws(&mut self, keep: impl FnMut(&DrawData) -> bool) {
        self.vertices.draws.retain(keep);
    }
}

#[derive(Debug, Clone, Default)]
pub struct VertexData {
    pub draws: Vec<DrawData>,
}

#[derive(Debug, Clone)]
pub struct DrawData {
    pub screen_width: u32,
    pub topology: Topology,
    pub vertex_count: u32,
    pub vertex_offset: u32,
    pub buffers: Vec<VertexBuffer>,
    pub attributes: Vec<VertexAttribute>,
    pub indices: Option<IndexData>,
    pub shader: usize,
    pub texture: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topology {
    List,
    Strip,
    Fan,
}

#[derive(Debug, Clone)]
pub struct VertexBuffer {
    pub data: Vec<u8>,
    pub stride: u32,
}

#[derive(Debug, Clone)]
pub struct VertexAttribute {
    pub buffer: usize,
    pub offset: u32,
    pub format: AttributeFormat,
    pub components: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeFormat {
    Byte,
    UnsignedByte,
    Short,
    Float,
}

impl AttributeFormat {
    pub fn size(self) -> usize {
        match self {
            Self::Byte | Self::UnsignedByte => 1,
            Self::Short => 2,
            Self::Float => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexData {
    pub format: IndexFormat,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexFormat {
    Byte,
    Short,
}

#[derive(Debug, Clone, Default)]
pub struct ShaderData {
    pub programs: Vec<ShaderProgram>,
}

#[derive(Debug, Clone, Default)]
pub struct ShaderProgram {
    pub code: Vec<u32>,
    pub swizzle: Vec<u32>,
    pub entry_point: u32,
    pub uniforms: Vec<[f32; 4]>,
    pub fragment_config: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub wrap_s: u8,
    pub wrap_t: u8,
    pub magnify_linear: bool,
    pub minify_linear: bool,
}
