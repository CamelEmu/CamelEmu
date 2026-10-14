// Copyright 2023 Citra Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

mod definitions;
mod lighting;
mod proctex;
mod shadow;
mod tev;

use emu_pica_types::regs::framebuffer::{BlendEquation, BlendFactor, CompareFunc, LogicOp};
use emu_pica_types::regs::rasterizer::{DepthBuffering, ScissorMode};
use emu_pica_types::regs::texturing::FogMode;

use crate::pica_fs_config::FSConfig;
use crate::profile::Profile;

const RESERVE_SIZE: usize = 8 * 1024 * 1024;

#[allow(dead_code)]
#[derive(Clone, Copy)]
#[repr(u32)]
enum Semantic {
    Position,
    Color,
    Texcoord0,
    Texcoord1,
    Texcoord2,
    Texcoord0W,
    Normquat,
    View,
}

const FS_UNIFORM_BLOCK_DEF: &str = r#"
#define NUM_TEV_STAGES 6
#define NUM_LIGHTS 8
#define NUM_LIGHTING_SAMPLERS 24
struct LightSrc {
    vec3 specular_0;
    vec3 specular_1;
    vec3 diffuse;
    vec3 ambient;
    vec3 position;
    vec3 spot_direction;
    float dist_atten_bias;
    float dist_atten_scale;
};
layout (binding = 2, std140) uniform fs_data {
    int framebuffer_scale;
    int alphatest_ref;
    float depth_scale;
    float depth_offset;
    float shadow_bias_constant;
    float shadow_bias_linear;
    int scissor_x1;
    int scissor_y1;
    int scissor_x2;
    int scissor_y2;
    int fog_lut_offset;
    int proctex_noise_lut_offset;
    int proctex_color_map_offset;
    int proctex_alpha_map_offset;
    int proctex_lut_offset;
    int proctex_diff_lut_offset;
    float proctex_bias;
    int shadow_texture_bias;
    ivec4 lighting_lut_offset[NUM_LIGHTING_SAMPLERS / 4];
    vec3 fog_color;
    vec2 proctex_noise_f;
    vec2 proctex_noise_a;
    vec2 proctex_noise_p;
    vec3 lighting_global_ambient;
    LightSrc light_src[NUM_LIGHTS];
    vec4 const_color[NUM_TEV_STAGES];
    vec4 tev_combiner_buffer_color;
    vec3 tex_lod_bias;
    vec4 tex_border_color[3];
    vec4 blend_color;
};
"#;

pub struct FragmentModule<'a> {
    config: &'a FSConfig,
    profile: &'a Profile,
    out: String,
    use_blend_fallback: bool,
    use_fragment_shader_interlock: bool,
    use_fragment_shader_barycentric: bool,
}

impl<'a> FragmentModule<'a> {
    pub fn new(config: &'a FSConfig, profile: &'a Profile) -> Self {
        let mut module = Self {
            config,
            profile,
            out: String::with_capacity(RESERVE_SIZE),
            use_blend_fallback: false,
            use_fragment_shader_interlock: false,
            use_fragment_shader_barycentric: false,
        };
        module.define_extensions();
        module.define_interface();
        if profile.is_vulkan {
            module.define_bindings_vk();
        } else {
            module.define_bindings_gl();
        }
        module.define_helpers();
        module.define_shadow_helpers();
        module.define_lighting_helpers();
        module.define_proctex_sampler();
        for i in 0..4 {
            module.define_tex_unit_sampler(i);
        }
        module
    }

    /// Emits GLSL source corresponding to the provided pica fragment configuration
    pub fn generate(mut self) -> String {
        // We round the interpolated primary color to the nearest 1/255th
        // This maintains the PICA's 8 bits of precision
        self.out += r#"
void main() {
vec4 rounded_primary_color = byteround(primary_color);
vec4 primary_fragment_color = vec4(0.0);
vec4 secondary_fragment_color = vec4(0.0);
"#;

        // Do not do any sort of processing if it's obvious we're not going to pass the alpha test
        if self.config.framebuffer.alpha_test_func() == CompareFunc::NEVER {
            self.out += "discard; }";
            return self.out;
        }

        // Append the scissor and depth tests
        self.write_scissor();
        self.write_depth();

        // Write shader source to emulate all enabled PICA lights
        self.write_lighting();

        self.out += "vec4 combiner_buffer = vec4(0.0);\n\
                     vec4 next_combiner_buffer = tev_combiner_buffer_color;\n\
                     vec4 combiner_output = vec4(0.0);\n";

        self.out += "vec3 color_results_1 = vec3(0.0);\n\
                     vec3 color_results_2 = vec3(0.0);\n\
                     vec3 color_results_3 = vec3(0.0);\n";

        self.out += "float alpha_results_1 = 0.0;\n\
                     float alpha_results_2 = 0.0;\n\
                     float alpha_results_3 = 0.0;\n";

        // Write shader source to emulate PICA TEV stages
        for index in 0..self.config.texture.tev_stages.len() as u32 {
            self.write_tev_stage(index);
        }

        // Append the alpha test condition
        self.write_alpha_test_condition(self.config.framebuffer.alpha_test_func());

        // Emulate the fog
        match self.config.texture.fog_mode() {
            FogMode::FOG => self.write_fog(),
            FogMode::GAS => {
                self.write_gas();
                return self.out;
            }
            _ => {}
        }

        if self.config.framebuffer.shadow_rendering() != 0 {
            self.write_shadow();
        } else {
            self.out += "gl_FragDepth = depth;\n";
            // Round the final fragment color to maintain the PICA's 8 bits of precision
            self.out += "combiner_output = byteround(combiner_output);\n";
            self.write_blending();
            self.out += "color = combiner_output;\n";
        }

        self.write_logic_op();

        self.out.push('}');
        self.out
    }

    /// Undos the host perspective transformation and applies the PICA one
    fn write_depth(&mut self) {
        // The PICA depth range is [-1, 0]. The vertex shader outputs the negated Z value, otherwise
        // unmodified. When the depth range is [-1, 1], it is converted into [near, far] = [0, 1].
        // This compresses our effective range into [0.5, 1]. To account for this we un-negate the value
        // to range [-1, -0.5], multiply by 2 to the range [-2, -1], and add 1 to arrive back at the
        // original range of [-1, 0]. If the depth range is [0, 1], so all we need to do is
        // un-negate the value to range [-1, 0]. Once we have z_over_w, we can do our own transformation
        // according to PICA specification.
        if self.profile.has_minus_one_to_one_range {
            self.out += "float z_over_w = -2.0 * gl_FragCoord.z + 1.0;\n";
        } else {
            self.out += "float z_over_w = -gl_FragCoord.z;\n";
        }
        self.out += "float depth = z_over_w * depth_scale + depth_offset;\n";
        if self.config.framebuffer.depthmap_enable() == DepthBuffering::W_BUFFERING {
            self.out += "depth /= gl_FragCoord.w;\n";
        }
    }

    /// Emits code to emulate the scissor rectangle
    fn write_scissor(&mut self) {
        let scissor_mode = self.config.framebuffer.scissor_test_mode();
        if scissor_mode == ScissorMode::DISABLED {
            return;
        }

        self.out += "if (";
        // Negate the condition if we have to keep only the pixels outside the scissor box
        if scissor_mode == ScissorMode::INCLUDE {
            self.out.push('!');
        }
        self.out += "(gl_FragCoord.x >= float(scissor_x1) && \
                     gl_FragCoord.y >= float(scissor_y1) && \
                     gl_FragCoord.x < float(scissor_x2) && \
                     gl_FragCoord.y < float(scissor_y2))) discard;\n";
    }

    /// Writes the code to emulate fog
    fn write_fog(&mut self) {
        // Get index into fog LUT
        if self.config.texture.fog_flip() != 0 {
            self.out += "float fog_index = (1.0 - float(depth)) * 128.0;\n";
        } else {
            self.out += "float fog_index = depth * 128.0;\n";
        }

        // Generate clamped fog factor from LUT for given fog index
        self.out += "float fog_i = clamp(floor(fog_index), 0.0, 127.0);\n\
                     float fog_f = fog_index - fog_i;\n\
                     vec2 fog_lut_entry = texelFetch(texture_buffer_lut_lf, int(fog_i) + \
                     fog_lut_offset).rg;\n\
                     float fog_factor = fog_lut_entry.r + fog_lut_entry.g * fog_f;\n\
                     fog_factor = clamp(fog_factor, 0.0, 1.0);\n";

        // Blend the fog
        self.out += "combiner_output.rgb = mix(fog_color.rgb, combiner_output.rgb, fog_factor);\n";
    }

    /// Writes the code to emulate gas rendering
    fn write_gas(&mut self) {
        // TODO: Implement me
        log::error!(target: "Render", "Unimplemented gas mode");
        self.out += "discard; }";
    }

    /// Writes the code to emulate shadow-map rendering
    fn write_shadow(&mut self) {
        self.out += r#"
uint d = uint(clamp(depth, 0.0, 1.0) * float(0xFFFFFF));
uint s = uint(combiner_output.g * float(0xFF));
ivec2 image_coord = ivec2(gl_FragCoord.xy);
"#;

        if self.use_fragment_shader_interlock {
            self.out += r#"
beginInvocationInterlock();
uint old_shadow = imageLoad(shadow_buffer, image_coord).x;
uint new_shadow = UpdateShadow(old_shadow, d, s);
imageStore(shadow_buffer, image_coord, uvec4(new_shadow));
endInvocationInterlock();
"#;
        } else {
            self.out += r#"
uint old = imageLoad(shadow_buffer, image_coord).x;
uint new1;
uint old2;
do {
    old2 = old;
    new1 = UpdateShadow(old, d, s);
} while ((old = imageAtomicCompSwap(shadow_buffer, image_coord, old, new1)) != old2);
"#;
        }
    }

    /// Writes the code to emulate logic ops in the fragment shader
    fn write_logic_op(&mut self) {
        let logic_op = self.config.framebuffer.logic_op();
        match logic_op {
            LogicOp::CLEAR => {
                self.out += "color = vec4(0);\n";
            }
            LogicOp::SET => {
                self.out += "color = vec4(1);\n";
            }
            LogicOp::COPY => {
                // Take the color output as-is
            }
            LogicOp::COPY_INVERTED => {
                self.out += "color = ~color;\n";
            }
            LogicOp::NO_OP => {
                // We need to discard the color, but not necessarily the depth. This is not possible
                // with fragment shader alone, so we emulate this behavior on GLES with glColorMask.
            }
            _ => {
                log::error!(target: "HW_GPU", "Unhandled logic_op {:x}", logic_op.0);
                log::error!(target: "Debug", "Unimplemented code!");
            }
        }
    }

    /// Writes the code to emulate PICA min/max blending factors
    fn write_blending(&mut self) {
        if !self.config.emulate_blend() || self.profile.is_vulkan {
            return;
        }

        self.out += "vec4 source_color = combiner_output;\n";
        self.out += "vec4 dest_color = destFactor;\n";
        let get_factor = |factor: BlendFactor| -> &'static str {
            match factor {
                BlendFactor::ZERO => "vec4(0.f)",
                BlendFactor::ONE => "vec4(1.f)",
                BlendFactor::SOURCE_COLOR => "source_color",
                BlendFactor::ONE_MINUS_SOURCE_COLOR => "vec4(1.f) - source_color",
                BlendFactor::DEST_COLOR => "dest_color",
                BlendFactor::ONE_MINUS_DEST_COLOR => "vec4(1.f) - dest_color",
                BlendFactor::SOURCE_ALPHA => "source_color.aaaa",
                BlendFactor::ONE_MINUS_SOURCE_ALPHA => "vec4(1.f) - source_color.aaaa",
                BlendFactor::DEST_ALPHA => "dest_color.aaaa",
                BlendFactor::ONE_MINUS_DEST_ALPHA => "vec4(1.f) - dest_color.aaaa",
                BlendFactor::CONSTANT_COLOR => "blend_color",
                BlendFactor::ONE_MINUS_CONSTANT_COLOR => "vec4(1.f) - blend_color",
                BlendFactor::CONSTANT_ALPHA => "blend_color.aaaa",
                BlendFactor::ONE_MINUS_CONSTANT_ALPHA => "vec4(1.f) - blend_color.aaaa",
                _ => {
                    log::error!(target: "Render_OpenGL", "Unknown blend factor {}", factor.0);
                    "vec4(1.f)"
                }
            }
        };
        let get_func = |eq: BlendEquation| {
            if eq == BlendEquation::MIN {
                "min"
            } else {
                "max"
            }
        };

        let rgb_blend = self.config.framebuffer.rgb_blend;
        if rgb_blend.eq != BlendEquation::ADD {
            self.out += &format!(
                "combiner_output.rgb = {}(source_color.rgb * ({}).rgb, dest_color.rgb * ({}).rgb);\n",
                get_func(rgb_blend.eq),
                get_factor(rgb_blend.src_factor),
                get_factor(rgb_blend.dst_factor)
            );
        }
        let alpha_blend = self.config.framebuffer.alpha_blend;
        if alpha_blend.eq != BlendEquation::ADD {
            self.out += &format!(
                "combiner_output.a = {}(source_color.a * ({}).a, dest_color.a * ({}).a);\n",
                get_func(alpha_blend.eq),
                get_factor(alpha_blend.src_factor),
                get_factor(alpha_blend.dst_factor)
            );
        }
    }
}

/**
 * Generates the GLSL fragment shader program source code for the current Pica state
 * @param config ShaderCacheKey object generated for the current Pica state, used for the shader
 *               configuration (NOTE: Use state in this struct only, not the Pica registers!)
 * @returns String of the shader source code
 */
pub fn generate_fragment_shader(config: &FSConfig, profile: &Profile) -> String {
    let module = FragmentModule::new(config, profile);
    module.generate()
}
