use emu_pica_types::regs::rasterizer::Semantic;

use crate::glsl_shader_decompiler::{ProgramCode, SwizzleData, decompile_program};
use crate::shader_gen::{
    ATTRIBUTE_COLOR, ATTRIBUTE_NORMQUAT, ATTRIBUTE_POSITION, ATTRIBUTE_TEXCOORD0,
    ATTRIBUTE_TEXCOORD0_W, ATTRIBUTE_TEXCOORD1, ATTRIBUTE_TEXCOORD2, ATTRIBUTE_VIEW,
    AttribLoadFlags, PicaFixedGSConfig, PicaGSConfigState, PicaVSConfig, SemanticMap,
};

// High precision may or may not be supported in GLES3. If it isn't, use medium precision instead.
pub const FRAGMENT_SHADER_PRECISION_OES: &str = r#"
#if GL_ES
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp int;
precision highp float;
precision highp samplerBuffer;
precision highp uimage2D;
#else
precision mediump int;
precision mediump float;
precision mediump samplerBuffer;
precision mediump uimage2D;
#endif // GL_FRAGMENT_PRECISION_HIGH
#endif
"#;

const VS_PICA_UNIFORM_BLOCK_DEF: &str = r#"
struct pica_uniforms {
    bool b[16];
    uvec4 i[4];
    vec4 f[96];
};

#ifdef VULKAN
layout (set = 0, binding = 0, std140) uniform vs_pica_data {
#else
layout (binding = 0, std140) uniform vs_pica_data {
#endif
    pica_uniforms uniforms;
};
"#;

const VS_UNIFORM_BLOCK_DEF: &str = r#"
#ifdef VULKAN
layout (set = 0, binding = 1, std140) uniform vs_data {
#else
layout (binding = 1, std140) uniform vs_data {
#endif
    bool enable_clip1;
    bool flip_viewport;
    vec4 clip_coef;
};

const vec2 EPSILON_Z = vec2(0.000001f, -1.00001f);

vec4 SanitizeVertex(vec4 vtx_pos) {
    float ndc_z = vtx_pos.z / vtx_pos.w;
    if (ndc_z > 0.f && ndc_z < EPSILON_Z[0]) {
        vtx_pos.z = 0.f;
    }
    if (ndc_z < -1.f && ndc_z > EPSILON_Z[1]) {
        vtx_pos.z = -vtx_pos.w;
    }
    return vtx_pos;
}
"#;

fn get_vertex_interface_declaration(
    is_output: bool,
    use_clip_planes: bool,
    separable_shader: bool,
) -> String {
    let mut out = String::new();

    let mut append_variable = |var: &str, location: u32| {
        if separable_shader {
            out += &format!("layout (location={}) ", location);
        }
        out += &format!("{}{};\n", if is_output { "out " } else { "in " }, var);
    };

    append_variable("vec4 primary_color", ATTRIBUTE_COLOR);
    append_variable("vec2 texcoord0", ATTRIBUTE_TEXCOORD0);
    append_variable("vec2 texcoord1", ATTRIBUTE_TEXCOORD1);
    append_variable("vec2 texcoord2", ATTRIBUTE_TEXCOORD2);
    append_variable("float texcoord0_w", ATTRIBUTE_TEXCOORD0_W);
    append_variable("vec4 normquat", ATTRIBUTE_NORMQUAT);
    append_variable("vec3 view", ATTRIBUTE_VIEW);

    if is_output && separable_shader {
        // gl_PerVertex redeclaration is required for separate shader object
        out += "out gl_PerVertex {\n";
        // Apple Silicon GPU drivers optimize more aggressively, which can create
        // too much variance and cause visual artifacting in games like Pokemon.
        if cfg!(target_vendor = "apple") {
            out += "    invariant vec4 gl_Position;\n";
        } else {
            out += "    vec4 gl_Position;\n";
        }
        if use_clip_planes {
            out += "    float gl_ClipDistance[2];\n";
        }
        out += "};\n";
    }

    out
}

/**
 * Generates the GLSL vertex shader program source code that accepts vertices from software shader
 * and directly passes them to the fragment shader.
 * @returns String of the shader source code
 */
pub fn generate_trivial_vertex_shader(use_clip_planes: bool, separable_shader: bool) -> String {
    let mut out = String::new();
    if separable_shader {
        out += "#extension GL_ARB_separate_shader_objects : enable\n";
    }

    out += &format!(
        "layout(location = {}) in vec4 vert_position;\n\
         layout(location = {}) in vec4 vert_color;\n\
         layout(location = {}) in vec2 vert_texcoord0;\n\
         layout(location = {}) in vec2 vert_texcoord1;\n\
         layout(location = {}) in vec2 vert_texcoord2;\n\
         layout(location = {}) in float vert_texcoord0_w;\n\
         layout(location = {}) in vec4 vert_normquat;\n\
         layout(location = {}) in vec3 vert_view;\n",
        ATTRIBUTE_POSITION,
        ATTRIBUTE_COLOR,
        ATTRIBUTE_TEXCOORD0,
        ATTRIBUTE_TEXCOORD1,
        ATTRIBUTE_TEXCOORD2,
        ATTRIBUTE_TEXCOORD0_W,
        ATTRIBUTE_NORMQUAT,
        ATTRIBUTE_VIEW
    );

    out += &get_vertex_interface_declaration(true, use_clip_planes, separable_shader);
    out += VS_UNIFORM_BLOCK_DEF;

    out += r#"
void main() {
    primary_color = vert_color;
    texcoord0 = vert_texcoord0;
    texcoord1 = vert_texcoord1;
    texcoord2 = vert_texcoord2;
    texcoord0_w = vert_texcoord0_w;
    normquat = vert_normquat;
    view = vert_view;
    vec4 vtx_pos = SanitizeVertex(vert_position);
    if (flip_viewport) {
        vtx_pos.y = -vtx_pos.y;
    }
    gl_Position = vec4(vtx_pos.x, vtx_pos.y, -vtx_pos.z, vtx_pos.w);
"#;
    if use_clip_planes {
        out += r#"
        gl_ClipDistance[0] = -vtx_pos.z; // fixed PICA clipping plane z <= 0
        if (enable_clip1) {
            gl_ClipDistance[1] = dot(clip_coef, vtx_pos);
        } else {
            gl_ClipDistance[1] = 0.0;
        }
        "#;
    }

    out += "}\n";

    out
}

pub fn make_load_prefix(flag: AttribLoadFlags) -> &'static str {
    if flag.contains(AttribLoadFlags::FLOAT) {
        ""
    } else if flag.contains(AttribLoadFlags::SINT) {
        "i"
    } else if flag.contains(AttribLoadFlags::UINT) {
        "u"
    } else {
        ""
    }
}

/// The GLSL expression of a semantic, or "1.0" when no output attribute holds it.
fn semantic_expression(
    semantic_maps: &[SemanticMap; 24],
    output_attributes: u32,
    attribute_name: impl Fn(u32) -> String,
    slot_semantic: Semantic,
) -> String {
    let slot = slot_semantic.0 as usize;
    let attrib = semantic_maps[slot].attribute_index;
    let comp = semantic_maps[slot].component_index;
    if attrib < output_attributes {
        return format!(
            "{}.{}",
            attribute_name(attrib),
            ['x', 'y', 'z', 'w'][comp as usize]
        );
    }
    "1.0".to_string()
}

/// Writes the assignments of the vertex outputs from the semantics, which `EmitVtx` of the vertex
/// shader and of the geometry shader share.
fn emit_vertex_outputs(
    out: &mut String,
    use_clip_planes: bool,
    semantic: &dyn Fn(Semantic) -> String,
    quaternion: &str,
) {
    *out += &format!(
        "    vec4 vtx_pos = vec4({}, {}, {}, {});\n",
        semantic(Semantic::POSITION_X),
        semantic(Semantic::POSITION_Y),
        semantic(Semantic::POSITION_Z),
        semantic(Semantic::POSITION_W)
    );
    *out += "    vtx_pos = SanitizeVertex(vtx_pos);\n";
    *out += "    if (flip_viewport) {\n";
    *out += "        vtx_pos.y = -vtx_pos.y;\n";
    *out += "    }\n";
    *out += "    gl_Position = vec4(vtx_pos.x, vtx_pos.y, -vtx_pos.z, vtx_pos.w);\n";
    if use_clip_planes {
        *out += "    gl_ClipDistance[0] = -vtx_pos.z;\n"; // fixed PICA clipping plane z <= 0
        *out += "    if (enable_clip1) {\n";
        *out += "        gl_ClipDistance[1] = dot(clip_coef, vtx_pos);\n";
        *out += "    } else {\n";
        *out += "        gl_ClipDistance[1] = 0.0;\n";
        *out += "    }\n\n";
    }

    *out += quaternion;
    *out += &format!(
        "    vec4 vtx_color = vec4({}, {}, {}, {});\n",
        semantic(Semantic::COLOR_R),
        semantic(Semantic::COLOR_G),
        semantic(Semantic::COLOR_B),
        semantic(Semantic::COLOR_A)
    );
    *out += "    primary_color = min(abs(vtx_color), vec4(1.0));\n\n";

    *out += &format!(
        "    texcoord0 = vec2({}, {});\n",
        semantic(Semantic::TEXCOORD0_U),
        semantic(Semantic::TEXCOORD0_V)
    );
    *out += &format!(
        "    texcoord1 = vec2({}, {});\n\n",
        semantic(Semantic::TEXCOORD1_U),
        semantic(Semantic::TEXCOORD1_V)
    );

    *out += &format!("    texcoord0_w = {};\n", semantic(Semantic::TEXCOORD0_W));
    *out += &format!(
        "    view = vec3({}, {}, {});\n\n",
        semantic(Semantic::VIEW_X),
        semantic(Semantic::VIEW_Y),
        semantic(Semantic::VIEW_Z)
    );

    *out += &format!(
        "    texcoord2 = vec2({}, {});\n\n",
        semantic(Semantic::TEXCOORD2_U),
        semantic(Semantic::TEXCOORD2_V)
    );
}

fn vertex_quaternion(semantic: &dyn Fn(Semantic) -> String) -> String {
    format!(
        "    return vec4({}, {}, {}, {});\n",
        semantic(Semantic::QUATERNION_X),
        semantic(Semantic::QUATERNION_Y),
        semantic(Semantic::QUATERNION_Z),
        semantic(Semantic::QUATERNION_W)
    )
}

/**
 * Generates the GLSL vertex shader program source code for the given VS program
 * @returns String of the shader source code; empty on failure
 */
pub fn generate_vertex_shader(
    program_code: &ProgramCode,
    swizzle_data: &SwizzleData,
    config: &PicaVSConfig,
    separable_shader: bool,
) -> String {
    let mut out = String::new();
    if separable_shader {
        out += "#extension GL_ARB_separate_shader_objects : enable\n";
    }

    out += VS_PICA_UNIFORM_BLOCK_DEF;
    out += VS_UNIFORM_BLOCK_DEF;

    let state = &config.state;
    let mut used_regs = [false; 16];
    let mut get_input_reg = |reg: u32| {
        assert!(reg < 16, "Assertion Failed!");
        used_regs[reg as usize] = true;
        format!("vs_in_reg{}", reg)
    };

    let mut get_output_reg = |reg: u32| -> String {
        assert!(reg < 16, "Assertion Failed!");
        if state.output_map[reg as usize] < state.num_outputs {
            return format!("vs_out_attr{}", state.output_map[reg as usize]);
        }
        String::new()
    };

    let program_source = decompile_program(
        program_code,
        swizzle_data,
        state.main_offset,
        &mut get_input_reg,
        &mut get_output_reg,
        state.sanitize_mul,
    );

    if program_source.is_empty() {
        return String::new();
    }

    // input attributes declaration
    for (i, &used) in used_regs.iter().enumerate() {
        if used {
            let flags = state.load_flags[i];
            let prefix = make_load_prefix(flags);
            out += &format!(
                "layout(location = {0}) in {1}vec4 vs_in_typed_reg{0};\n",
                i, prefix
            );
            out += &format!("vec4 vs_in_reg{0};\n", i);
        }
    }
    out.push('\n');

    if state.use_geometry_shader {
        // output attributes declaration
        for i in 0..state.num_outputs {
            if separable_shader {
                out += &format!("layout(location = {}) ", i);
            }
            out += &format!("out vec4 vs_out_attr{};\n", i);
        }
        out += "void EmitVtx() {}\n";
    } else {
        out += &get_vertex_interface_declaration(true, state.use_clip_planes, separable_shader);

        // output attributes declaration
        for i in 0..state.num_outputs {
            out += &format!("vec4 vs_out_attr{};\n", i);
        }

        let semantic = |slot_semantic: Semantic| {
            semantic_expression(
                &state.gs_state.semantic_maps,
                state.gs_state.gs_output_attributes,
                |attrib| format!("vs_out_attr{}", attrib),
                slot_semantic,
            )
        };

        out += "vec4 GetVertexQuaternion() {\n";
        out += &vertex_quaternion(&semantic);
        out += "}\n\n";

        out += "void EmitVtx() {\n";
        emit_vertex_outputs(
            &mut out,
            state.use_clip_planes,
            &semantic,
            "    normquat = GetVertexQuaternion();\n",
        );
        out += "}\n";
    }

    out += "bool exec_shader();\n\n";

    out += "\nvoid main() {\n";
    for (i, &used) in used_regs.iter().enumerate() {
        if used {
            out += &format!("vs_in_reg{0} = vec4(vs_in_typed_reg{0});\n", i);
            if state.load_flags[i].contains(AttribLoadFlags::ZERO_W) {
                out += &format!("vs_in_reg{0}.w = 0;\n", i);
            }
        }
    }
    for i in 0..state.num_outputs {
        out += &format!("    vs_out_attr{} = vec4(0.0, 0.0, 0.0, 1.0);\n", i);
    }
    out += "\n    exec_shader();\n    EmitVtx();\n}\n\n";

    out += &program_source;

    out
}

fn get_gs_common_source(state: &PicaGSConfigState, separable_shader: bool) -> String {
    let mut out = get_vertex_interface_declaration(true, state.use_clip_planes, separable_shader);
    out += VS_UNIFORM_BLOCK_DEF;

    out.push('\n');
    for i in 0..state.vs_output_attributes {
        if separable_shader {
            out += &format!("layout(location = {}) ", i);
        }
        out += &format!("in vec4 vs_out_attr{}[];\n", i);
    }

    out += r#"
struct Vertex {
"#;
    out += &format!("    vec4 attributes[{}];\n", state.gs_output_attributes);
    out += "};\n\n";

    let semantic = |slot_semantic: Semantic| {
        semantic_expression(
            &state.semantic_maps,
            state.gs_output_attributes,
            |attrib| format!("vtx.attributes[{}]", attrib),
            slot_semantic,
        )
    };

    out += "vec4 GetVertexQuaternion(Vertex vtx) {\n";
    out += &vertex_quaternion(&semantic);
    out += "}\n\n";

    out += "void EmitVtx(Vertex vtx, bool quats_opposite) {\n";
    emit_vertex_outputs(
        &mut out,
        state.use_clip_planes,
        &semantic,
        "    vec4 vtx_quat = GetVertexQuaternion(vtx);\n    normquat = mix(vtx_quat, -vtx_quat, bvec4(quats_opposite));\n\n",
    );

    out += "    EmitVertex();\n";
    out += "}\n";

    out += r#"
bool AreQuaternionsOpposite(vec4 qa, vec4 qb) {
    return (dot(qa, qb) < 0.0);
}

void EmitPrim(Vertex vtx0, Vertex vtx1, Vertex vtx2) {
    EmitVtx(vtx0, false);
    EmitVtx(vtx1, AreQuaternionsOpposite(GetVertexQuaternion(vtx0), GetVertexQuaternion(vtx1)));
    EmitVtx(vtx2, AreQuaternionsOpposite(GetVertexQuaternion(vtx0), GetVertexQuaternion(vtx2)));
    EndPrimitive();
}
"#;

    out
}

/**
 * Generates the GLSL fixed geometry shader program source code for non-GS PICA pipeline
 * @returns String of the shader source code
 */
pub fn generate_fixed_geometry_shader(
    config: &PicaFixedGSConfig,
    separable_shader: bool,
) -> String {
    let mut out = String::new();
    if separable_shader {
        out += "#extension GL_ARB_separate_shader_objects : enable\n";
    }

    out += r#"
layout(triangles) in;
layout(triangle_strip, max_vertices = 3) out;

"#;

    out += &get_gs_common_source(&config.state, separable_shader);

    out += r#"
void main() {
    Vertex prim_buffer[3];
"#;
    for vtx in 0..3 {
        out += &format!(
            "    prim_buffer[{}].attributes = vec4[{}](",
            vtx, config.state.gs_output_attributes
        );
        for i in 0..config.state.vs_output_attributes {
            out += &format!(
                "{}vs_out_attr{}[{}]",
                if i == 0 { "" } else { ", " },
                i,
                vtx
            );
        }
        out += ");\n";
    }
    out += "    EmitPrim(prim_buffer[0], prim_buffer[1], prim_buffer[2]);\n";
    out += "}\n";

    out
}
