#ifndef EMU_SHADER_H
#define EMU_SHADER_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct EmuShaderModule EmuShaderModule;

typedef struct EmuShaderProfile {
    bool has_separable_shaders;
    bool has_clip_planes;
    bool has_geometry_shader;
    bool has_custom_border_color;
    bool has_fragment_shader_interlock;
    bool has_fragment_shader_barycentric;
    bool has_blend_minmax_factor;
    bool has_minus_one_to_one_range;
    bool has_logic_op;
    bool has_gl_ext_framebuffer_fetch;
    bool has_gl_arm_framebuffer_fetch;
    bool has_gl_nv_fragment_shader_interlock;
    bool has_gl_intel_fragment_shader_ordering;
    bool has_gl_nv_fragment_shader_barycentric;
    bool is_vulkan;
} EmuShaderProfile;

EmuShaderModule* emu_shader_generate_fragment(const uint8_t* config, size_t config_len,
                                              const EmuShaderProfile* profile);

const uint32_t* emu_shader_module_words(const EmuShaderModule* handle);

size_t emu_shader_module_word_count(const EmuShaderModule* handle);

void emu_shader_module_free(EmuShaderModule* handle);

#ifdef __cplusplus
}
#endif

#endif
