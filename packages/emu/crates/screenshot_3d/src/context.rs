use crate::camera::CameraData;
use crate::settings::ViewSettings;

#[derive(Debug, Clone, Default)]
pub struct Screenshot3dContext {
    pub vertices: VertexData,
    pub shaders: ShaderData,
    pub camera: CameraData,
    pub textures: Vec<TextureData>,
}

impl Screenshot3dContext {
    pub fn new(vertices: VertexData, shaders: ShaderData, textures: Vec<TextureData>) -> Self {
        let camera = CameraData::for_draws(&vertices.draws, &shaders, &ViewSettings::default());
        Self {
            vertices,
            shaders,
            camera,
            textures,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VertexData {
    pub draws: Vec<DrawData>,
}

#[derive(Debug, Clone)]
pub struct DrawData {
    pub screen: u32,
    pub screen_width: u32,
    pub flip_vertically: bool,
    pub shader: usize,
    pub texture: Option<usize>,
    pub position_output: u32,
    pub viewport: Viewport,
    pub depth_order: DepthOrder,
    pub w_buffer: bool,
    pub alpha_mode: AlphaMode,
    pub alpha_cutoff: f32,
    pub vertices: Vec<ClipVertex>,
    pub modes: Vec<TriangleMode>,
    pub patches: Vec<PatchData>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub half_x: f32,
    pub half_y: f32,
    pub corner_x: f32,
    pub corner_y: f32,
    pub width: f32,
    pub height: f32,
    pub depth_scale: f32,
    pub depth_offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthOrder {
    DrawOrder,
    NearerSmaller,
    NearerGreater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriangleMode {
    Texture,
    Color,
    Patch,
}

#[derive(Debug, Clone)]
pub struct PatchData {
    pub size: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaMode {
    Opaque,
    Mask,
    Blend,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipVertex {
    pub position: [f32; 4],
    pub texcoord: [f32; 2],
    pub color: [f32; 4],
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
    pub origin_s: i32,
    pub top_t: i32,
    pub source_width: u32,
    pub source_height: u32,
    pub rgba: Vec<u8>,
    pub wrap_s: u8,
    pub wrap_t: u8,
    pub magnify_linear: bool,
    pub minify_linear: bool,
}
