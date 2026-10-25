// Copyright 2018 Citra Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <algorithm>
#include <emu_crypto.h>
#include <emu_json.h>
#include "common/logging/log.h"
#include "common/web_result.h"
#include "web_service/verify_user_jwt.h"
#include "web_service/web_backend.h"

namespace WebService {

static std::string public_key;
std::string GetPublicKey(const std::string& host) {
    if (public_key.empty()) {
        Client client(host, "", ""); // no need for credentials here
        public_key = client.GetPlain("/jwt/external/key.pem", true).returned_data;
        if (public_key.empty()) {
            LOG_ERROR(WebService, "Could not fetch external JWT public key, verification may fail");
        } else {
            LOG_INFO(WebService, "Fetched external JWT public key (size={})", public_key.size());
        }
    }
    return public_key;
}

namespace {

struct JsonDeleter {
    void operator()(EmuJson* json) const {
        emu_json_free(json);
    }
};

struct StringDeleter {
    void operator()(char* string) const {
        emu_crypto_string_free(string);
    }
};

std::string StringOf(const EmuJson* json) {
    const char* value = emu_json_string(json);
    return value ? std::string(value, emu_json_string_len(json)) : std::string{};
}

} // namespace

void VerifyUserJWT::KeyDeleter::operator()(EmuKeyHandle* key) const {
    emu_crypto_key_free(key);
}

VerifyUserJWT::VerifyUserJWT(const std::string& host) {
    const std::string pub_key = GetPublicKey(host);
    key.reset(emu_crypto_key_parse(reinterpret_cast<const uint8_t*>(pub_key.data()),
                                   pub_key.size()));
    if (!key) {
        LOG_ERROR(WebService, "Could not parse external JWT public key, verification will fail");
    }
}

VerifyUserJWT::~VerifyUserJWT() = default;

Network::VerifyUser::UserData VerifyUserJWT::LoadUserData(const std::string& verify_UID,
                                                          const std::string& token) {
    if (!key) {
        return {};
    }
    const std::string audience = fmt::format("external-{}", verify_UID);
    const int32_t status =
        emu_crypto_jwt_verify_rs256(key.get(), token.c_str(), "citra-core", audience.c_str());
    if (status != EMU_CRYPTO_JWT_OK) {
        LOG_INFO(WebService, "Verification failed: status={}", status);
        return {};
    }

    size_t claims_len = 0;
    std::unique_ptr<char, StringDeleter> claims_text(
        emu_crypto_jwt_claims(key.get(), token.c_str(), "citra-core", audience.c_str(),
                              &claims_len));
    if (!claims_text) {
        return {};
    }
    std::unique_ptr<EmuJson, JsonDeleter> claims(emu_json_parse(claims_text.get(), claims_len));
    if (!claims) {
        return {};
    }

    Network::VerifyUser::UserData user_data{};
    if (const EmuJson* username = emu_json_get(claims.get(), "username")) {
        user_data.username = StringOf(username);
    }
    if (const EmuJson* display_name = emu_json_get(claims.get(), "displayName")) {
        user_data.display_name = StringOf(display_name);
    }
    if (const EmuJson* avatar_url = emu_json_get(claims.get(), "avatarUrl")) {
        user_data.avatar_url = StringOf(avatar_url);
    }
    if (const EmuJson* roles = emu_json_get(claims.get(), "roles")) {
        const size_t count = emu_json_array_len(roles);
        for (size_t i = 0; i < count; ++i) {
            if (StringOf(emu_json_array_at(roles, i)) == "moderator") {
                user_data.moderator = true;
                break;
            }
        }
    }
    return user_data;
}

} // namespace WebService
