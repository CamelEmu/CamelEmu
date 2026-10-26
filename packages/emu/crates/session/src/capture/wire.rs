use screenshot_3d::{
    AlphaMode, ClipVertex, DrawData, Screenshot3dContext, ShaderData, ShaderProgram, TextureData,
    VertexData,
};

use super::screen::Screen;

struct WireSettings {
    no_texture: u32,
    uniform_components: usize,
    alpha_modes: [AlphaMode; 3],
}

impl Default for WireSettings {
    fn default() -> Self {
        Self {
            no_texture: u32::MAX,
            uniform_components: 4,
            alpha_modes: [AlphaMode::Opaque, AlphaMode::Mask, AlphaMode::Blend],
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let (head, tail) = self.data.split_at_checked(len)?;
        self.data = tail;
        Some(head)
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }

    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }

    fn words(&mut self) -> Option<Vec<u32>> {
        let count = self.u32()? as usize;
        let bytes = self.bytes(count.checked_mul(4)?)?;
        Some(
            bytes
                .chunks_exact(4)
                .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
                .collect(),
        )
    }

    fn blob(&mut self) -> Option<Vec<u8>> {
        let len = self.u32()? as usize;
        Some(self.bytes(len)?.to_vec())
    }

    fn items<T>(&mut self, mut read: impl FnMut(&mut Self) -> Option<T>) -> Option<Vec<T>> {
        let count = self.u32()?;
        (0..count).map(|_| read(self)).collect()
    }
}

pub struct RawCapture {
    vertices: VertexData,
    shaders: ShaderData,
    textures: Vec<TextureData>,
}

impl RawCapture {
    pub fn parse(data: &[u8]) -> Option<Self> {
        let settings = WireSettings::default();
        let mut reader = Reader { data };
        let draws = reader.items(|reader| read_draw(reader, &settings))?;
        let programs = reader.items(|reader| read_program(reader, &settings))?;
        let textures = reader.items(read_texture)?;
        Some(Self {
            vertices: VertexData { draws },
            shaders: ShaderData { programs },
            textures,
        })
    }

    pub fn context_for(&self, screen: &dyn Screen) -> Screenshot3dContext {
        let mut vertices = self.vertices.clone();
        vertices.draws.retain(|draw| screen.owns_draw(draw.screen));
        Screenshot3dContext::new(vertices, self.shaders.clone(), self.textures.clone())
    }
}

fn read_draw(reader: &mut Reader<'_>, settings: &WireSettings) -> Option<DrawData> {
    let screen = reader.u32()?;
    let screen_width = reader.u32()?;
    let flip_vertically = reader.u32()? != 0;
    let shader = reader.u32()? as usize;
    let texture = reader.u32()?;
    let position_output = reader.u32()?;
    let alpha_mode = *settings.alpha_modes.get(reader.u32()? as usize)?;
    let alpha_cutoff = reader.f32()?;
    let vertices = reader.items(|reader| {
        Some(ClipVertex {
            position: [reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?],
            texcoord: [reader.f32()?, reader.f32()?],
            color: [reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?],
        })
    })?;
    Some(DrawData {
        screen,
        screen_width,
        flip_vertically,
        shader,
        texture: (texture != settings.no_texture).then_some(texture as usize),
        position_output,
        alpha_mode,
        alpha_cutoff,
        vertices,
    })
}

fn read_program(reader: &mut Reader<'_>, settings: &WireSettings) -> Option<ShaderProgram> {
    let entry_point = reader.u32()?;
    let code = reader.words()?;
    let swizzle = reader.words()?;
    let vectors = reader.u32()? as usize;
    let mut uniforms = Vec::with_capacity(vectors);
    for _ in 0..vectors {
        let mut vector = [0.0; 4];
        for component in vector.iter_mut().take(settings.uniform_components) {
            *component = reader.f32()?;
        }
        uniforms.push(vector);
    }
    Some(ShaderProgram {
        code,
        swizzle,
        entry_point,
        uniforms,
        fragment_config: Vec::new(),
    })
}

fn read_texture(reader: &mut Reader<'_>) -> Option<TextureData> {
    let width = reader.u32()?;
    let height = reader.u32()?;
    let origin_s = reader.u32()? as i32;
    let top_t = reader.u32()? as i32;
    let source_width = reader.u32()?;
    let source_height = reader.u32()?;
    let wrap_s = u8::try_from(reader.u32()?).ok()?;
    let wrap_t = u8::try_from(reader.u32()?).ok()?;
    let magnify_linear = reader.u32()? != 0;
    let minify_linear = reader.u32()? != 0;
    Some(TextureData {
        width,
        height,
        origin_s,
        top_t,
        source_width,
        source_height,
        wrap_s,
        wrap_t,
        magnify_linear,
        minify_linear,
        rgba: reader.blob()?,
    })
}
