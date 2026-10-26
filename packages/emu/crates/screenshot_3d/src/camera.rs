use crate::context::{DrawData, ShaderData};
use crate::settings::ViewSettings;
use crate::view::Frustum;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraData {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub projection: Projection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    pub yfov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraData {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            projection: ViewSettings::default().fallback_projection,
        }
    }
}

impl CameraData {
    pub fn for_draws(draws: &[DrawData], shaders: &ShaderData, settings: &ViewSettings) -> Self {
        let fallback = settings.fallback_projection;
        if draws.is_empty() {
            return Self::default();
        }
        let frustum = Frustum::reference(draws, shaders, settings);
        let depths = draws
            .iter()
            .flat_map(|draw| draw.vertices.iter())
            .map(|vertex| vertex.position[3])
            .filter(|w| w.is_finite() && *w > settings.min_w);
        let (near, far) = depths.fold((f32::MAX, f32::MIN), |(near, far), w| {
            (near.min(w), far.max(w))
        });
        let (near, far) = if near <= far {
            (near * settings.near_margin, far * settings.far_margin)
        } else {
            (fallback.near, fallback.far)
        };
        Self {
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            projection: Projection {
                yfov: 2.0 * frustum.half_height.atan(),
                aspect: frustum.half_width / frustum.half_height,
                near,
                far,
            },
        }
    }
}
