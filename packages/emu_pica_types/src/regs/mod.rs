pub mod framebuffer;
pub mod lcd;
pub mod lighting;
pub mod pipeline;
pub mod rasterizer;
mod register_names;
pub mod shader;
pub mod texturing;

use std::mem::offset_of;

pub use framebuffer::FramebufferRegs;
pub use lcd::RegsLcd;
pub use lighting::LightingRegs;
pub use pipeline::PipelineRegs;
pub use rasterizer::RasterizerRegs;
pub use shader::ShaderRegs;
pub use texturing::TexturingRegs;

#[macro_export]
macro_rules! pica_reg_index {
    ($($field:tt).+) => {
        ::std::mem::offset_of!($crate::regs::RegsInternal, $($field).+) / ::std::mem::size_of::<u32>()
    };
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct RegsInternal {
    _padding0: [u32; 0x10],
    pub trigger_irq: u32,
    _padding1: [u32; 0x2f],
    pub rasterizer: RasterizerRegs,
    pub texturing: TexturingRegs,
    pub framebuffer: FramebufferRegs,
    pub lighting: LightingRegs,
    pub pipeline: PipelineRegs,
    pub gs: ShaderRegs,
    pub vs: ShaderRegs,
    _padding2: [u32; 0x20],
}

impl Default for RegsInternal {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl RegsInternal {
    pub const NUM_REGS: usize = 0x300;

    pub fn reg_array(&self) -> &[u32; Self::NUM_REGS] {
        unsafe { &*(self as *const Self).cast() }
    }

    pub fn reg_array_mut(&mut self) -> &mut [u32; Self::NUM_REGS] {
        unsafe { &mut *(self as *mut Self).cast() }
    }

    /// Map register indices to names readable by humans
    pub fn get_register_name(index: u16) -> &'static str {
        match register_names::REGISTER_NAMES.binary_search_by_key(&index, |&(id, _)| id) {
            Ok(position) => register_names::REGISTER_NAMES[position].1,
            // Return empty string if no match is found
            Err(_) => "",
        }
    }
}

const _: () = assert!(
    size_of::<RegsInternal>() == RegsInternal::NUM_REGS * size_of::<u32>(),
    "Regs struct has wrong size"
);

macro_rules! assert_reg_position {
    ($($field:tt).+, $position:expr) => {
        const _: () = assert!(
            offset_of!(RegsInternal, $($field).+) == $position * 4,
            concat!("Field ", stringify!($($field).+), " has invalid position")
        );
    };
}

assert_reg_position!(trigger_irq, 0x10);

assert_reg_position!(rasterizer, 0x40);
assert_reg_position!(rasterizer.cull_mode_raw, 0x40);
assert_reg_position!(rasterizer.viewport_size_x_raw, 0x41);
assert_reg_position!(rasterizer.viewport_size_y_raw, 0x43);
assert_reg_position!(rasterizer.viewport_depth_range_raw, 0x4d);
assert_reg_position!(rasterizer.viewport_depth_near_plane_raw, 0x4e);
assert_reg_position!(rasterizer.vs_output_attributes, 0x50);
assert_reg_position!(rasterizer.scissor_test, 0x65);
assert_reg_position!(rasterizer.viewport_corner, 0x68);
assert_reg_position!(rasterizer.depthmap_enable_raw, 0x6D);

assert_reg_position!(texturing, 0x80);
assert_reg_position!(texturing.main_config, 0x80);
assert_reg_position!(texturing.texture0, 0x81);
assert_reg_position!(texturing.texture0_format_raw, 0x8e);
assert_reg_position!(texturing.fragment_lighting_enable_raw, 0x8f);
assert_reg_position!(texturing.texture1, 0x91);
assert_reg_position!(texturing.texture1_format_raw, 0x96);
assert_reg_position!(texturing.texture2, 0x99);
assert_reg_position!(texturing.texture2_format_raw, 0x9e);
assert_reg_position!(texturing.proctex, 0xa8);
assert_reg_position!(texturing.proctex_noise_u, 0xa9);
assert_reg_position!(texturing.proctex_noise_v, 0xaa);
assert_reg_position!(texturing.proctex_noise_frequency, 0xab);
assert_reg_position!(texturing.proctex_lut, 0xac);
assert_reg_position!(texturing.proctex_lut_offset, 0xad);
assert_reg_position!(texturing.proctex_lut_config, 0xaf);
assert_reg_position!(texturing.tev_stage0, 0xc0);
assert_reg_position!(texturing.tev_stage1, 0xc8);
assert_reg_position!(texturing.tev_stage2, 0xd0);
assert_reg_position!(texturing.tev_stage3, 0xd8);
assert_reg_position!(texturing.fog_mode, 0xe0);
assert_reg_position!(texturing.fog_color, 0xe1);
assert_reg_position!(texturing.fog_lut_offset_raw, 0xe6);
assert_reg_position!(texturing.fog_lut_data, 0xe8);
assert_reg_position!(texturing.tev_stage4, 0xf0);
assert_reg_position!(texturing.tev_stage5, 0xf8);
assert_reg_position!(texturing.tev_combiner_buffer_color, 0xfd);

assert_reg_position!(framebuffer, 0x100);
assert_reg_position!(framebuffer.output_merger, 0x100);
assert_reg_position!(framebuffer.framebuffer, 0x110);

assert_reg_position!(lighting, 0x140);

assert_reg_position!(pipeline, 0x200);
assert_reg_position!(pipeline.vertex_attributes, 0x200);
assert_reg_position!(pipeline.index_array, 0x227);
assert_reg_position!(pipeline.num_vertices, 0x228);
assert_reg_position!(pipeline.vertex_offset, 0x22a);
assert_reg_position!(pipeline.trigger_draw, 0x22e);
assert_reg_position!(pipeline.trigger_draw_indexed, 0x22f);
assert_reg_position!(pipeline.vs_default_attributes_setup, 0x232);
assert_reg_position!(pipeline.command_buffer, 0x238);
assert_reg_position!(pipeline.gpu_mode, 0x245);
assert_reg_position!(pipeline.triangle_topology_raw, 0x25e);
assert_reg_position!(pipeline.restart_primitive, 0x25f);

assert_reg_position!(gs, 0x280);
assert_reg_position!(vs, 0x2b0);
