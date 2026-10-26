use crate::camera::CameraData;
use crate::context::{DrawData, ShaderData, ShaderProgram};
use crate::settings::ViewSettings;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frustum {
    pub half_width: f32,
    pub half_height: f32,
}

impl Frustum {
    pub fn probe(draw: &DrawData, shaders: &ShaderData, settings: &ViewSettings) -> Option<Self> {
        shaders
            .programs
            .get(draw.shader)
            .and_then(|program| Self::from_program(program, draw.position_output, settings))
    }

    pub fn reference(draws: &[DrawData], shaders: &ShaderData, settings: &ViewSettings) -> Self {
        draws
            .iter()
            .filter_map(|draw| {
                Self::probe(draw, shaders, settings).map(|frustum| (draw.vertices.len(), frustum))
            })
            .max_by_key(|(count, _)| *count)
            .map(|(_, frustum)| frustum)
            .unwrap_or_else(|| {
                let width = draws
                    .iter()
                    .max_by_key(|draw| draw.vertices.len())
                    .map_or(settings.reference_width, |draw| draw.screen_width);
                Self::with_half_height((settings.yfov / 2.0).tan(), width, settings)
            })
    }

    pub fn from_camera(camera: &CameraData) -> Self {
        let half_height = (camera.projection.yfov / 2.0).tan();
        Self {
            half_width: half_height * camera.projection.aspect,
            half_height,
        }
    }

    fn with_half_height(half_height: f32, screen_width: u32, settings: &ViewSettings) -> Self {
        Self {
            half_width: half_height * screen_width as f32 / settings.screen_height,
            half_height,
        }
    }

    fn from_program(
        program: &ShaderProgram,
        output: u32,
        settings: &ViewSettings,
    ) -> Option<Self> {
        let mut rows: [Option<[f32; 4]>; 4] = [None; 4];
        for word in &program.code {
            let opcode = word >> settings.opcode_shift;
            let dest = (word >> settings.dest_shift) & settings.dest_mask;
            let src1 = (word >> settings.src1_shift) & settings.src1_mask;
            if (opcode != settings.dp4_opcode && opcode != settings.dph_opcode)
                || dest != output
                || src1 < settings.first_float_uniform
            {
                continue;
            }
            let Some(row) = program
                .uniforms
                .get((src1 - settings.first_float_uniform) as usize)
            else {
                continue;
            };
            let Some(descriptor) = program.swizzle.get((word & settings.descriptor_mask) as usize)
            else {
                continue;
            };
            let mask = descriptor & settings.component_mask;
            for (component, slot) in rows.iter_mut().enumerate() {
                if mask & (settings.component_top_bit >> component) != 0 {
                    *slot = Some(*row);
                }
            }
        }

        let length = |row: [f32; 4]| (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
        let [Some(x), Some(y), _, Some(w)] = rows else {
            return None;
        };
        let (x, y, w) = (length(x), length(y), length(w));
        if [x, y, w]
            .iter()
            .any(|value| !value.is_finite() || *value < settings.min_row_length)
        {
            return None;
        }
        Some(Self {
            half_width: w / y,
            half_height: w / x,
        })
    }
}
