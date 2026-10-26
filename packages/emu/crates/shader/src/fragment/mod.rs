mod generate;
mod helpers;
mod interface;
mod lighting;
mod proctex;
mod sampler;
mod shadow;
mod tev;

use std::ops::{Deref, DerefMut};

use emu_spirv::{Id, Module};

use crate::config::FsConfig;
use crate::error::Result;
use crate::profile::Profile;
use crate::settings::FragmentSettings;

#[derive(Debug, Clone, Copy, Default)]
pub struct VectorIds {
    pub ids: [Id; 3],
}

impl VectorIds {
    pub fn get(&self, size: u32) -> Id {
        self.ids[(size - 2) as usize]
    }
}

pub struct FragmentModule {
    module: Module,
    pub(crate) config: FsConfig,
    pub(crate) settings: FragmentSettings,
    pub(crate) use_fragment_shader_barycentric: bool,

    pub(crate) void_id: Id,
    pub(crate) bool_id: Id,
    pub(crate) f32_id: Id,
    pub(crate) i32_id: Id,
    pub(crate) u32_id: Id,

    pub(crate) vec_ids: VectorIds,
    pub(crate) ivec_ids: VectorIds,
    pub(crate) uvec_ids: VectorIds,
    pub(crate) bvec_ids: VectorIds,

    pub(crate) image2d_id: Id,
    pub(crate) image_cube_id: Id,
    pub(crate) image_buffer_id: Id,
    pub(crate) image_r32_id: Id,
    pub(crate) sampler_id: Id,
    pub(crate) shader_data_id: Id,

    pub(crate) primary_color_id: Id,
    pub(crate) texcoord_id: [Id; 3],
    pub(crate) texcoord0_w_id: Id,
    pub(crate) normquat_id: Id,
    pub(crate) view_id: Id,
    pub(crate) color_id: Id,

    pub(crate) gl_frag_coord_id: Id,
    pub(crate) gl_frag_depth_id: Id,
    pub(crate) gl_bary_coord_id: Id,
    pub(crate) depth: Id,

    pub(crate) tex0_id: Id,
    pub(crate) tex1_id: Id,
    pub(crate) tex2_id: Id,
    pub(crate) texture_buffer_lut_lf_id: Id,
    pub(crate) texture_buffer_lut_rg_id: Id,
    pub(crate) texture_buffer_lut_rgba_id: Id,
    pub(crate) shadow_texture_px_id: Id,

    pub(crate) texture_buffer_lut_lf: Id,
    pub(crate) texture_buffer_lut_rg: Id,
    pub(crate) texture_buffer_lut_rgba: Id,

    pub(crate) rounded_primary_color: Id,
    pub(crate) primary_fragment_color: Id,
    pub(crate) secondary_fragment_color: Id,
    pub(crate) combiner_buffer: Id,
    pub(crate) next_combiner_buffer: Id,
    pub(crate) combiner_output: Id,

    pub(crate) color_results_1: Id,
    pub(crate) color_results_2: Id,
    pub(crate) color_results_3: Id,
    pub(crate) alpha_results_1: Id,
    pub(crate) alpha_results_2: Id,
    pub(crate) alpha_results_3: Id,

    pub(crate) sample_tex_unit_func: [Id; 4],
    pub(crate) noise1d_table: Id,
    pub(crate) noise2d_table: Id,
    pub(crate) lut_offsets: Id,
}

impl Deref for FragmentModule {
    type Target = Module;

    fn deref(&self) -> &Module {
        &self.module
    }
}

impl DerefMut for FragmentModule {
    fn deref_mut(&mut self) -> &mut Module {
        &mut self.module
    }
}

impl FragmentModule {
    pub fn new(config: FsConfig, profile: Profile) -> Result<Self> {
        let settings = FragmentSettings::default();
        let use_fragment_shader_barycentric =
            profile.has_fragment_shader_barycentric && config.lighting.enable();
        let mut fragment = Self {
            module: Module::new(settings.spirv_version),
            config,
            settings,
            use_fragment_shader_barycentric,
            void_id: Id::default(),
            bool_id: Id::default(),
            f32_id: Id::default(),
            i32_id: Id::default(),
            u32_id: Id::default(),
            vec_ids: VectorIds::default(),
            ivec_ids: VectorIds::default(),
            uvec_ids: VectorIds::default(),
            bvec_ids: VectorIds::default(),
            image2d_id: Id::default(),
            image_cube_id: Id::default(),
            image_buffer_id: Id::default(),
            image_r32_id: Id::default(),
            sampler_id: Id::default(),
            shader_data_id: Id::default(),
            primary_color_id: Id::default(),
            texcoord_id: [Id::default(); 3],
            texcoord0_w_id: Id::default(),
            normquat_id: Id::default(),
            view_id: Id::default(),
            color_id: Id::default(),
            gl_frag_coord_id: Id::default(),
            gl_frag_depth_id: Id::default(),
            gl_bary_coord_id: Id::default(),
            depth: Id::default(),
            tex0_id: Id::default(),
            tex1_id: Id::default(),
            tex2_id: Id::default(),
            texture_buffer_lut_lf_id: Id::default(),
            texture_buffer_lut_rg_id: Id::default(),
            texture_buffer_lut_rgba_id: Id::default(),
            shadow_texture_px_id: Id::default(),
            texture_buffer_lut_lf: Id::default(),
            texture_buffer_lut_rg: Id::default(),
            texture_buffer_lut_rgba: Id::default(),
            rounded_primary_color: Id::default(),
            primary_fragment_color: Id::default(),
            secondary_fragment_color: Id::default(),
            combiner_buffer: Id::default(),
            next_combiner_buffer: Id::default(),
            combiner_output: Id::default(),
            color_results_1: Id::default(),
            color_results_2: Id::default(),
            color_results_3: Id::default(),
            alpha_results_1: Id::default(),
            alpha_results_2: Id::default(),
            alpha_results_3: Id::default(),
            sample_tex_unit_func: [Id::default(); 4],
            noise1d_table: Id::default(),
            noise2d_table: Id::default(),
            lut_offsets: Id::default(),
        };

        fragment.define_arithmetic_types();
        fragment.define_uniform_structs();
        fragment.define_interface();
        for texture_unit in 0..fragment.settings.num_tex_units {
            fragment.define_tex_sampler(texture_unit)?;
        }
        fragment.define_entry_point();
        Ok(fragment)
    }
}

pub fn generate_fragment_shader(config: FsConfig, profile: Profile) -> Result<Vec<u32>> {
    let mut fragment = FragmentModule::new(config, profile)?;
    fragment.generate()?;
    Ok(fragment.assemble())
}
