// Copyright 2023 Citra Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include "common/file_util.h"
#include "common/literals.h"
#include "common/memory_detect.h"
#include "common/microprofile.h"
#include "common/settings.h"
#include "common/string_util.h"
#include "common/texture.h"
#include "core/core.h"
#include "core/frontend/image_interface.h"
#include "core/hle/kernel/kernel.h"
#include "core/hle/kernel/process.h"
#include "emu_json.h"
#include "video_core/custom_textures/custom_tex_manager.h"
#include "video_core/rasterizer_cache/surface_params.h"
#include "video_core/rasterizer_cache/utils.h"

namespace VideoCore {

namespace {

MICROPROFILE_DEFINE(CustomTexManager_TickFrame, "CustomTexManager", "TickFrame",
                    MP_RGB(54, 16, 32));

constexpr std::size_t MAX_UPLOADS_PER_TICK = 8;

using namespace Common::Literals;

bool IsPow2(u32 value) {
    return value != 0 && (value & (value - 1)) == 0;
}

CustomFileFormat MakeFileFormat(std::string_view ext) {
    if (ext == "png") {
        return CustomFileFormat::PNG;
    } else if (ext == "dds") {
        return CustomFileFormat::DDS;
    } else if (ext == "ktx") {
        return CustomFileFormat::KTX;
    }
    return CustomFileFormat::None;
}

MapType MakeMapType(std::string_view ext) {
    if (ext == "norm") {
        return MapType::Normal;
    }
    LOG_ERROR(Render, "Unknown material extension {}", ext);
    return MapType::Color;
}

} // Anonymous namespace

CustomTexManager::CustomTexManager(Core::System& system_)
    : system{system_}, image_interface{*system.GetImageInterface()},
      async_custom_loading{Settings::values.async_custom_loading.GetValue()} {}

CustomTexManager::~CustomTexManager() = default;

void CustomTexManager::TickFrame() {
    MICROPROFILE_SCOPE(CustomTexManager_TickFrame);
    if (!textures_loaded) {
        return;
    }
    std::size_t num_uploads = 0;
    for (auto it = async_uploads.begin(); it != async_uploads.end();) {
        if (num_uploads >= MAX_UPLOADS_PER_TICK) {
            return;
        }
        switch (it->material->state) {
        case DecodeState::Decoded:
            it->func();
            num_uploads++;
            [[fallthrough]];
        case DecodeState::Failed:
            it = async_uploads.erase(it);
            continue;
        default:
            it++;
            break;
        }
    }
}

void CustomTexManager::FindCustomTextures() {
    if (textures_loaded) {
        return;
    }
    if (!workers) {
        CreateWorkers();
    }

    const u64 title_id = system.Kernel().GetCurrentProcess()->codeset->program_id;
    const auto textures = GetTextures(title_id);
    if (!ReadConfig(title_id)) {
        use_new_hash = false;
        skip_mipmap = true;
    }

    custom_textures.reserve(textures.size());
    for (const FileUtil::FSTEntry& file : textures) {
        if (file.isDirectory) {
            continue;
        }
        custom_textures.push_back(std::make_unique<CustomTexture>(image_interface));
        CustomTexture* const texture{custom_textures.back().get()};
        if (!ParseFilename(file, texture)) {
            continue;
        }
        for (const u64 hash : texture->hashes) {
            auto& material = material_map[hash];
            if (!material) {
                material = std::make_unique<Material>();
            }
            material->hash = hash;
            material->AddMapTexture(texture);
        }
    }
    textures_loaded = true;
}

bool CustomTexManager::ParseFilename(const FileUtil::FSTEntry& file, CustomTexture* texture) {
    auto parts = Common::SplitString(file.virtualName, '.');
    if (parts.size() > 3) {
        LOG_ERROR(Render, "Invalid filename {}, ignoring", file.virtualName);
        return false;
    }
    // The last string should always be the file extension.
    const CustomFileFormat file_format = MakeFileFormat(parts.back());
    if (file_format == CustomFileFormat::None) {
        return false;
    }
    if (file_format == CustomFileFormat::DDS && skip_mipmap) {
        LOG_ERROR(Render, "Mipmap skip is incompatible with DDS textures, skipping!");
        return false;
    }
    texture->file_format = file_format;
    parts.pop_back();

    // This means the texture is a material type other than color.
    texture->type = MapType::Color;
    if (parts.size() > 1) {
        texture->type = MakeMapType(parts.back());
        parts.pop_back();
    }

    // First look if this file is mapped to any number of hashes.
    std::vector<u64>& hashes = texture->hashes;
    const auto it = path_to_hash_map.find(file.virtualName);
    if (it != path_to_hash_map.end()) {
        hashes = it->second;
    }

    // It's also possible for pack creators to retain the default texture name
    // still map the texture to another hash. Support that as well.
    u32 width;
    u32 height;
    u32 format;
    unsigned long long hash{};
    const bool is_parsed = std::sscanf(parts.back().c_str(), "tex1_%ux%u_%llX_%u", &width, &height,
                                       &hash, &format) == 4;
    const bool is_mapped =
        !hashes.empty() && std::find(hashes.begin(), hashes.end(), hash) != hashes.end();
    if (is_parsed && !is_mapped) {
        hashes.push_back(hash);
    }

    texture->path = file.physicalName;
    return true;
}

void CustomTexManager::PrepareDumping(u64 title_id) {
    // If a pack exists in the load folder that uses the old hash, dump textures using the old hash.
    // This occurs either if a configuration file doesn't exist or that file sets the old hash.
    const std::string load_path =
        fmt::format("{}textures/{:016X}/", GetUserPath(FileUtil::UserPath::LoadDir), title_id);
    if (FileUtil::Exists(load_path) && !ReadConfig(title_id, true)) {
        use_new_hash = false;
    }

    // Write template config file
    const std::string dump_path =
        fmt::format("{}textures/{:016X}/", GetUserPath(FileUtil::UserPath::DumpDir), title_id);
    const std::string pack_config = dump_path + "pack.json";
    if (FileUtil::Exists(pack_config)) {
        return;
    }

    // emu_json_free is a no-op on borrowed handles, so one deleter serves
    // every handle in this function.
    using JsonHandle = std::unique_ptr<EmuJson, decltype(&emu_json_free)>;
    const JsonHandle json(emu_json_new_object(), &emu_json_free);
    emu_json_set(json.get(), "author", emu_json_new_string("citra"));
    emu_json_set(json.get(), "version", emu_json_new_string("1.0.0"));
    emu_json_set(json.get(), "description", emu_json_new_string("A graphics pack"));

    JsonHandle options(emu_json_new_object(), &emu_json_free);
    emu_json_set(options.get(), "skip_mipmap", emu_json_new_bool(0));
    emu_json_set(options.get(), "flip_png_files", emu_json_new_bool(1));
    emu_json_set(options.get(), "use_new_hash", emu_json_new_bool(1));
    emu_json_set(json.get(), "options", options.release());

    FileUtil::IOFile file{pack_config, "w"};
    char* output = emu_json_stringify_indent(json.get(), 4, nullptr);
    if (output != nullptr) {
        file.WriteString(output);
        emu_json_free_string(output);
    }
}

void CustomTexManager::PreloadTextures(const std::atomic_bool& stop_run,
                                       const VideoCore::DiskResourceLoadCallback& callback) {
    u64 size_sum = 0;
    std::size_t preloaded = 0;
    const u64 sys_mem = Common::GetMemInfo().total_physical_memory;
    const u64 recommended_min_mem = 2_GiB;

    // keep 2GiB memory for system stability if system RAM is 4GiB+ - use half of memory in other
    // cases
    const u64 max_mem =
        (sys_mem / 2 < recommended_min_mem) ? (sys_mem / 2) : (sys_mem - recommended_min_mem);

    workers->QueueWork([&]() {
        for (auto& [hash, material] : material_map) {
            if (size_sum > max_mem) {
                LOG_WARNING(Render, "Aborting texture preload due to insufficient memory");
                return;
            }
            if (stop_run) {
                return;
            }
            material->LoadFromDisk(flip_png_files);
            size_sum += material->size;
            if (callback) {
                callback(VideoCore::LoadCallbackStage::Preload, preloaded, custom_textures.size());
            }
            preloaded++;
        }
    });
    workers->WaitForRequests();
    async_custom_loading = false;
}

void CustomTexManager::DumpTexture(const SurfaceParams& params, u32 level, std::span<u8> data,
                                   u64 data_hash) {
    const u64 program_id = system.Kernel().GetCurrentProcess()->codeset->program_id;
    const u32 data_size = static_cast<u32>(data.size());
    const u32 width = params.width;
    const u32 height = params.height;
    const PixelFormat format = params.pixel_format;

    std::string dump_path = fmt::format(
        "{}textures/{:016X}/", FileUtil::GetUserPath(FileUtil::UserPath::DumpDir), program_id);
    if (!FileUtil::CreateFullPath(dump_path)) {
        LOG_ERROR(Render, "Unable to create {}", dump_path);
        return;
    }

    dump_path +=
        fmt::format("tex1_{}x{}_{:016X}_{}_mip{}.png", width, height, data_hash, format, level);
    if (dumped_textures.contains(data_hash) || FileUtil::Exists(dump_path)) {
        return;
    }

    // Make sure the texture size is a power of 2.
    // If not, the surface is probably a framebuffer
    if (!IsPow2(width) || !IsPow2(height)) {
        LOG_WARNING(Render, "Not dumping {:016X} because size isn't a power of 2 ({}x{})",
                    data_hash, width, height);
        return;
    }

    const u32 decoded_size = width * height * 4;
    std::vector<u8> pixels(data_size + decoded_size);
    std::memcpy(pixels.data(), data.data(), data_size);

    auto dump = [this, width, height, params, data_size, decoded_size, pixels = std::move(pixels),
                 dump_path = std::move(dump_path)]() mutable {
        const std::span encoded = std::span{pixels}.first(data_size);
        const std::span decoded = std::span{pixels}.last(decoded_size);
        DecodeTexture(params, params.addr, params.end, encoded, decoded,
                      params.type == SurfaceType::Color);
        Common::FlipRGBA8Texture(decoded, width, height);
        image_interface.EncodePNG(dump_path, width, height, decoded);
    };
    if (!workers) {
        CreateWorkers();
    }
    workers->QueueWork(std::move(dump));
    dumped_textures.insert(data_hash);
}

Material* CustomTexManager::GetMaterial(u64 data_hash) {
    const auto it = material_map.find(data_hash);
    if (it == material_map.end()) {
        LOG_WARNING(Render, "Unable to find replacement for surface with hash {:016X}", data_hash);
        return nullptr;
    }
    return it->second.get();
}

bool CustomTexManager::Decode(Material* material, std::function<bool()>&& upload) {
    if (!async_custom_loading) {
        material->LoadFromDisk(flip_png_files);
        return upload();
    }
    if (material->IsUnloaded()) {
        material->state = DecodeState::Pending;
        workers->QueueWork([material, this] { material->LoadFromDisk(flip_png_files); });
    }
    async_uploads.push_back({
        .material = material,
        .func = std::move(upload),
    });
    return false;
}

bool CustomTexManager::ReadConfig(u64 title_id, bool options_only) {
    const std::string load_path =
        fmt::format("{}textures/{:016X}/", GetUserPath(FileUtil::UserPath::LoadDir), title_id);
    if (!FileUtil::Exists(load_path)) {
        FileUtil::CreateFullPath(load_path);
    }

    const std::string config_path = load_path + "pack.json";
    FileUtil::IOFile config_file{config_path, "r"};
    if (!config_file.IsOpen()) {
        LOG_INFO(Render, "Unable to find pack config file, using legacy defaults");
        return false;
    }
    std::string config(config_file.GetSize(), '\0');
    const std::size_t read_size = config_file.ReadBytes(config.data(), config.size());
    if (!read_size) {
        return false;
    }

    // emu_json_free is a no-op on borrowed handles, so one deleter serves
    // every handle in this function.
    using JsonHandle = std::unique_ptr<EmuJson, decltype(&emu_json_free)>;
    const JsonHandle json(emu_json_parse(config.data(), read_size), &emu_json_free);
    if (!json) {
        LOG_ERROR(Render, "Failed to parse pack config {}: {}", config_path, emu_json_error());
        return false;
    }

    const JsonHandle options(emu_json_get(json.get(), "options"), &emu_json_free);
    if (!options || emu_json_type(options.get()) != EMU_JSON_OBJECT) {
        return false;
    }
    const auto get_bool = [&options](const char* key, bool& out) {
        const JsonHandle value(emu_json_get(options.get(), key), &emu_json_free);
        if (!value || emu_json_type(value.get()) != EMU_JSON_BOOL) {
            return false;
        }
        out = emu_json_bool(value.get()) != 0;
        return true;
    };
    if (!get_bool("skip_mipmap", skip_mipmap) || !get_bool("flip_png_files", flip_png_files) ||
        !get_bool("use_new_hash", use_new_hash)) {
        return false;
    }

    if (options_only) {
        return true;
    }

    const JsonHandle textures(emu_json_get(json.get(), "textures"), &emu_json_free);
    if (!textures || emu_json_type(textures.get()) != EMU_JSON_OBJECT) {
        return true;
    }
    const std::size_t material_count = emu_json_object_len(textures.get());
    for (std::size_t i = 0; i < material_count; ++i) {
        std::size_t key_len{};
        const char* key_ptr = emu_json_key_at(textures.get(), i, &key_len);
        if (key_ptr == nullptr) {
            continue;
        }
        const std::string key(key_ptr, key_len);
        std::size_t idx{};
        const u64 hash = std::stoull(key, &idx, 16);
        if (!idx) {
            LOG_ERROR(Render, "Key {} is invalid, skipping", key);
            continue;
        }
        const auto parse = [&](const std::string& file) {
            const std::string filename{FileUtil::GetFilename(file)};
            auto [it, new_hash] = path_to_hash_map.try_emplace(filename);
            it->second.push_back(hash);
        };
        const JsonHandle value(emu_json_object_value_at(textures.get(), i), &emu_json_free);
        if (emu_json_type(value.get()) == EMU_JSON_STRING) {
            parse(std::string(emu_json_string(value.get()), emu_json_string_len(value.get())));
        } else if (emu_json_type(value.get()) == EMU_JSON_ARRAY) {
            const std::size_t file_count = emu_json_array_len(value.get());
            for (std::size_t j = 0; j < file_count; ++j) {
                const JsonHandle file(emu_json_array_at(value.get(), j), &emu_json_free);
                if (emu_json_type(file.get()) == EMU_JSON_STRING) {
                    parse(std::string(emu_json_string(file.get()),
                                      emu_json_string_len(file.get())));
                }
            }
        } else {
            LOG_ERROR(Render, "Material with key {} is invalid", key);
        }
    }
    return true;
}

std::vector<FileUtil::FSTEntry> CustomTexManager::GetTextures(u64 title_id) {
    const std::string load_path =
        fmt::format("{}textures/{:016X}/", GetUserPath(FileUtil::UserPath::LoadDir), title_id);
    if (!FileUtil::Exists(load_path)) {
        FileUtil::CreateFullPath(load_path);
    }

    FileUtil::FSTEntry texture_dir;
    std::vector<FileUtil::FSTEntry> textures;
    FileUtil::ScanDirectoryTree(load_path, texture_dir, 64);
    FileUtil::GetAllFilesFromNestedEntries(texture_dir, textures);
    return textures;
}

void CustomTexManager::CreateWorkers() {
    const std::size_t num_workers = std::max(std::thread::hardware_concurrency(), 2U) >> 1;
    workers = std::make_unique<Common::ThreadWorker>(num_workers, "Custom textures");
}

} // namespace VideoCore
