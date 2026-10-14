use emu_pica_types::regs::texturing::{TextureFilter, WrapMode};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SamplerParams {
    pub mag_filter: TextureFilter,
    pub min_filter: TextureFilter,
    pub mip_filter: TextureFilter,
    pub wrap_s: WrapMode,
    pub wrap_t: WrapMode,
    pub border_color: u32,
    pub lod_min: u32,
    pub lod_max: u32,
    pub lod_bias: i32,
}
