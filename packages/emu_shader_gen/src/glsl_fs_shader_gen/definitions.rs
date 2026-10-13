use emu_pica_types::regs::texturing::TextureType;

use super::{FS_UNIFORM_BLOCK_DEF, FragmentModule, Semantic};
use crate::glsl_shader_gen::FRAGMENT_SHADER_PRECISION_OES;

impl FragmentModule<'_> {
    pub(super) fn define_extensions(&mut self) {
        if self.profile.has_separable_shaders {
            self.out += "#extension GL_ARB_separate_shader_objects : enable\n";
        }
        if self.config.framebuffer.shadow_rendering() != 0 {
            self.use_fragment_shader_interlock = true;
            if self.profile.has_fragment_shader_interlock {
                self.out += "#extension GL_ARB_fragment_shader_interlock : enable\n";
                self.out += "#define beginInvocationInterlock beginInvocationInterlockARB\n";
                self.out += "#define endInvocationInterlock endInvocationInterlockARB\n";
            } else if self.profile.has_gl_nv_fragment_shader_interlock {
                self.out += "#extension GL_NV_fragment_shader_interlock : enable\n";
                self.out += "#define beginInvocationInterlock beginInvocationInterlockNV\n";
                self.out += "#define endInvocationInterlock endInvocationInterlockNV\n";
            } else if self.profile.has_gl_intel_fragment_shader_ordering {
                // NOTE: Intel does not have an end function for this.
                self.out += "#extension GL_INTEL_fragment_shader_ordering : enable\n";
                self.out += "#define beginInvocationInterlock beginFragmentShaderOrderingINTEL\n";
                self.out += "#define endInvocationInterlock()\n";
            } else {
                self.use_fragment_shader_interlock = false;
            }
        }
        if self.config.lighting.enable() != 0 {
            self.use_fragment_shader_barycentric = true;
            if self.profile.has_fragment_shader_barycentric {
                self.out += "#extension GL_EXT_fragment_shader_barycentric : enable\n";
                self.out += "#define pervertex pervertexEXT\n";
                self.out += "#define gl_BaryCoord gl_BaryCoordEXT\n";
            } else if self.profile.has_gl_nv_fragment_shader_barycentric {
                self.out += "#extension GL_NV_fragment_shader_barycentric : enable\n";
                self.out += "#define pervertex pervertexNV\n";
                self.out += "#define gl_BaryCoord gl_BaryCoordNV\n";
            } else {
                self.use_fragment_shader_barycentric = false;
            }
        }
        if self.config.emulate_blend() && !self.profile.is_vulkan {
            if self.profile.has_gl_ext_framebuffer_fetch {
                self.out += "#extension GL_EXT_shader_framebuffer_fetch : enable\n";
                self.out += "#define destFactor color\n";
            } else if self.profile.has_gl_arm_framebuffer_fetch {
                self.out += "#extension GL_ARM_shader_framebuffer_fetch : enable\n";
                self.out += "#define destFactor gl_LastFragColorARM\n";
            } else {
                self.out += "#define destFactor texelFetch(tex_color, ivec2(gl_FragCoord.xy), 0)\n";
                self.use_blend_fallback = true;
            }
        }

        if !self.profile.is_vulkan {
            self.out += FRAGMENT_SHADER_PRECISION_OES;
        }
    }

    fn define_input(&mut self, var: &str, location: Semantic) {
        if self.profile.has_separable_shaders {
            self.out += &format!("layout (location = {}) ", location as u32);
        }
        self.out += &format!("in {};\n", var);
    }

    pub(super) fn define_interface(&mut self) {
        // Input attributes
        self.define_input("vec4 primary_color", Semantic::Color);
        self.define_input("vec2 texcoord0", Semantic::Texcoord0);
        self.define_input("vec2 texcoord1", Semantic::Texcoord1);
        self.define_input("vec2 texcoord2", Semantic::Texcoord2);
        self.define_input("float texcoord0_w", Semantic::Texcoord0W);
        if self.use_fragment_shader_barycentric {
            self.define_input("pervertex vec4 normquats[]", Semantic::Normquat);
        } else {
            self.define_input("vec4 normquat", Semantic::Normquat);
        }
        self.define_input("vec3 view", Semantic::View);

        // Output attributes
        self.out += "layout (location = 0) out vec4 color;\n\n";
    }

    pub(super) fn define_bindings_vk(&mut self) {
        // Uniform and texture buffers
        self.out += FS_UNIFORM_BLOCK_DEF;
        self.out += "layout(set = 0, binding = 3) uniform samplerBuffer texture_buffer_lut_lf;\n";
        self.out += "layout(set = 0, binding = 4) uniform samplerBuffer texture_buffer_lut_rg;\n";
        self.out +=
            "layout(set = 0, binding = 5) uniform samplerBuffer texture_buffer_lut_rgba;\n\n";

        // Texture samplers
        let texture_type = self.config.texture.texture0_type();
        let sampler_tex0 = match texture_type {
            TextureType::SHADOW_2D | TextureType::SHADOW_CUBE => "usampler2D",
            TextureType::TEXTURE_CUBE => "samplerCube",
            _ => "sampler2D",
        };
        for i in 0..3 {
            let sampler = if i == 0 { sampler_tex0 } else { "sampler2D" };
            let num_descriptors = if i == 0 && texture_type == TextureType::SHADOW_CUBE {
                "[6]"
            } else {
                ""
            };
            self.out += &format!(
                "layout(set = 1, binding = {0}) uniform {1} tex{0}{2};\n",
                i, sampler, num_descriptors
            );
        }

        // Utility textures
        if self.config.framebuffer.shadow_rendering() != 0 {
            self.out += "layout(set = 2, binding = 0, r32ui) uniform uimage2D shadow_buffer;\n\n";
        }
        if self.config.user.use_custom_normal() != 0 {
            self.out += "layout(set = 2, binding = 1) uniform sampler2D tex_normal;\n";
        }
    }

    pub(super) fn define_bindings_gl(&mut self) {
        // Uniform and texture buffers
        self.out += FS_UNIFORM_BLOCK_DEF;
        self.out += "layout(binding = 3) uniform samplerBuffer texture_buffer_lut_lf;\n";
        self.out += "layout(binding = 4) uniform samplerBuffer texture_buffer_lut_rg;\n";
        self.out += "layout(binding = 5) uniform samplerBuffer texture_buffer_lut_rgba;\n\n";

        // Texture samplers
        let texture_type = self.config.texture.texture0_type();
        for i in 0..3 {
            let sampler = if i == 0 && texture_type == TextureType::TEXTURE_CUBE {
                "samplerCube"
            } else {
                "sampler2D"
            };
            self.out += &format!("layout(binding = {0}) uniform {1} tex{0};\n", i, sampler);
        }

        // Utility textures
        if self.config.user.use_custom_normal() != 0 {
            self.out += "layout(binding = 6) uniform sampler2D tex_normal;\n";
        }
        if self.use_blend_fallback {
            self.out += "layout(location = 7) uniform sampler2D tex_color;\n";
        }

        // Shadow textures
        if texture_type == TextureType::SHADOW_2D || texture_type == TextureType::SHADOW_CUBE {
            const POSTFIXES: [&str; 6] = ["px", "nx", "py", "ny", "pz", "nz"];
            for (i, postfix) in POSTFIXES.iter().enumerate() {
                self.out += &format!(
                    "layout(binding = {}, r32ui) uniform readonly uimage2D shadow_texture_{};\n",
                    i, postfix
                );
            }
        }
        if self.config.framebuffer.shadow_rendering() != 0 {
            self.out += "layout(binding = 6, r32ui) uniform uimage2D shadow_buffer;\n\n";
        }
    }

    pub(super) fn define_helpers(&mut self) {
        self.out += r#"
vec3 quaternion_rotate(vec4 q, vec3 v) {
    return v + 2.0 * cross(q.xyz, cross(q.xyz, v) + q.w * v);
}

float byteround(float x) {
    return round(x * 255.0) * (1.0 / 255.0);
}

vec2 byteround(vec2 x) {
    return round(x * 255.0) * (1.0 / 255.0);
}

vec3 byteround(vec3 x) {
    return round(x * 255.0) * (1.0 / 255.0);
}

vec4 byteround(vec4 x) {
    return round(x * 255.0) * (1.0 / 255.0);
}

float getLod(vec2 coord) {
    vec2 d = max(abs(dFdx(coord)), abs(dFdy(coord)));
    return log2(max(d.x, d.y));
}

uvec2 DecodeShadow(uint pixel) {
    return uvec2(pixel >> 8, pixel & 0xFFu);
}
"#;
    }

    pub(super) fn define_lighting_helpers(&mut self) {
        if self.config.lighting.enable() == 0 {
            return;
        }

        self.out += r#"
float LookupLightingLUT(int lut_index, int index, float delta) {
    vec2 entry = texelFetch(texture_buffer_lut_lf, lighting_lut_offset[lut_index >> 2][lut_index & 3] + index).rg;
    return entry.r + entry.g * delta;
}

float LookupLightingLUTUnsigned(int lut_index, float pos) {
    int index = int(clamp(floor(pos * 256.0), 0.f, 255.f));
    float delta = pos * 256.0 - float(index);
    return LookupLightingLUT(lut_index, index, delta);
}

float LookupLightingLUTSigned(int lut_index, float pos) {
    int index = int(clamp(floor(pos * 128.0), -128.f, 127.f));
    float delta = pos * 128.0 - float(index);
    if (index < 0) index += 256;
    return LookupLightingLUT(lut_index, index, delta);
}
"#;

        if self.use_fragment_shader_barycentric {
            self.out += r#"
bool AreQuaternionsOpposite(vec4 qa, vec4 qb) {
    return (dot(qa, qb) < 0.0);
}
"#;
        }
    }

    pub(super) fn define_tex_unit_sampler(&mut self, texture_unit: u32) {
        let texture = self.config.texture;
        self.out += &format!("vec4 sampleTexUnit{}() {{\n", texture_unit);
        if texture_unit == 0 && texture.texture0_type() == TextureType::DISABLED {
            self.out += "return vec4(0.0);\n}";
            return;
        }

        if texture_unit < 3 {
            let texcoord_num = if texture_unit == 2 && texture.texture2_use_coord1() != 0 {
                1
            } else {
                texture_unit
            };
            if texture.texture_border_color[texture_unit as usize].enable_s() != 0 {
                self.out += &format!(
                    r#"
                if (texcoord{}.x < 0 || texcoord{}.x > 1) {{
                    return tex_border_color[{}];
                }}
                "#,
                    texcoord_num, texcoord_num, texture_unit
                );
            }
            if texture.texture_border_color[texture_unit as usize].enable_t() != 0 {
                self.out += &format!(
                    r#"
                if (texcoord{}.y < 0 || texcoord{}.y > 1) {{
                    return tex_border_color[{}];
                }}
                "#,
                    texcoord_num, texcoord_num, texture_unit
                );
            }
        }

        match texture_unit {
            0 => match texture.texture0_type() {
                TextureType::TEXTURE_2D => {
                    self.out += "return textureLod(tex0, texcoord0, getLod(texcoord0 * \
                                 vec2(textureSize(tex0, 0))) + tex_lod_bias[0]);";
                }
                TextureType::PROJECTION_2D => {
                    // TODO (wwylele): find the exact LOD formula for projection texture
                    self.out += "return textureProj(tex0, vec3(texcoord0, texcoord0_w));";
                }
                TextureType::TEXTURE_CUBE => {
                    self.out += "return texture(tex0, vec3(texcoord0, texcoord0_w));";
                }
                TextureType::SHADOW_2D => {
                    self.out += "return shadowTexture(texcoord0, texcoord0_w);";
                }
                TextureType::SHADOW_CUBE => {
                    self.out += "return shadowTextureCube(texcoord0, texcoord0_w);";
                }
                _ => {
                    log::error!(
                        target: "HW_GPU",
                        "Unhandled texture type {:x}",
                        texture.texture0_type().0
                    );
                    log::error!(target: "Debug", "Unimplemented code!");
                    self.out += "return texture(tex0, texcoord0);";
                }
            },
            1 => {
                self.out += "return textureLod(tex1, texcoord1, getLod(texcoord1 * vec2(textureSize(tex1, \
                             0))) + tex_lod_bias[1]);";
            }
            2 => {
                if texture.texture2_use_coord1() != 0 {
                    self.out += "return textureLod(tex2, texcoord1, getLod(texcoord1 * \
                                 vec2(textureSize(tex2, 0))) + tex_lod_bias[2]);";
                } else {
                    self.out += "return textureLod(tex2, texcoord2, getLod(texcoord2 * \
                                 vec2(textureSize(tex2, 0))) + tex_lod_bias[2]);";
                }
            }
            3 => {
                if self.config.proctex.enable() != 0 {
                    self.out += "return ProcTex();";
                } else {
                    self.out += "return vec4(0.0);";
                }
            }
            _ => unreachable!(),
        }

        self.out += "\n}\n";
    }
}
