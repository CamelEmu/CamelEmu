use std::ptr;
use std::slice;

use crate::{Profile, generate_fragment_shader};

pub struct EmuShaderModule(Vec<u32>);

#[repr(C)]
pub struct EmuShaderProfile {
    has_separable_shaders: bool,
    has_clip_planes: bool,
    has_geometry_shader: bool,
    has_custom_border_color: bool,
    has_fragment_shader_interlock: bool,
    has_fragment_shader_barycentric: bool,
    has_blend_minmax_factor: bool,
    has_minus_one_to_one_range: bool,
    has_logic_op: bool,
    has_gl_ext_framebuffer_fetch: bool,
    has_gl_arm_framebuffer_fetch: bool,
    has_gl_nv_fragment_shader_interlock: bool,
    has_gl_intel_fragment_shader_ordering: bool,
    has_gl_nv_fragment_shader_barycentric: bool,
    is_vulkan: bool,
}

impl From<&EmuShaderProfile> for Profile {
    fn from(profile: &EmuShaderProfile) -> Self {
        Self {
            has_separable_shaders: profile.has_separable_shaders,
            has_clip_planes: profile.has_clip_planes,
            has_geometry_shader: profile.has_geometry_shader,
            has_custom_border_color: profile.has_custom_border_color,
            has_fragment_shader_interlock: profile.has_fragment_shader_interlock,
            has_fragment_shader_barycentric: profile.has_fragment_shader_barycentric,
            has_blend_minmax_factor: profile.has_blend_minmax_factor,
            has_minus_one_to_one_range: profile.has_minus_one_to_one_range,
            has_logic_op: profile.has_logic_op,
            has_gl_ext_framebuffer_fetch: profile.has_gl_ext_framebuffer_fetch,
            has_gl_arm_framebuffer_fetch: profile.has_gl_arm_framebuffer_fetch,
            has_gl_nv_fragment_shader_interlock: profile.has_gl_nv_fragment_shader_interlock,
            has_gl_intel_fragment_shader_ordering: profile.has_gl_intel_fragment_shader_ordering,
            has_gl_nv_fragment_shader_barycentric: profile.has_gl_nv_fragment_shader_barycentric,
            is_vulkan: profile.is_vulkan,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_shader_generate_fragment(
    config: *const u8,
    config_len: usize,
    profile: *const EmuShaderProfile,
) -> *mut EmuShaderModule {
    let Some(profile) = (unsafe { profile.as_ref() }) else {
        return ptr::null_mut();
    };
    if config.is_null() {
        return ptr::null_mut();
    }
    let config = unsafe { slice::from_raw_parts(config, config_len) };
    match generate_fragment_shader(config, Profile::from(profile)) {
        Ok(words) => Box::into_raw(Box::new(EmuShaderModule(words))),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_shader_module_words(handle: *const EmuShaderModule) -> *const u32 {
    match unsafe { handle.as_ref() } {
        Some(module) => module.0.as_ptr(),
        None => ptr::null(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_shader_module_word_count(handle: *const EmuShaderModule) -> usize {
    match unsafe { handle.as_ref() } {
        Some(module) => module.0.len(),
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_shader_module_free(handle: *mut EmuShaderModule) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}
