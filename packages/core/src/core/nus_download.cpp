// Copyright 2020 Citra Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <memory>
#include "common/http_client.h"
#include "common/logging/log.h"
#include "core/nus_download.h"

namespace Core::NUS {

std::optional<std::vector<u8>> Download(const std::string& path) {
    constexpr auto HOST = "http://nus.cdn.c.shop.nintendowifi.net";

    const auto client = Common::Http::Client::Open(HOST, 0);
    if (client == nullptr) {
        LOG_ERROR(WebService, "Invalid URL {}{}", HOST, path);
        return {};
    }

    const auto response = client->Send({
        .method = "GET",
        .path = path,
        .follow_redirects = true,
    });
    if (response.Failed()) {
        LOG_ERROR(WebService, "GET to {}{} returned null: {}", HOST, path, response.Error());
        return {};
    }
    if (response.Status() >= 400) {
        LOG_ERROR(WebService, "GET to {}{} returned error status code: {}", HOST, path,
                  response.Status());
        return {};
    }
    if (!response.Header("content-type")) {
        LOG_ERROR(WebService, "GET to {}{} returned no content", HOST, path);
        return {};
    }

    const auto body = response.Body();
    return std::vector<u8>(body.begin(), body.end());
}

} // namespace Core::NUS
