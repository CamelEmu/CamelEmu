// Copyright 2025 Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <algorithm>
#include <array>
#include <cmath>
#include <cstring>
#include <memory>
#include <tuple>
#include "common/quaternion.h"
#include "core/core.h"
#include "core/memory.h"
#include "video_core/capture/frame_capture.h"
#include "video_core/gpu.h"
#include "video_core/pica/pica_core.h"
#include "video_core/renderer_software/sw_lighting.h"
#include "video_core/renderer_software/sw_rasterizer.h"

namespace VideoCore {

namespace {

struct CaptureSettings {
    u32 program_words = 512;
    u32 swizzle_words = 128;
    u32 float_uniforms = 96;
    u32 output_registers = 7;
    u32 screens = 2;
    u32 max_frames = 8;
    u32 max_texture_extent = 4096;
    u32 no_texture = 0xffffffff;
    u32 wrap_clamp_to_edge = 0;
    u32 alpha_opaque = 0;
    u32 alpha_mask = 1;
    u32 alpha_blend = 2;
    f32 min_triangle_area = 0.000001f;
    f32 inside_tolerance = 0.0001f;
    f32 color_scale = 255.0f;
};

const CaptureSettings kSettings;

class Writer {
public:
    void U32(u32 value) {
        Bytes(&value, sizeof(value));
    }

    void F32(f32 value) {
        Bytes(&value, sizeof(value));
    }

    void Bytes(const void* data, std::size_t size) {
        const auto* bytes = static_cast<const u8*>(data);
        out.insert(out.end(), bytes, bytes + size);
    }

    std::vector<u8> out;
};

struct Vertex {
    f32 x{};
    f32 y{};
    f32 z{};
    f32 w{};
    f32 u{};
    f32 v{};
    f32 r{};
    f32 g{};
    f32 b{};
    f32 a{};
};

struct Texture {
    s32 origin_s{};
    s32 top_t{};
    u32 width{};
    u32 height{};
    u32 source_width{};
    u32 source_height{};
    u32 magnify_linear{};
    u32 minify_linear{};
    u32 baked_pixels{};
    std::vector<u8> rgba;

    bool Reserve(s32 s0, s32 t0, s32 s1, s32 t1) {
        if (width == 0) {
            if (!Fits(s0, t0, s1, t1)) {
                return false;
            }
            origin_s = s0;
            top_t = t1;
            width = static_cast<u32>(s1 - s0);
            height = static_cast<u32>(t1 - t0);
            rgba.assign(std::size_t{width} * height * 4, 0);
            return true;
        }
        const s32 new_s0 = std::min(origin_s, s0);
        const s32 new_s1 = std::max(origin_s + static_cast<s32>(width), s1);
        const s32 new_t0 = std::min(top_t - static_cast<s32>(height), t0);
        const s32 new_t1 = std::max(top_t, t1);
        if (new_s0 == origin_s && new_s1 == origin_s + static_cast<s32>(width) &&
            new_t1 == top_t && new_t0 == top_t - static_cast<s32>(height)) {
            return true;
        }
        if (!Fits(new_s0, new_t0, new_s1, new_t1)) {
            return false;
        }
        const u32 new_width = static_cast<u32>(new_s1 - new_s0);
        const u32 new_height = static_cast<u32>(new_t1 - new_t0);
        std::vector<u8> grown(std::size_t{new_width} * new_height * 4, 0);
        for (u32 row = 0; row < height; ++row) {
            const s32 t = top_t - 1 - static_cast<s32>(row);
            const u32 new_row = static_cast<u32>(new_t1 - 1 - t);
            const u32 new_col = static_cast<u32>(origin_s - new_s0);
            std::memcpy(&grown[(std::size_t{new_row} * new_width + new_col) * 4],
                        &rgba[std::size_t{row} * width * 4], std::size_t{width} * 4);
        }
        origin_s = new_s0;
        top_t = new_t1;
        width = new_width;
        height = new_height;
        rgba = std::move(grown);
        return true;
    }

    u8* Pixel(s32 s, s32 t) {
        const u32 col = static_cast<u32>(s - origin_s);
        const u32 row = static_cast<u32>(top_t - 1 - t);
        return &rgba[(std::size_t{row} * width + col) * 4];
    }

private:
    static bool Fits(s32 s0, s32 t0, s32 s1, s32 t1) {
        return s1 > s0 && t1 > t0 && static_cast<u32>(s1 - s0) <= kSettings.max_texture_extent &&
               static_cast<u32>(t1 - t0) <= kSettings.max_texture_extent;
    }
};

struct Draw {
    PAddr color_address{};
    u32 sequence{};
    u32 shader{};
    u32 texture{kSettings.no_texture};
    u32 position_output{};
    u32 alpha_mode{};
    f32 alpha_cutoff{};
    std::vector<Vertex> vertices;
};

struct Transfer {
    PAddr input_address{};
    u32 sequence{};
    u32 screen{};
    u32 screen_width{};
    u32 flip_vertically{};
};

struct Resolved {
    u32 screen_width{};
    u32 flip_vertically{};
    std::vector<const Draw*> draws;
};

struct Program {
    std::vector<u32> code;
    std::vector<u32> swizzle;
    u32 entry_point{};
    std::vector<f32> uniforms;

    bool operator==(const Program& other) const {
        return code == other.code && swizzle == other.swizzle &&
               entry_point == other.entry_point && uniforms == other.uniforms;
    }
};

class FragmentShader {
public:
    FragmentShader(Memory::MemorySystem& memory, Pica::PicaCore& pica_)
        : pica{pica_}, rasterizer{memory, pica_} {}

    Common::Vec4<u8> Shade(const Pica::OutputVertex& v0, const Pica::OutputVertex& v1,
                           const Pica::OutputVertex& v2, f32 l0, f32 l1, f32 l2) {
        using Pica::f24;
        const auto& regs = pica.regs.internal;
        const auto mix = [&](f24 a, f24 b, f24 c) {
            return f24::FromFloat32(l0 * a.ToFloat32() + l1 * b.ToFloat32() + l2 * c.ToFloat32());
        };
        const auto channel = [&](f24 a, f24 b, f24 c) {
            const f32 value = mix(a, b, c).ToFloat32() * kSettings.color_scale;
            return static_cast<u8>(std::clamp(std::round(value), 0.0f, kSettings.color_scale));
        };

        const Common::Vec4<u8> primary_color{
            channel(v0.color.r(), v1.color.r(), v2.color.r()),
            channel(v0.color.g(), v1.color.g(), v2.color.g()),
            channel(v0.color.b(), v1.color.b(), v2.color.b()),
            channel(v0.color.a(), v1.color.a(), v2.color.a()),
        };

        std::array<Common::Vec2<f24>, 3> uv;
        uv[0].u() = mix(v0.tc0.u(), v1.tc0.u(), v2.tc0.u());
        uv[0].v() = mix(v0.tc0.v(), v1.tc0.v(), v2.tc0.v());
        uv[1].u() = mix(v0.tc1.u(), v1.tc1.u(), v2.tc1.u());
        uv[1].v() = mix(v0.tc1.v(), v1.tc1.v(), v2.tc1.v());
        uv[2].u() = mix(v0.tc2.u(), v1.tc2.u(), v2.tc2.u());
        uv[2].v() = mix(v0.tc2.v(), v1.tc2.v(), v2.tc2.v());

        const f24 tc0_w = mix(v0.tc0_w, v1.tc0_w, v2.tc0_w);
        const auto textures = regs.texturing.GetTextures();
        const auto tev_stages = regs.texturing.GetTevStages();
        const auto texture_color = rasterizer.TextureColor(uv, textures, tc0_w);

        Common::Vec4<u8> primary_fragment_color = {0, 0, 0, 0};
        Common::Vec4<u8> secondary_fragment_color = {0, 0, 0, 0};
        if (!regs.lighting.disable) {
            const auto normquat =
                Common::Quaternion<f32>{
                    {mix(v0.quat.x, v1.quat.x, v2.quat.x).ToFloat32(),
                     mix(v0.quat.y, v1.quat.y, v2.quat.y).ToFloat32(),
                     mix(v0.quat.z, v1.quat.z, v2.quat.z).ToFloat32()},
                    mix(v0.quat.w, v1.quat.w, v2.quat.w).ToFloat32(),
                }
                    .Normalized();
            const Common::Vec3f view{
                mix(v0.view.x, v1.view.x, v2.view.x).ToFloat32(),
                mix(v0.view.y, v1.view.y, v2.view.y).ToFloat32(),
                mix(v0.view.z, v1.view.z, v2.view.z).ToFloat32(),
            };
            std::tie(primary_fragment_color, secondary_fragment_color) =
                SwRenderer::ComputeFragmentsColors(regs.lighting, pica.lighting, normquat, view,
                                                   texture_color);
        }

        return rasterizer.WriteTevConfig(texture_color, tev_stages, primary_color,
                                         primary_fragment_color, secondary_fragment_color);
    }

    bool Bake(Texture& image, const Pica::OutputVertex& v0, const Pica::OutputVertex& v1,
              const Pica::OutputVertex& v2) {
        const f32 width = static_cast<f32>(image.source_width);
        const f32 height = static_cast<f32>(image.source_height);
        const std::array<std::array<f32, 2>, 3> corners = {{
            {v0.tc0.u().ToFloat32() * width, v0.tc0.v().ToFloat32() * height},
            {v1.tc0.u().ToFloat32() * width, v1.tc0.v().ToFloat32() * height},
            {v2.tc0.u().ToFloat32() * width, v2.tc0.v().ToFloat32() * height},
        }};
        const f32 e1s = corners[1][0] - corners[0][0];
        const f32 e1t = corners[1][1] - corners[0][1];
        const f32 e2s = corners[2][0] - corners[0][0];
        const f32 e2t = corners[2][1] - corners[0][1];
        const f32 area = e1s * e2t - e1t * e2s;
        if (!(std::abs(area) > kSettings.min_triangle_area)) {
            return false;
        }

        const f32 min_s = std::min({corners[0][0], corners[1][0], corners[2][0]});
        const f32 max_s = std::max({corners[0][0], corners[1][0], corners[2][0]});
        const f32 min_t = std::min({corners[0][1], corners[1][1], corners[2][1]});
        const f32 max_t = std::max({corners[0][1], corners[1][1], corners[2][1]});
        const s32 s0 = static_cast<s32>(std::floor(min_s));
        const s32 s1 = static_cast<s32>(std::ceil(max_s));
        const s32 t0 = static_cast<s32>(std::floor(min_t));
        const s32 t1 = static_cast<s32>(std::ceil(max_t));
        if (!image.Reserve(s0, t0, std::max(s1, s0 + 1), std::max(t1, t0 + 1))) {
            return false;
        }

        bool painted = false;
        for (s32 t = t0; t < std::max(t1, t0 + 1); ++t) {
            for (s32 s = s0; s < std::max(s1, s0 + 1); ++s) {
                const f32 ds = static_cast<f32>(s) + 0.5f - corners[0][0];
                const f32 dt = static_cast<f32>(t) + 0.5f - corners[0][1];
                const f32 l1 = (ds * e2t - dt * e2s) / area;
                const f32 l2 = (e1s * dt - e1t * ds) / area;
                const f32 l0 = 1.0f - l1 - l2;
                if (l0 < -kSettings.inside_tolerance || l1 < -kSettings.inside_tolerance ||
                    l2 < -kSettings.inside_tolerance) {
                    continue;
                }
                const auto color = Shade(v0, v1, v2, l0, l1, l2);
                u8* pixel = image.Pixel(s, t);
                pixel[0] = color.r();
                pixel[1] = color.g();
                pixel[2] = color.b();
                pixel[3] = color.a();
                ++image.baked_pixels;
                painted = true;
            }
        }
        return painted;
    }

private:
    Pica::PicaCore& pica;
    SwRenderer::RasterizerSoftware rasterizer;
};

Vertex ToVertex(FragmentShader& shader, const Pica::OutputVertex& vertex) {
    const auto color = shader.Shade(vertex, vertex, vertex, 1.0f, 0.0f, 0.0f);
    return {vertex.pos.x.ToFloat32(),  vertex.pos.y.ToFloat32(), vertex.pos.z.ToFloat32(),
            vertex.pos.w.ToFloat32(),  vertex.tc0.x.ToFloat32(), vertex.tc0.y.ToFloat32(),
            color.r() / kSettings.color_scale, color.g() / kSettings.color_scale,
            color.b() / kSettings.color_scale, color.a() / kSettings.color_scale};
}

} // namespace

struct FrameCapture::Snapshot {
    std::vector<Draw> draws;
    std::vector<Transfer> transfers;
    std::vector<Program> programs;
    std::vector<Texture> textures;
    std::unique_ptr<FragmentShader> shader;
    u32 sequence{};
    u32 frames{};
};

FrameCapture::FrameCapture(Core::System& system_) : system{system_} {}

FrameCapture::~FrameCapture() {
    if (system.IsPoweredOn()) {
        system.GPU().PicaCore().SetDrawObserver(nullptr);
    }
}

bool FrameCapture::Request(Callback new_callback) {
    std::lock_guard lock{mutex};
    if (state.load() != State::Idle || !system.IsPoweredOn()) {
        return false;
    }
    callback = std::move(new_callback);
    snapshot = std::make_unique<Snapshot>();
    state.store(State::Collecting);
    system.GPU().PicaCore().SetDrawObserver(this);
    return true;
}

bool FrameCapture::WantsOutputVertices() const {
    return state.load() == State::Collecting;
}

void FrameCapture::OnDraw(const Pica::PicaCore& core, bool) {
    if (state.load() != State::Collecting) {
        return;
    }
    BeginDraw(core);
}

void FrameCapture::OnTriangle(const Pica::PicaCore& core, const Pica::OutputVertex& v0,
                              const Pica::OutputVertex& v1, const Pica::OutputVertex& v2) {
    if (state.load() != State::Collecting) {
        return;
    }
    if (snapshot->draws.empty()) {
        BeginDraw(core);
    }
    Draw& draw = snapshot->draws.back();
    FragmentShader& shader = *snapshot->shader;
    draw.vertices.push_back(ToVertex(shader, v0));
    draw.vertices.push_back(ToVertex(shader, v1));
    draw.vertices.push_back(ToVertex(shader, v2));
    if (draw.texture != kSettings.no_texture) {
        shader.Bake(snapshot->textures[draw.texture], v0, v1, v2);
    }
}

void FrameCapture::OnDisplayTransfer(const Pica::PicaCore& core,
                                     const Pica::DisplayTransferConfig& config) {
    if (state.load() != State::Collecting || config.is_texture_copy.Value() != 0) {
        return;
    }
    const PAddr output = config.GetPhysicalOutputAddress();
    for (u32 screen = 0; screen < kSettings.screens; ++screen) {
        const auto& lcd = core.regs.framebuffer_config[screen];
        if (output != lcd.address_left1 && output != lcd.address_left2) {
            continue;
        }
        snapshot->transfers.push_back({config.GetPhysicalInputAddress(), snapshot->sequence++,
                                       screen, lcd.height.Value(),
                                       config.flip_vertically.Value()});
        return;
    }
}

void FrameCapture::BeginDraw(const Pica::PicaCore& core) {
    const auto& regs = core.regs.internal;
    if (!snapshot->shader) {
        snapshot->shader =
            std::make_unique<FragmentShader>(system.Memory(), system.GPU().PicaCore());
    }

    Draw draw;
    draw.color_address = regs.framebuffer.framebuffer.GetColorBufferPhysicalAddress();
    draw.sequence = snapshot->sequence++;
    for (u32 output = 0; output < kSettings.output_registers; ++output) {
        const auto& attributes = regs.rasterizer.vs_output_attributes[output];
        if (attributes.map_x.Value() == Pica::RasterizerRegs::VSOutputAttributes::POSITION_X) {
            draw.position_output = output;
            break;
        }
    }

    const auto& merger = regs.framebuffer.output_merger;
    draw.alpha_mode = kSettings.alpha_opaque;
    if (merger.alpha_test.enable) {
        using Func = Pica::FramebufferRegs::CompareFunc;
        const f32 ref = static_cast<f32>(merger.alpha_test.ref) / kSettings.color_scale;
        if (merger.alpha_test.func == Func::GreaterThanOrEqual) {
            draw.alpha_mode = kSettings.alpha_mask;
            draw.alpha_cutoff = ref;
        } else if (merger.alpha_test.func == Func::GreaterThan) {
            draw.alpha_mode = kSettings.alpha_mask;
            draw.alpha_cutoff = ref + 1.0f / kSettings.color_scale;
        } else {
            draw.alpha_mode = kSettings.alpha_blend;
        }
    } else if (merger.alphablend_enable) {
        draw.alpha_mode = kSettings.alpha_blend;
    }

    Program program;
    program.code.assign(core.vs_setup.program_code.begin(),
                        core.vs_setup.program_code.begin() + kSettings.program_words);
    program.swizzle.assign(core.vs_setup.swizzle_data.begin(),
                           core.vs_setup.swizzle_data.begin() + kSettings.swizzle_words);
    program.entry_point = core.vs_setup.entry_point;
    for (u32 i = 0; i < kSettings.float_uniforms; ++i) {
        const auto& value = core.vs_setup.uniforms.f[i];
        program.uniforms.insert(program.uniforms.end(),
                                {value.x.ToFloat32(), value.y.ToFloat32(), value.z.ToFloat32(),
                                 value.w.ToFloat32()});
    }
    auto& programs = snapshot->programs;
    auto found = std::find(programs.begin(), programs.end(), program);
    if (found == programs.end()) {
        programs.push_back(std::move(program));
        found = programs.end() - 1;
    }
    draw.shader = static_cast<u32>(found - programs.begin());

    const auto& texturing = regs.texturing;
    if (texturing.main_config.texture0_enable.Value() != 0 &&
        texturing.texture0.width.Value() != 0 && texturing.texture0.height.Value() != 0) {
        Texture texture;
        texture.source_width = texturing.texture0.width.Value();
        texture.source_height = texturing.texture0.height.Value();
        texture.magnify_linear = texturing.texture0.mag_filter.Value() ==
                                 Pica::TexturingRegs::TextureConfig::Linear;
        texture.minify_linear = texturing.texture0.min_filter.Value() ==
                                Pica::TexturingRegs::TextureConfig::Linear;
        draw.texture = static_cast<u32>(snapshot->textures.size());
        snapshot->textures.push_back(std::move(texture));
    }

    snapshot->draws.push_back(std::move(draw));
}

void FrameCapture::OnFrameEnd() {
    Callback finished;
    std::vector<u8> data;
    {
        std::lock_guard lock{mutex};
        if (state.load() != State::Collecting) {
            return;
        }

        auto& draws = snapshot->draws;
        draws.erase(std::remove_if(draws.begin(), draws.end(),
                                   [](const Draw& draw) { return draw.vertices.empty(); }),
                    draws.end());

        std::array<Resolved, 2> screens{};
        std::array<bool, 2> resolved{};
        std::vector<bool> consumed(draws.size(), false);
        for (const Transfer& transfer : snapshot->transfers) {
            std::vector<const Draw*> matched;
            for (std::size_t i = 0; i < draws.size(); ++i) {
                const Draw& draw = draws[i];
                if (!consumed[i] && draw.sequence < transfer.sequence &&
                    draw.color_address == transfer.input_address) {
                    consumed[i] = true;
                    matched.push_back(&draw);
                }
            }
            if (matched.empty()) {
                continue;
            }
            screens[transfer.screen] = {transfer.screen_width, transfer.flip_vertically,
                                        std::move(matched)};
            resolved[transfer.screen] = true;
        }

        const bool all_resolved = resolved[0] && resolved[1];
        if (!all_resolved && ++snapshot->frames < kSettings.max_frames) {
            return;
        }

        std::vector<u32> texture_order;
        std::vector<u32> remap(snapshot->textures.size(), kSettings.no_texture);
        for (const Resolved& screen : screens) {
            for (const Draw* draw : screen.draws) {
                if (draw->texture != kSettings.no_texture &&
                    snapshot->textures[draw->texture].baked_pixels != 0 &&
                    remap[draw->texture] == kSettings.no_texture) {
                    remap[draw->texture] = static_cast<u32>(texture_order.size());
                    texture_order.push_back(draw->texture);
                }
            }
        }

        Writer writer;
        u32 draw_count = 0;
        for (const Resolved& screen : screens) {
            draw_count += static_cast<u32>(screen.draws.size());
        }
        writer.U32(draw_count);
        for (u32 index = 0; index < kSettings.screens; ++index) {
            const Resolved& screen = screens[index];
            for (const Draw* draw : screen.draws) {
                writer.U32(index);
                writer.U32(screen.screen_width);
                writer.U32(screen.flip_vertically);
                writer.U32(draw->shader);
                writer.U32(draw->texture == kSettings.no_texture ? kSettings.no_texture
                                                                 : remap[draw->texture]);
                writer.U32(draw->position_output);
                writer.U32(draw->alpha_mode);
                writer.F32(draw->alpha_cutoff);
                writer.U32(static_cast<u32>(draw->vertices.size()));
                writer.Bytes(draw->vertices.data(), draw->vertices.size() * sizeof(Vertex));
            }
        }
        writer.U32(static_cast<u32>(snapshot->programs.size()));
        for (const Program& program : snapshot->programs) {
            writer.U32(program.entry_point);
            writer.U32(static_cast<u32>(program.code.size()));
            writer.Bytes(program.code.data(), program.code.size() * sizeof(u32));
            writer.U32(static_cast<u32>(program.swizzle.size()));
            writer.Bytes(program.swizzle.data(), program.swizzle.size() * sizeof(u32));
            writer.U32(static_cast<u32>(program.uniforms.size() / 4));
            for (const f32 value : program.uniforms) {
                writer.F32(value);
            }
        }
        writer.U32(static_cast<u32>(texture_order.size()));
        for (const u32 index : texture_order) {
            const Texture& texture = snapshot->textures[index];
            writer.U32(texture.width);
            writer.U32(texture.height);
            writer.U32(static_cast<u32>(texture.origin_s));
            writer.U32(static_cast<u32>(texture.top_t));
            writer.U32(texture.source_width);
            writer.U32(texture.source_height);
            writer.U32(kSettings.wrap_clamp_to_edge);
            writer.U32(kSettings.wrap_clamp_to_edge);
            writer.U32(texture.magnify_linear);
            writer.U32(texture.minify_linear);
            writer.U32(static_cast<u32>(texture.rgba.size()));
            writer.Bytes(texture.rgba.data(), texture.rgba.size());
        }

        data = std::move(writer.out);
        finished = std::move(callback);
        callback = nullptr;
        snapshot.reset();
        system.GPU().PicaCore().SetDrawObserver(nullptr);
        state.store(State::Idle);
    }
    if (finished) {
        finished(std::move(data));
    }
}

} // namespace VideoCore
