// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <memory>

#include <emu_shader.h>

#include "common/assert.h"
#include "video_core/shader/generator/pica_fs_config.h"
#include "video_core/shader/generator/spv_fs_shader_gen.h"

namespace Pica::Shader::Generator::SPIRV {

std::vector<u32> GenerateFragmentShader(const FSConfig& config, const Profile& profile) {
    const EmuShaderProfile emu_profile{
        .has_separable_shaders = profile.has_separable_shaders,
        .has_clip_planes = profile.has_clip_planes,
        .has_geometry_shader = profile.has_geometry_shader,
        .has_custom_border_color = profile.has_custom_border_color,
        .has_fragment_shader_interlock = profile.has_fragment_shader_interlock,
        .has_fragment_shader_barycentric = profile.has_fragment_shader_barycentric,
        .has_blend_minmax_factor = profile.has_blend_minmax_factor,
        .has_minus_one_to_one_range = profile.has_minus_one_to_one_range,
        .has_logic_op = profile.has_logic_op,
        .has_gl_ext_framebuffer_fetch = profile.has_gl_ext_framebuffer_fetch,
        .has_gl_arm_framebuffer_fetch = profile.has_gl_arm_framebuffer_fetch,
        .has_gl_nv_fragment_shader_interlock = profile.has_gl_nv_fragment_shader_interlock,
        .has_gl_intel_fragment_shader_ordering = profile.has_gl_intel_fragment_shader_ordering,
        .has_gl_nv_fragment_shader_barycentric = profile.has_gl_nv_fragment_shader_barycentric,
        .is_vulkan = profile.is_vulkan,
    };

    const std::unique_ptr<EmuShaderModule, decltype(&emu_shader_module_free)> module{
        emu_shader_generate_fragment(reinterpret_cast<const u8*>(&config), sizeof(FSConfig),
                                     &emu_profile),
        &emu_shader_module_free};
    ASSERT_MSG(module, "Failed to generate the SPIR-V fragment shader");

    const u32* words = emu_shader_module_words(module.get());
    return std::vector<u32>(words, words + emu_shader_module_word_count(module.get()));
}

} // namespace Pica::Shader::Generator::SPIRV
