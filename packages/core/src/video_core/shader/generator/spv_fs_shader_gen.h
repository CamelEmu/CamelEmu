// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include <vector>

#include "common/common_types.h"

namespace Pica::Shader {
struct FSConfig;
struct Profile;
} // namespace Pica::Shader

namespace Pica::Shader::Generator::SPIRV {

/**
 * Generates the SPIR-V fragment shader program source code for the current Pica state
 * @param config ShaderCacheKey object generated for the current Pica state, used for the shader
 *               configuration (NOTE: Use state in this struct only, not the Pica registers!)
 * @param profile the capabilities of the host the shader is generated for
 * @returns The words of the SPIR-V module
 */
std::vector<u32> GenerateFragmentShader(const FSConfig& config, const Profile& profile);

} // namespace Pica::Shader::Generator::SPIRV
