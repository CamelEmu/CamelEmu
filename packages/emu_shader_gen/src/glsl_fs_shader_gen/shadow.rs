use emu_pica_types::regs::texturing::TextureType;

use super::FragmentModule;

impl FragmentModule<'_> {
    pub(super) fn define_shadow_helpers(&mut self) {
        if self.config.framebuffer.shadow_rendering() != 0 {
            self.out += r#"
uint EncodeShadow(uvec2 pixel) {
    return (pixel.x << 8) | pixel.y;
}

uint UpdateShadow(uint pixel, uint d, uint s) {
    uvec2 ref = DecodeShadow(pixel);
    if (d < ref.x) {
        if (s == 0u) {
            ref.x = d;
        } else {
            s = uint(float(s) / (shadow_bias_constant + shadow_bias_linear * float(d) / float(ref.x)));
            ref.y = min(s, ref.y);
        }
    }
    return EncodeShadow(ref);
}
"#;
        }

        let texture0_type = self.config.texture.texture0_type();
        if texture0_type == TextureType::SHADOW_2D || texture0_type == TextureType::SHADOW_CUBE {
            self.out += r#"
float CompareShadow(uint pixel, uint z) {
    uvec2 p = DecodeShadow(pixel);
    return mix(float(p.y) * (1.0 / 255.0), 0.0, p.x <= z);
}

float mix2(vec4 s, vec2 a) {
    vec2 t = mix(s.xy, s.zw, a.yy);
    return mix(t.x, t.y, a.x);
}
"#;

            if texture0_type == TextureType::SHADOW_2D {
                if self.profile.is_vulkan {
                    self.out += r#"
float SampleShadow2D(ivec2 uv, uint z) {
    if (any(bvec4(lessThan(uv, ivec2(0)), greaterThanEqual(uv, textureSize(tex0, 0)))))
        return 1.0;
    return CompareShadow(texelFetch(tex0, uv, 0).x, z);
}

vec4 shadowTexture(vec2 uv, float w) {
"#;
                    if self.config.texture.shadow_texture_orthographic() == 0 {
                        self.out += "uv /= w;";
                    }
                    self.out += r#"
    uint z = uint(max(0, int(min(abs(w), 1.0) * float(0xFFFFFF)) - shadow_texture_bias));
    vec2 coord = vec2(textureSize(tex0, 0)) * uv - vec2(0.5);
    vec2 coord_floor = floor(coord);
    vec2 f = coord - coord_floor;
    ivec2 i = ivec2(coord_floor);
    vec4 s = vec4(
        SampleShadow2D(i              , z),
        SampleShadow2D(i + ivec2(1, 0), z),
        SampleShadow2D(i + ivec2(0, 1), z),
        SampleShadow2D(i + ivec2(1, 1), z));
    return vec4(mix2(s, f));
}
"#;
                } else {
                    self.out += r#"
float SampleShadow2D(ivec2 uv, uint z) {
    if (any(bvec4(lessThan(uv, ivec2(0)), greaterThanEqual(uv, imageSize(shadow_texture_px)))))
        return 1.0;
    return CompareShadow(imageLoad(shadow_texture_px, uv).x, z);
}

vec4 shadowTexture(vec2 uv, float w) {
"#;
                    if self.config.texture.shadow_texture_orthographic() == 0 {
                        self.out += "uv /= w;";
                    }
                    self.out += r#"
    uint z = uint(max(0, int(min(abs(w), 1.0) * float(0xFFFFFF)) - shadow_texture_bias));
    vec2 coord = vec2(imageSize(shadow_texture_px)) * uv - vec2(0.5);
    vec2 coord_floor = floor(coord);
    vec2 f = coord - coord_floor;
    ivec2 i = ivec2(coord_floor);
    vec4 s = vec4(
        SampleShadow2D(i              , z),
        SampleShadow2D(i + ivec2(1, 0), z),
        SampleShadow2D(i + ivec2(0, 1), z),
        SampleShadow2D(i + ivec2(1, 1), z));
    return vec4(mix2(s, f));
}
"#;
                }
            } else if texture0_type == TextureType::SHADOW_CUBE {
                if self.profile.is_vulkan {
                    self.out += r#"
uvec4 SampleShadowCube(int face, ivec2 i00, ivec2 i10, ivec2 i01, ivec2 i11) {
    return uvec4(
        texelFetch(tex0[face], i00, 0).r,
        texelFetch(tex0[face], i10, 0).r,
        texelFetch(tex0[face], i01, 0).r,
        texelFetch(tex0[face], i11, 0).r);
}

vec4 shadowTextureCube(vec2 uv, float w) {
    ivec2 size = textureSize(tex0[0], 0);
    vec3 c = vec3(uv, w);
    vec3 a = abs(c);
    if (a.x > a.y && a.x > a.z) {
        w = a.x;
        uv = -c.zy;
        if (c.x < 0.0) uv.x = -uv.x;
    } else if (a.y > a.z) {
        w = a.y;
        uv = c.xz;
        if (c.y < 0.0) uv.y = -uv.y;
    } else {
        w = a.z;
        uv = -c.xy;
        if (c.z > 0.0) uv.x = -uv.x;
    }
    uint z = uint(max(0, int(min(w, 1.0) * float(0xFFFFFF)) - shadow_texture_bias));
    vec2 coord = vec2(size) * (uv / w * vec2(0.5) + vec2(0.5)) - vec2(0.5);
    vec2 coord_floor = floor(coord);
    vec2 f = coord - coord_floor;
    ivec2 i00 = ivec2(coord_floor);
    ivec2 i10 = i00 + ivec2(1, 0);
    ivec2 i01 = i00 + ivec2(0, 1);
    ivec2 i11 = i00 + ivec2(1, 1);
    ivec2 cmin = ivec2(0), cmax = size - ivec2(1, 1);
    i00 = clamp(i00, cmin, cmax);
    i10 = clamp(i10, cmin, cmax);
    i01 = clamp(i01, cmin, cmax);
    i11 = clamp(i11, cmin, cmax);
    uvec4 pixels;
    if (a.x > a.y && a.x > a.z) {
        if (c.x > 0.0)
            pixels = SampleShadowCube(0, i00, i10, i01, i11);
        else
            pixels = SampleShadowCube(1, i00, i10, i01, i11);
    } else if (a.y > a.z) {
        if (c.y > 0.0)
            pixels = SampleShadowCube(2, i00, i10, i01, i11);
        else
            pixels = SampleShadowCube(3, i00, i10, i01, i11);
    } else {
        if (c.z > 0.0)
            pixels = SampleShadowCube(4, i00, i10, i01, i11);
        else
            pixels = SampleShadowCube(5, i00, i10, i01, i11);
    }
    vec4 s = vec4(
        CompareShadow(pixels.x, z),
        CompareShadow(pixels.y, z),
        CompareShadow(pixels.z, z),
        CompareShadow(pixels.w, z));
    return vec4(mix2(s, f));
}
    "#;
                } else {
                    self.out += r#"
vec4 shadowTextureCube(vec2 uv, float w) {
    ivec2 size = imageSize(shadow_texture_px);
    vec3 c = vec3(uv, w);
    vec3 a = abs(c);
    if (a.x > a.y && a.x > a.z) {
        w = a.x;
        uv = -c.zy;
        if (c.x < 0.0) uv.x = -uv.x;
    } else if (a.y > a.z) {
        w = a.y;
        uv = c.xz;
        if (c.y < 0.0) uv.y = -uv.y;
    } else {
        w = a.z;
        uv = -c.xy;
        if (c.z > 0.0) uv.x = -uv.x;
    }
    uint z = uint(max(0, int(min(w, 1.0) * float(0xFFFFFF)) - shadow_texture_bias));
    vec2 coord = vec2(size) * (uv / w * vec2(0.5) + vec2(0.5)) - vec2(0.5);
    vec2 coord_floor = floor(coord);
    vec2 f = coord - coord_floor;
    ivec2 i00 = ivec2(coord_floor);
    ivec2 i10 = i00 + ivec2(1, 0);
    ivec2 i01 = i00 + ivec2(0, 1);
    ivec2 i11 = i00 + ivec2(1, 1);
    ivec2 cmin = ivec2(0), cmax = size - ivec2(1, 1);
    i00 = clamp(i00, cmin, cmax);
    i10 = clamp(i10, cmin, cmax);
    i01 = clamp(i01, cmin, cmax);
    i11 = clamp(i11, cmin, cmax);
    uvec4 pixels;
    // This part should have been refactored into functions,
    // but many drivers don't like passing uimage2D as parameters
    if (a.x > a.y && a.x > a.z) {
        if (c.x > 0.0)
            pixels = uvec4(
                imageLoad(shadow_texture_px, i00).r,
                imageLoad(shadow_texture_px, i10).r,
                imageLoad(shadow_texture_px, i01).r,
                imageLoad(shadow_texture_px, i11).r);
        else
            pixels = uvec4(
                imageLoad(shadow_texture_nx, i00).r,
                imageLoad(shadow_texture_nx, i10).r,
                imageLoad(shadow_texture_nx, i01).r,
                imageLoad(shadow_texture_nx, i11).r);
    } else if (a.y > a.z) {
        if (c.y > 0.0)
            pixels = uvec4(
                imageLoad(shadow_texture_py, i00).r,
                imageLoad(shadow_texture_py, i10).r,
                imageLoad(shadow_texture_py, i01).r,
                imageLoad(shadow_texture_py, i11).r);
        else
            pixels = uvec4(
                imageLoad(shadow_texture_ny, i00).r,
                imageLoad(shadow_texture_ny, i10).r,
                imageLoad(shadow_texture_ny, i01).r,
                imageLoad(shadow_texture_ny, i11).r);
    } else {
        if (c.z > 0.0)
            pixels = uvec4(
                imageLoad(shadow_texture_pz, i00).r,
                imageLoad(shadow_texture_pz, i10).r,
                imageLoad(shadow_texture_pz, i01).r,
                imageLoad(shadow_texture_pz, i11).r);
        else
            pixels = uvec4(
                imageLoad(shadow_texture_nz, i00).r,
                imageLoad(shadow_texture_nz, i10).r,
                imageLoad(shadow_texture_nz, i01).r,
                imageLoad(shadow_texture_nz, i11).r);
    }
    vec4 s = vec4(
        CompareShadow(pixels.x, z),
        CompareShadow(pixels.y, z),
        CompareShadow(pixels.z, z),
        CompareShadow(pixels.w, z));
    return vec4(mix2(s, f));
}
    "#;
                }
            }
        }
    }
}
