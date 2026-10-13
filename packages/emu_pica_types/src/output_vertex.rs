use crate::float::F24;
use crate::vector::{Vec2, Vec3, Vec4};

pub type AttributeBuffer = [Vec4<F24>; 16];

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OutputVertex {
    pub pos: Vec4<F24>,
    pub quat: Vec4<F24>,
    pub color: Vec4<F24>,
    pub tc0: Vec2<F24>,
    pub tc1: Vec2<F24>,
    pub tc0_w: F24,
    _padding0: u32,
    pub view: Vec3<F24>,
    _padding1: u32,
    pub tc2: Vec2<F24>,
}

const _: () = assert!(size_of::<OutputVertex>() == 24 * size_of::<f32>());
