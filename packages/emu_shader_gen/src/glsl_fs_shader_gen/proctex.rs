use emu_pica_types::regs::texturing::{ProcTexClamp, ProcTexCombiner, ProcTexFilter, ProcTexShift};

use super::FragmentModule;

impl FragmentModule<'_> {
    fn append_proctex_shift_offset(
        &mut self,
        v: &str,
        mode: ProcTexShift,
        clamp_mode: ProcTexClamp,
    ) {
        let offset = if clamp_mode == ProcTexClamp::MIRRORED_REPEAT {
            "1.0"
        } else {
            "0.5"
        };
        match mode {
            ProcTexShift::NONE => {
                self.out += "0.0";
            }
            ProcTexShift::ODD => {
                self.out += &format!("{} * float((int({}) / 2) % 2)", offset, v);
            }
            ProcTexShift::EVEN => {
                self.out += &format!("{} * float(((int({}) + 1) / 2) % 2)", offset, v);
            }
            _ => {
                log::error!(target: "HW_GPU", "Unknown shift mode {}", mode.0);
                self.out += "0.0";
            }
        }
    }

    fn append_proctex_clamp(&mut self, var: &str, mode: ProcTexClamp) {
        match mode {
            ProcTexClamp::TO_ZERO => {
                self.out += &format!("{0} = {0} > 1.0 ? 0 : {0};\n", var);
            }
            ProcTexClamp::TO_EDGE => {
                self.out += &format!("{0} = min({0}, 1.0);\n", var);
            }
            ProcTexClamp::SYMMETRICAL_REPEAT => {
                self.out += &format!("{0} = fract({0});\n", var);
            }
            ProcTexClamp::MIRRORED_REPEAT => {
                self.out += &format!(
                    "{0} = int({0}) % 2 == 0 ? fract({0}) : 1.0 - fract({0});\n",
                    var
                );
            }
            ProcTexClamp::PULSE => {
                self.out += &format!("{0} = {0} > 0.5 ? 1.0 : 0.0;\n", var);
            }
            _ => {
                log::error!(target: "HW_GPU", "Unknown clamp mode {}", mode.0);
                self.out += &format!("{0} = min({0}, 1.0);\n", var);
            }
        }
    }

    fn append_proctex_combine_and_map(&mut self, combiner: ProcTexCombiner, offset: &str) {
        let combined = match combiner {
            ProcTexCombiner::U => "u",
            ProcTexCombiner::U2 => "(u * u)",
            ProcTexCombiner::V => "v",
            ProcTexCombiner::V2 => "(v * v)",
            ProcTexCombiner::ADD => "((u + v) * 0.5)",
            ProcTexCombiner::ADD2 => "((u * u + v * v) * 0.5)",
            ProcTexCombiner::SQRT_ADD2 => "min(sqrt(u * u + v * v), 1.0)",
            ProcTexCombiner::MIN => "min(u, v)",
            ProcTexCombiner::MAX => "max(u, v)",
            ProcTexCombiner::R_MAX => "min(((u + v) * 0.5 + sqrt(u * u + v * v)) * 0.5, 1.0)",
            _ => {
                log::error!(target: "HW_GPU", "Unknown combiner {}", combiner.0);
                "0.0"
            }
        };
        self.out += &format!("ProcTexLookupLUT({}, {})", offset, combined);
    }

    pub(super) fn define_proctex_sampler(&mut self) {
        let proctex = self.config.proctex;
        if proctex.enable() == 0 {
            return;
        }

        // LUT sampling uitlity
        // For NoiseLUT/ColorMap/AlphaMap, coord=0.0 is lut[0], coord=127.0/128.0 is lut[127] and
        // coord=1.0 is lut[127]+lut_diff[127]. For other indices, the result is interpolated using
        // value entries and difference entries.
        self.out += r#"
float ProcTexLookupLUT(int offset, float coord) {
    coord *= 128.0;
    float index_i = clamp(floor(coord), 0.0, 127.0);
    float index_f = coord - index_i; // fract() cannot be used here because 128.0 needs to be
                                     // extracted as index_i = 127.0 and index_f = 1.0
    vec2 entry = texelFetch(texture_buffer_lut_rg, int(index_i) + offset).rg;
    return clamp(entry.r + entry.g * index_f, 0.0, 1.0);
}
    "#;

        // Noise utility
        if proctex.noise_enable() != 0 {
            // See swrasterizer/proctex.cpp for more information about these functions
            self.out += r#"
int ProcTexNoiseRand1D(int v) {
    const int table[] = int[](0,4,10,8,4,9,7,12,5,15,13,14,11,15,2,11);
    return ((v % 9 + 2) * 3 & 0xF) ^ table[(v / 9) & 0xF];
}

float ProcTexNoiseRand2D(vec2 point) {
    const int table[] = int[](10,2,15,8,0,7,4,5,5,13,2,6,13,9,3,14);
    int u2 = ProcTexNoiseRand1D(int(point.x));
    int v2 = ProcTexNoiseRand1D(int(point.y));
    v2 += ((u2 & 3) == 1) ? 4 : 0;
    v2 ^= (u2 & 1) * 6;
    v2 += 10 + u2;
    v2 &= 0xF;
    v2 ^= table[u2];
    return -1.0 + float(v2) * (2.0/15.0);
}

float ProcTexNoiseCoef(vec2 x) {
    vec2 grid  = 9.0 * proctex_noise_f * abs(x + proctex_noise_p);
    vec2 point = floor(grid);
    vec2 frac  = grid - point;

    float g0 = ProcTexNoiseRand2D(point) * (frac.x + frac.y);
    float g1 = ProcTexNoiseRand2D(point + vec2(1.0, 0.0)) * (frac.x + frac.y - 1.0);
    float g2 = ProcTexNoiseRand2D(point + vec2(0.0, 1.0)) * (frac.x + frac.y - 1.0);
    float g3 = ProcTexNoiseRand2D(point + vec2(1.0, 1.0)) * (frac.x + frac.y - 2.0);

    float x_noise = ProcTexLookupLUT(proctex_noise_lut_offset, frac.x);
    float y_noise = ProcTexLookupLUT(proctex_noise_lut_offset, frac.y);
    float x0 = mix(g0, g1, x_noise);
    float x1 = mix(g2, g3, x_noise);
    return mix(x0, x1, y_noise);
}
        "#;
        }

        self.out += "vec4 SampleProcTexColor(float lut_coord, int level) {\n";
        self.out += &format!("int lut_width = {} >> level;\n", proctex.lut_width);
        // Offsets for level 4-7 seem to be hardcoded
        self.out += &format!(
            "int lut_offsets[8] = int[]({}, {}, {}, {}, 0xF0, 0xF8, 0xFC, 0xFE);\n",
            proctex.lut_offset0, proctex.lut_offset1, proctex.lut_offset2, proctex.lut_offset3
        );
        self.out += "int lut_offset = lut_offsets[level];\n";
        // For the color lut, coord=0.0 is lut[offset] and coord=1.0 is lut[offset+width-1]
        self.out += "lut_coord *= float(lut_width - 1);\n";

        match proctex.lut_filter() {
            ProcTexFilter::LINEAR
            | ProcTexFilter::LINEAR_MIPMAP_LINEAR
            | ProcTexFilter::LINEAR_MIPMAP_NEAREST => {
                self.out += "int lut_index_i = int(lut_coord) + lut_offset;\n";
                self.out += "float lut_index_f = fract(lut_coord);\n";
                self.out += "return texelFetch(texture_buffer_lut_rgba, lut_index_i + \
                             proctex_lut_offset) + \
                             lut_index_f * \
                             texelFetch(texture_buffer_lut_rgba, lut_index_i + proctex_diff_lut_offset);\n";
            }
            ProcTexFilter::NEAREST
            | ProcTexFilter::NEAREST_MIPMAP_LINEAR
            | ProcTexFilter::NEAREST_MIPMAP_NEAREST => {
                self.out += "lut_coord += float(lut_offset);\n";
                self.out += "return texelFetch(texture_buffer_lut_rgba, int(round(lut_coord)) + \
                             proctex_lut_offset);\n";
            }
            _ => {}
        }

        self.out += "}\n";

        self.out += "vec4 ProcTex() {\n";
        if proctex.coord() < 3 {
            self.out += &format!("vec2 uv = abs(texcoord{});\n", proctex.coord());
        } else {
            log::error!(target: "Render", "Unexpected proctex.coord >= 3");
            self.out += "vec2 uv = abs(texcoord0);\n";
        }

        // This LOD formula is the same as the LOD upper limit defined in OpenGL.
        // f(x, y) <= m_u + m_v + m_w
        // (See OpenGL 4.6 spec, 8.14.1 - Scale Factor and Level-of-Detail)
        // Note: this is different from the one normal 2D textures use.
        self.out += "vec2 duv = max(abs(dFdx(uv)), abs(dFdy(uv)));\n";
        // unlike normal texture, the bias is inside the log2
        self.out += &format!(
            "float lod = log2(abs(float({}) * proctex_bias) * (duv.x + duv.y));\n",
            proctex.lut_width
        );
        self.out += "if (proctex_bias == 0.0) lod = 0.0;\n";
        self.out += &format!(
            "lod = clamp(lod, {:?}, {:?});\n",
            f32::from(proctex.lod_min).max(0.0),
            f32::from(proctex.lod_max).min(7.0)
        );

        // Get shift offset before noise generation
        self.out += "float u_shift = ";
        self.append_proctex_shift_offset("uv.y", proctex.u_shift(), proctex.u_clamp());
        self.out += ";\n";
        self.out += "float v_shift = ";
        self.append_proctex_shift_offset("uv.x", proctex.v_shift(), proctex.v_clamp());
        self.out += ";\n";

        // Generate noise
        if proctex.noise_enable() != 0 {
            self.out += "uv += proctex_noise_a * ProcTexNoiseCoef(uv);\n\
                         uv = abs(uv);\n";
        }

        // Shift
        self.out += "float u = uv.x + u_shift;\n\
                     float v = uv.y + v_shift;\n";

        // Clamp
        self.append_proctex_clamp("u", proctex.u_clamp());
        self.append_proctex_clamp("v", proctex.v_clamp());

        // Combine and map
        self.out += "float lut_coord = ";
        self.append_proctex_combine_and_map(proctex.color_combiner(), "proctex_color_map_offset");
        self.out += ";\n";

        match proctex.lut_filter() {
            ProcTexFilter::LINEAR | ProcTexFilter::NEAREST => {
                self.out += "vec4 final_color = SampleProcTexColor(lut_coord, 0);\n";
            }
            ProcTexFilter::NEAREST_MIPMAP_NEAREST | ProcTexFilter::LINEAR_MIPMAP_NEAREST => {
                self.out += "vec4 final_color = SampleProcTexColor(lut_coord, int(round(lod)));\n";
            }
            ProcTexFilter::NEAREST_MIPMAP_LINEAR | ProcTexFilter::LINEAR_MIPMAP_LINEAR => {
                self.out += "int lod_i = int(lod);\n\
                             float lod_f = fract(lod);\n\
                             vec4 final_color = mix(SampleProcTexColor(lut_coord, lod_i), \
                             SampleProcTexColor(lut_coord, lod_i + 1), lod_f);\n";
            }
            _ => {}
        }

        if proctex.separate_alpha() != 0 {
            // Note: in separate alpha mode, the alpha channel skips the color LUT look up stage. It
            // uses the output of CombineAndMap directly instead.
            self.out += "float final_alpha = ";
            self.append_proctex_combine_and_map(
                proctex.alpha_combiner(),
                "proctex_alpha_map_offset",
            );
            self.out += ";\n";
            self.out += "return vec4(final_color.xyz, final_alpha);\n}\n";
        } else {
            self.out += "return final_color;\n}\n";
        }
    }
}
