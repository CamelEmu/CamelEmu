use emu_pica_types::regs::lighting::{
    LightingBumpMode, LightingConfig, LightingLutInput, LightingSampler,
    distance_attenuation_sampler, is_lighting_sampler_supported, spotlight_attenuation_sampler,
};

use super::FragmentModule;

impl FragmentModule<'_> {
    /// Writes the code to emulate fragment lighting
    pub(super) fn write_lighting(&mut self) {
        let config = self.config;
        if config.lighting.enable() == 0 {
            return;
        }

        let lighting = &config.lighting;

        // Define lighting globals
        self.out += "vec4 diffuse_sum = vec4(0.0, 0.0, 0.0, 1.0);\n\
                     vec4 specular_sum = vec4(0.0, 0.0, 0.0, 1.0);\n\
                     vec3 light_vector = vec3(0.0);\n\
                     float light_distance = 0.0;\n\
                     vec3 refl_value = vec3(0.0);\n\
                     vec3 spot_dir = vec3(0.0);\n\
                     vec3 half_vector = vec3(0.0);\n\
                     float dot_product = 0.0;\n\
                     float clamp_highlights = 1.0;\n\
                     float geo_factor = 1.0;\n";

        // Compute fragment normals and tangents
        let perturbation = || {
            format!(
                "2.0 * (sampleTexUnit{}()).rgb - 1.0",
                lighting.bump_selector()
            )
        };

        if config.user.use_custom_normal() != 0 {
            let texel = "2.0 * (texture(tex_normal, texcoord0)).rgb - 1.0".to_string();
            self.out += &format!("vec3 surface_normal = {};\n", texel);
            self.out += "vec3 surface_tangent = vec3(1.0, 0.0, 0.0);\n";
        } else {
            match lighting.bump_mode() {
                LightingBumpMode::NORMAL_MAP => {
                    // Bump mapping is enabled using a normal map
                    self.out += &format!("vec3 surface_normal = {};\n", perturbation());

                    // Recompute Z-component of perturbation if 'renorm' is enabled, this provides a higher
                    // precision result
                    if lighting.bump_renorm() != 0 {
                        const VAL: &str = "(1.0 - (surface_normal.x*surface_normal.x + \
                                           surface_normal.y*surface_normal.y))";
                        self.out += &format!("surface_normal.z = sqrt(max({}, 0.0));\n", VAL);
                    }

                    // The tangent vector is not perturbed by the normal map and is just a unit vector.
                    self.out += "vec3 surface_tangent = vec3(1.0, 0.0, 0.0);\n";
                }
                LightingBumpMode::TANGENT_MAP => {
                    // Bump mapping is enabled using a tangent map
                    self.out += &format!("vec3 surface_tangent = {};\n", perturbation());
                    // Mathematically, recomputing Z-component of the tangent vector won't affect the
                    // relevant computation below, which is also confirmed on 3DS. So we don't bother
                    // recomputing here even if 'renorm' is enabled.

                    // The normal vector is not perturbed by the tangent map and is just a unit vector.
                    self.out += "vec3 surface_normal = vec3(0.0, 0.0, 1.0);\n";
                }
                _ => {
                    // No bump mapping - surface local normal and tangent are just unit vectors
                    self.out += "vec3 surface_normal = vec3(0.0, 0.0, 1.0);\n\
                                 vec3 surface_tangent = vec3(1.0, 0.0, 0.0);\n";
                }
            }
        }

        // If the barycentric extension is enabled, perform quaternion correction here.
        if self.use_fragment_shader_barycentric {
            self.out += "vec4 normquat_0 = normquats[0];\n\
                         vec4 normquat_1 = mix(normquats[1], -normquats[1], \
                         bvec4(AreQuaternionsOpposite(normquats[0], normquats[1])));\n\
                         vec4 normquat_2 = mix(normquats[2], -normquats[2], \
                         bvec4(AreQuaternionsOpposite(normquats[0], normquats[2])));\n\
                         vec4 normquat = gl_BaryCoord.x * normquat_0 + gl_BaryCoord.y * normquat_1 + \
                         gl_BaryCoord.z * normquat_2;\n";
        }

        // Rotate the surface-local normal by the interpolated normal quaternion to convert it to
        // eyespace.
        self.out += "vec4 normalized_normquat = normalize(normquat);\n\
                     vec3 normal = quaternion_rotate(normalized_normquat, surface_normal);\n\
                     vec3 tangent = quaternion_rotate(normalized_normquat, surface_tangent);\n";

        if lighting.enable_shadow() != 0 {
            let shadow_texture = format!("sampleTexUnit{}()", lighting.shadow_selector());
            if lighting.shadow_invert() != 0 {
                self.out += &format!("vec4 shadow = vec4(1.0) - {};\n", shadow_texture);
            } else {
                self.out += &format!("vec4 shadow = {};\n", shadow_texture);
            }
        } else {
            self.out += "vec4 shadow = vec4(1.0);\n";
        }

        // Samples the specified lookup table for specular lighting
        let get_lut_value = |sampler: LightingSampler,
                             light_num: u32,
                             input: LightingLutInput,
                             abs: bool|
         -> String {
            let mut index = match input {
                LightingLutInput::NH => "dot(normal, normalize(half_vector))".to_string(),
                LightingLutInput::VH => "dot(normalize(view), normalize(half_vector))".to_string(),
                LightingLutInput::NV => "dot(normal, normalize(view))".to_string(),
                LightingLutInput::LN => "dot(light_vector, normal)".to_string(),
                LightingLutInput::SP => "dot(light_vector, spot_dir)".to_string(),
                LightingLutInput::CP => {
                    // CP input is only available with configuration 7
                    if lighting.config() == LightingConfig::CONFIG7 {
                        // Note: even if the normal vector is modified by normal map, which is not the
                        // normal of the tangent plane anymore, the half angle vector is still projected
                        // using the modified normal vector.
                        const HALF_ANGLE_PROJ: &str =
                            "normalize(half_vector) - normal * dot(normal, normalize(half_vector))";
                        // Note: the half angle vector projection is confirmed not normalized before the dot
                        // product. The result is in fact not cos(phi) as the name suggested.
                        format!("dot({}, tangent)", HALF_ANGLE_PROJ)
                    } else {
                        "0.0".to_string()
                    }
                }
                _ => {
                    log::error!(target: "HW_GPU", "Unknown lighting LUT input {}", input.0 as i32);
                    log::error!(target: "Debug", "Unimplemented code!");
                    "0.0".to_string()
                }
            };

            let sampler_index = sampler.0;

            if abs {
                // LUT index is in the range of (0.0, 1.0)
                index = if lighting.lights[light_num as usize].two_sided_diffuse() != 0 {
                    format!("abs({})", index)
                } else {
                    format!("max({}, 0.0)", index)
                };
                format!("LookupLightingLUTUnsigned({}, {})", sampler_index, index)
            } else {
                // LUT index is in the range of (-1.0, 1.0)
                format!("LookupLightingLUTSigned({}, {})", sampler_index, index)
            }
        };

        // Write the code to emulate each enabled light
        for light_index in 0..lighting.src_num() {
            let light_config = &lighting.lights[light_index as usize];
            let light_num = u32::from(light_config.num());
            let light_src = format!("light_src[{}]", light_num);

            // Compute light vector (directional or positional)
            if light_config.directional() != 0 {
                self.out += &format!("light_vector = {}.position;\n", light_src);
            } else {
                self.out += &format!("light_vector = {}.position + view;\n", light_src);
            }
            self.out += "light_distance = length(light_vector);\n";
            self.out += "light_vector = normalize(light_vector);\n";

            self.out += &format!("spot_dir = {}.spot_direction;\n", light_src);
            self.out += "half_vector = normalize(view) + light_vector;\n";

            // Compute dot product of light_vector and normal, adjust if lighting is one-sided or
            // two-sided
            self.out += "dot_product = ";
            self.out += if light_config.two_sided_diffuse() != 0 {
                "abs(dot(light_vector, normal));\n"
            } else {
                "max(dot(light_vector, normal), 0.0);\n"
            };

            // If enabled, clamp specular component if lighting result is zero
            if lighting.clamp_highlights() != 0 {
                self.out += "clamp_highlights = sign(dot_product);\n";
            }

            // If enabled, compute spot light attenuation value
            let mut spot_atten = "1.0".to_string();
            if light_config.spot_atten_enable() != 0
                && is_lighting_sampler_supported(
                    lighting.config(),
                    LightingSampler::SPOTLIGHT_ATTENUATION,
                )
            {
                let value = get_lut_value(
                    spotlight_attenuation_sampler(light_num),
                    light_num,
                    lighting.lut_sp.ty(),
                    lighting.lut_sp.abs_input() != 0,
                );
                spot_atten = format!("({:?} * {})", lighting.lut_sp.scale, value);
            }

            // If enabled, compute distance attenuation value
            let mut dist_atten = "1.0".to_string();
            if light_config.dist_atten_enable() != 0 {
                let index = format!(
                    "clamp({}.dist_atten_scale * light_distance \
                     + {}.dist_atten_bias, 0.0, 1.0)",
                    light_src, light_src
                );
                let sampler = distance_attenuation_sampler(light_num);
                dist_atten = format!("LookupLightingLUTUnsigned({}, {})", sampler.0, index);
            }

            if light_config.geometric_factor_0() != 0 || light_config.geometric_factor_1() != 0 {
                self.out += "geo_factor = dot(half_vector, half_vector);\n\
                             geo_factor = geo_factor == 0.0 ? 0.0 : min(\
                             dot_product / geo_factor, 1.0);\n";
            }

            // Specular 0 component
            let mut d0_lut_value = "1.0".to_string();
            if lighting.lut_d0.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::DISTRIBUTION0)
            {
                // Lookup specular "distribution 0" LUT value
                let value = get_lut_value(
                    LightingSampler::DISTRIBUTION0,
                    light_num,
                    lighting.lut_d0.ty(),
                    lighting.lut_d0.abs_input() != 0,
                );
                d0_lut_value = format!("({:?} * {})", lighting.lut_d0.scale, value);
            }
            let mut specular_0 = format!("({} * {}.specular_0)", d0_lut_value, light_src);
            if light_config.geometric_factor_0() != 0 {
                specular_0 = format!("({} * geo_factor)", specular_0);
            }

            // If enabled, lookup ReflectRed value, otherwise, 1.0 is used
            if lighting.lut_rr.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::REFLECT_RED)
            {
                let mut value = get_lut_value(
                    LightingSampler::REFLECT_RED,
                    light_num,
                    lighting.lut_rr.ty(),
                    lighting.lut_rr.abs_input() != 0,
                );
                value = format!("({:?} * {})", lighting.lut_rr.scale, value);
                self.out += &format!("refl_value.r = {};\n", value);
            } else {
                self.out += "refl_value.r = 1.0;\n";
            }

            // If enabled, lookup ReflectGreen value, otherwise, ReflectRed value is used
            if lighting.lut_rg.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::REFLECT_GREEN)
            {
                let mut value = get_lut_value(
                    LightingSampler::REFLECT_GREEN,
                    light_num,
                    lighting.lut_rg.ty(),
                    lighting.lut_rg.abs_input() != 0,
                );
                value = format!("({:?} * {})", lighting.lut_rg.scale, value);
                self.out += &format!("refl_value.g = {};\n", value);
            } else {
                self.out += "refl_value.g = refl_value.r;\n";
            }

            // If enabled, lookup ReflectBlue value, otherwise, ReflectRed value is used
            if lighting.lut_rb.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::REFLECT_BLUE)
            {
                let mut value = get_lut_value(
                    LightingSampler::REFLECT_BLUE,
                    light_num,
                    lighting.lut_rb.ty(),
                    lighting.lut_rb.abs_input() != 0,
                );
                value = format!("({:?} * {})", lighting.lut_rb.scale, value);
                self.out += &format!("refl_value.b = {};\n", value);
            } else {
                self.out += "refl_value.b = refl_value.r;\n";
            }

            // Specular 1 component
            let mut d1_lut_value = "1.0".to_string();
            if lighting.lut_d1.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::DISTRIBUTION1)
            {
                // Lookup specular "distribution 1" LUT value
                let value = get_lut_value(
                    LightingSampler::DISTRIBUTION1,
                    light_num,
                    lighting.lut_d1.ty(),
                    lighting.lut_d1.abs_input() != 0,
                );
                d1_lut_value = format!("({:?} * {})", lighting.lut_d1.scale, value);
            }
            let mut specular_1 =
                format!("({} * refl_value * {}.specular_1)", d1_lut_value, light_src);
            if light_config.geometric_factor_1() != 0 {
                specular_1 = format!("({} * geo_factor)", specular_1);
            }

            // Fresnel
            // Note: only the last entry in the light slots applies the Fresnel factor
            if light_index == lighting.src_num() - 1
                && lighting.lut_fr.enable() != 0
                && is_lighting_sampler_supported(lighting.config(), LightingSampler::FRESNEL)
            {
                // Lookup fresnel LUT value
                let mut value = get_lut_value(
                    LightingSampler::FRESNEL,
                    light_num,
                    lighting.lut_fr.ty(),
                    lighting.lut_fr.abs_input() != 0,
                );
                value = format!("({:?} * {})", lighting.lut_fr.scale, value);

                // Enabled for diffuse lighting alpha component
                if lighting.enable_primary_alpha() != 0 {
                    self.out += &format!("diffuse_sum.a = {};\n", value);
                }

                // Enabled for the specular lighting alpha component
                if lighting.enable_secondary_alpha() != 0 {
                    self.out += &format!("specular_sum.a = {};\n", value);
                }
            }

            let shadow_primary_enable =
                lighting.shadow_primary() != 0 && light_config.shadow_enable() != 0;
            let shadow_secondary_enable =
                lighting.shadow_secondary() != 0 && light_config.shadow_enable() != 0;
            let shadow_primary = if shadow_primary_enable {
                " * shadow.rgb"
            } else {
                ""
            };
            let shadow_secondary = if shadow_secondary_enable {
                " * shadow.rgb"
            } else {
                ""
            };

            // Compute primary fragment color (diffuse lighting) function
            self.out += &format!(
                "diffuse_sum.rgb += (({}.diffuse * dot_product{}) + {}.ambient) * {} * {};\n",
                light_src, shadow_primary, light_src, dist_atten, spot_atten
            );

            // Compute secondary fragment color (specular lighting) function
            self.out += &format!(
                "specular_sum.rgb += ({} + {}) * clamp_highlights * {} * {}{};\n",
                specular_0, specular_1, dist_atten, spot_atten, shadow_secondary
            );
        }

        // Apply shadow attenuation to alpha components if enabled
        if lighting.shadow_alpha() != 0 {
            if lighting.enable_primary_alpha() != 0 {
                self.out += "diffuse_sum.a *= shadow.a;\n";
            }
            if lighting.enable_secondary_alpha() != 0 {
                self.out += "specular_sum.a *= shadow.a;\n";
            }
        }

        // Sum final lighting result
        self.out += "diffuse_sum.rgb += lighting_global_ambient;\n\
                     primary_fragment_color = clamp(diffuse_sum, vec4(0.0), vec4(1.0));\n\
                     secondary_fragment_color = clamp(specular_sum, vec4(0.0), vec4(1.0));\n";
    }
}
