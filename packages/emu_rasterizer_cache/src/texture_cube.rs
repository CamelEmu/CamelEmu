use emu_pica_types::regs::texturing::TextureFormat;

use crate::slot_id::SurfaceId;

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureCube {
    pub surface_id: SurfaceId,
    pub face_ids: [SurfaceId; 6],
    pub ticks: [u64; 6],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureCubeConfig {
    pub px: u32,
    pub nx: u32,
    pub py: u32,
    pub ny: u32,
    pub pz: u32,
    pub nz: u32,
    pub width: u32,
    pub levels: u32,
    pub format: TextureFormat,
}
