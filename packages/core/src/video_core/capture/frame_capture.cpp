// Copyright 2025 Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <algorithm>
#include <array>
#include <cstring>
#include <memory>
#include "core/core.h"
#include "core/memory.h"
#include "video_core/capture/frame_capture.h"
#include "video_core/gpu.h"
#include "video_core/pica/pica_core.h"
#include "video_core/texture/texture_decode.h"

namespace VideoCore {

namespace {

struct CaptureSettings {
    u32 program_words = 512;
    u32 swizzle_words = 128;
    u32 float_uniforms = 96;
    u32 attribute_loaders = 12;
    u32 no_texture = 0xffffffff;
    u32 topology_list = 0;
    u32 topology_strip = 1;
    u32 topology_fan = 2;
    u32 index_none = 0;
    u32 index_byte = 1;
    u32 index_short = 2;
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

struct Buffer {
    std::vector<u8> data;
    u32 stride{};
};

struct Attribute {
    u32 buffer{};
    u32 offset{};
    u32 format{};
    u32 components{};
};

struct Draw {
    u32 screen_width{};
    u32 topology{};
    u32 vertex_count{};
    u32 vertex_offset{};
    u32 shader{};
    u32 texture{};
    std::vector<Buffer> buffers;
    std::vector<Attribute> attributes;
    u32 index_format{};
    std::vector<u8> indices;
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

struct Texture {
    PAddr address{};
    u32 width{};
    u32 height{};
    u32 format{};
    u32 wrap_s{};
    u32 wrap_t{};
    u32 magnify_linear{};
    u32 minify_linear{};
    std::vector<u8> rgba;
};

u32 TopologyOf(Pica::PipelineRegs::TriangleTopology topology) {
    switch (topology) {
    case Pica::PipelineRegs::TriangleTopology::Strip:
        return kSettings.topology_strip;
    case Pica::PipelineRegs::TriangleTopology::Fan:
        return kSettings.topology_fan;
    default:
        return kSettings.topology_list;
    }
}

} // namespace

struct FrameCapture::Snapshot {
    std::vector<Draw> draws;
    std::vector<Program> programs;
    std::vector<Texture> textures;
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
    state.store(State::Armed);
    system.GPU().PicaCore().SetDrawObserver(this);
    return true;
}

void FrameCapture::OnDraw(const Pica::PicaCore& core, bool is_indexed) {
    if (state.load() != State::Collecting) {
        return;
    }
    Memory::MemorySystem& memory = system.Memory();
    const auto& regs = core.regs.internal;
    const auto& pipeline = regs.pipeline;
    const auto& attributes = pipeline.vertex_attributes;

    Draw draw;
    draw.screen_width = regs.framebuffer.framebuffer.GetHeight();
    draw.topology = TopologyOf(pipeline.triangle_topology.Value());
    draw.vertex_count = pipeline.num_vertices;
    draw.vertex_offset = pipeline.vertex_offset;

    const PAddr base = attributes.GetPhysicalBaseAddress();
    u32 max_vertex = pipeline.vertex_offset + pipeline.num_vertices;

    if (is_indexed) {
        const bool short_indices = pipeline.index_array.format.Value() == pipeline.index_array.SHORT;
        const std::size_t size = short_indices ? sizeof(u16) : sizeof(u8);
        const PAddr indices_address = base + pipeline.index_array.offset;
        const u8* source = memory.GetPhysicalPointer(indices_address);
        if (source == nullptr) {
            return;
        }
        draw.index_format = short_indices ? kSettings.index_short : kSettings.index_byte;
        draw.indices.assign(source, source + size * pipeline.num_vertices);
        max_vertex = 0;
        for (u32 i = 0; i < pipeline.num_vertices; ++i) {
            u32 index = draw.indices[i * size];
            if (short_indices) {
                index |= static_cast<u32>(draw.indices[i * size + 1]) << 8;
            }
            max_vertex = std::max(max_vertex, index + 1);
        }
    } else {
        draw.index_format = kSettings.index_none;
    }

    std::array<u32, 12> attribute_order{};
    std::array<bool, 12> attribute_loaded{};
    for (u32 loader = 0; loader < kSettings.attribute_loaders; ++loader) {
        const auto& config = attributes.attribute_loaders[loader];
        Buffer buffer;
        buffer.stride = config.byte_count;
        if (config.component_count != 0 && buffer.stride != 0) {
            const u8* source = memory.GetPhysicalPointer(base + config.data_offset);
            if (source != nullptr) {
                buffer.data.assign(source, source + std::size_t{buffer.stride} * max_vertex);
            }
        }

        u32 offset = 0;
        for (u32 component = 0; component < config.component_count && component < 12;
             ++component) {
            const u32 index = config.GetComponent(component);
            if (index < 12) {
                const u32 size = attributes.GetElementSizeInBytes(index);
                offset = (offset + size - 1) / size * size;
                if (!attributes.IsDefaultAttribute(index)) {
                    attribute_order[index] = static_cast<u32>(draw.attributes.size());
                    attribute_loaded[index] = true;
                    draw.attributes.push_back(
                        {loader, offset, static_cast<u32>(attributes.GetFormat(index)),
                         attributes.GetNumElements(index)});
                }
                offset += attributes.GetStride(index);
            } else {
                offset = (offset + 3) / 4 * 4 + (index - 11) * 4;
            }
        }
        draw.buffers.push_back(std::move(buffer));
    }

    std::vector<Attribute> ordered;
    for (u32 index = 0; index < 12; ++index) {
        if (attribute_loaded[index]) {
            ordered.push_back(draw.attributes[attribute_order[index]]);
        }
    }
    draw.attributes = std::move(ordered);

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

    draw.texture = kSettings.no_texture;
    const auto& texturing = regs.texturing;
    if (texturing.main_config.texture0_enable.Value() != 0) {
        const auto info = Pica::Texture::TextureInfo::FromPicaRegister(
            texturing.texture0, texturing.texture0_format.Value());
        auto& textures = snapshot->textures;
        auto existing = std::find_if(textures.begin(), textures.end(), [&](const Texture& t) {
            return t.address == info.physical_address && t.width == info.width &&
                   t.height == info.height && t.format == static_cast<u32>(info.format);
        });
        if (existing == textures.end()) {
            const u8* source = memory.GetPhysicalPointer(info.physical_address);
            if (source != nullptr && info.width != 0 && info.height != 0) {
                Texture texture;
                texture.address = info.physical_address;
                texture.width = info.width;
                texture.height = info.height;
                texture.format = static_cast<u32>(info.format);
                texture.wrap_s = static_cast<u32>(texturing.texture0.wrap_s.Value());
                texture.wrap_t = static_cast<u32>(texturing.texture0.wrap_t.Value());
                texture.magnify_linear = texturing.texture0.mag_filter.Value() ==
                                         Pica::TexturingRegs::TextureConfig::Linear;
                texture.minify_linear = texturing.texture0.min_filter.Value() ==
                                        Pica::TexturingRegs::TextureConfig::Linear;
                texture.rgba.reserve(std::size_t{info.width} * info.height * 4);
                for (u32 y = 0; y < info.height; ++y) {
                    for (u32 x = 0; x < info.width; ++x) {
                        const auto texel = Pica::Texture::LookupTexture(source, x, y, info);
                        texture.rgba.insert(texture.rgba.end(),
                                            {texel.r(), texel.g(), texel.b(), texel.a()});
                    }
                }
                textures.push_back(std::move(texture));
                existing = textures.end() - 1;
            }
        }
        if (existing != textures.end()) {
            draw.texture = static_cast<u32>(existing - textures.begin());
        }
    }

    snapshot->draws.push_back(std::move(draw));
}

void FrameCapture::OnFrameEnd() {
    Callback finished;
    std::vector<u8> data;
    {
        std::lock_guard lock{mutex};
        switch (state.load()) {
        case State::Armed:
            state.store(State::Collecting);
            return;
        case State::Collecting:
            break;
        case State::Idle:
            return;
        }

        Writer writer;
        writer.U32(static_cast<u32>(snapshot->draws.size()));
        for (const Draw& draw : snapshot->draws) {
            writer.U32(draw.screen_width);
            writer.U32(draw.topology);
            writer.U32(draw.vertex_count);
            writer.U32(draw.vertex_offset);
            writer.U32(draw.shader);
            writer.U32(draw.texture);
            writer.U32(static_cast<u32>(draw.buffers.size()));
            for (const Buffer& buffer : draw.buffers) {
                writer.U32(buffer.stride);
                writer.U32(static_cast<u32>(buffer.data.size()));
                writer.Bytes(buffer.data.data(), buffer.data.size());
            }
            writer.U32(static_cast<u32>(draw.attributes.size()));
            for (const Attribute& attribute : draw.attributes) {
                writer.U32(attribute.buffer);
                writer.U32(attribute.offset);
                writer.U32(attribute.format);
                writer.U32(attribute.components);
            }
            writer.U32(draw.index_format);
            writer.U32(static_cast<u32>(draw.indices.size()));
            writer.Bytes(draw.indices.data(), draw.indices.size());
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
        writer.U32(static_cast<u32>(snapshot->textures.size()));
        for (const Texture& texture : snapshot->textures) {
            writer.U32(texture.width);
            writer.U32(texture.height);
            writer.U32(texture.wrap_s);
            writer.U32(texture.wrap_t);
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
