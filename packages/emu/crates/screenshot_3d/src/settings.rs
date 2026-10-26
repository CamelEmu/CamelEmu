pub struct ExportSettings {
    pub stack_gap: f32,
    pub glb_magic: u32,
    pub glb_version: u32,
    pub json_chunk_type: u32,
    pub bin_chunk_type: u32,
    pub chunk_alignment: usize,
    pub json_padding: u8,
    pub component_float: u32,
    pub component_unsigned_int: u32,
    pub target_array_buffer: u32,
    pub target_element_array_buffer: u32,
    pub mode_triangles: u32,
    pub filter_nearest: u32,
    pub filter_linear: u32,
    pub wrap_clamp_to_edge: u32,
    pub wrap_repeat: u32,
    pub wrap_mirrored_repeat: u32,
    pub pica_wrap_repeat: u8,
    pub pica_wrap_mirrored_repeat: u8,
    pub png_mime_type: &'static str,
    pub generator: &'static str,
    pub texture_file_prefix: &'static str,
    pub material_name_prefix: &'static str,
    pub mesh_name_prefix: &'static str,
}

impl Default for ExportSettings {
    fn default() -> Self {
        Self {
            stack_gap: 0.1,
            glb_magic: 0x4654_6c67,
            glb_version: 2,
            json_chunk_type: 0x4e4f_534a,
            bin_chunk_type: 0x004e_4942,
            chunk_alignment: 4,
            json_padding: b' ',
            component_float: 5126,
            component_unsigned_int: 5125,
            target_array_buffer: 34962,
            target_element_array_buffer: 34963,
            mode_triangles: 4,
            filter_nearest: 9728,
            filter_linear: 9729,
            wrap_clamp_to_edge: 33071,
            wrap_repeat: 10497,
            wrap_mirrored_repeat: 33648,
            pica_wrap_repeat: 2,
            pica_wrap_mirrored_repeat: 3,
            png_mime_type: "image/png",
            generator: "CamelEmu screenshot_3d",
            texture_file_prefix: "texture_",
            material_name_prefix: "material_",
            mesh_name_prefix: "draw_",
        }
    }
}

pub struct MeshSettings {
    pub position_min_components: u8,
    pub texcoord_components: u8,
}

impl Default for MeshSettings {
    fn default() -> Self {
        Self {
            position_min_components: 3,
            texcoord_components: 2,
        }
    }
}
