// Copyright 2017 Citra Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <memory>
#include <string>
#include "common/web_result.h"
#include "emu_json.h"
#include "web_service/verify_login.h"
#include "web_service/web_backend.h"

namespace WebService {

bool VerifyLogin(const std::string& host, const std::string& username, const std::string& token) {
    Client client(host, username, token);
    auto reply = client.GetJson("/profile", false).returned_data;
    if (reply.empty()) {
        return false;
    }
    // emu_json_free is a no-op on the borrowed handle from emu_json_get, so
    // one deleter serves both handles.
    using JsonHandle = std::unique_ptr<EmuJson, decltype(&emu_json_free)>;
    const JsonHandle json(emu_json_parse(reply.data(), reply.size()), &emu_json_free);
    if (!json) {
        return false;
    }
    const JsonHandle iter(emu_json_get(json.get(), "username"), &emu_json_free);
    if (!iter) {
        return username.empty();
    }
    if (emu_json_type(iter.get()) != EMU_JSON_STRING) {
        return false;
    }
    const char* value = emu_json_string(iter.get());
    return value != nullptr && username == std::string(value, emu_json_string_len(iter.get()));
}

} // namespace WebService
