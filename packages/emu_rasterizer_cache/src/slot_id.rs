use crate::slot_vector::SlotId;

pub type SurfaceId = SlotId;
pub type SamplerId = SlotId;
pub type FramebufferId = SlotId;

/// Fake surface ID for null surfaces
pub const NULL_SURFACE_ID: SurfaceId = SlotId::new(0);
/// Fake surface ID for null cube surfaces
pub const NULL_SURFACE_CUBE_ID: SurfaceId = SlotId::new(1);
/// Fake sampler ID for null samplers
pub const NULL_SAMPLER_ID: SamplerId = SlotId::new(0);
