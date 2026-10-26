#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Profile {
    pub has_separable_shaders: bool,
    pub has_clip_planes: bool,
    pub has_geometry_shader: bool,
    pub has_custom_border_color: bool,
    pub has_fragment_shader_interlock: bool,
    pub has_fragment_shader_barycentric: bool,
    pub has_blend_minmax_factor: bool,
    pub has_minus_one_to_one_range: bool,
    pub has_logic_op: bool,
    pub has_gl_ext_framebuffer_fetch: bool,
    pub has_gl_arm_framebuffer_fetch: bool,
    pub has_gl_nv_fragment_shader_interlock: bool,
    pub has_gl_intel_fragment_shader_ordering: bool,
    pub has_gl_nv_fragment_shader_barycentric: bool,
    pub is_vulkan: bool,
}
