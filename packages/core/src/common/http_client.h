// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include <algorithm>
#include <cctype>
#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <string_view>
#include <utility>
#include <vector>
#include <emu_http.h>

namespace Common::Http {

using Headers = std::vector<std::pair<std::string, std::string>>;

struct Request {
    std::string method;
    std::string path;
    Headers headers;
    std::string_view body;
    /// Supplies the body instead of `body`. `body_length` is its total size, or 0 when unknown.
    EmuHttpProvideBody provide_body = nullptr;
    void* user = nullptr;
    std::size_t body_length = 0;
    bool follow_redirects = false;
};

inline bool HeaderNameEquals(std::string_view a, std::string_view b) {
    return std::ranges::equal(a, b, [](char x, char y) {
        return std::tolower(static_cast<unsigned char>(x)) ==
               std::tolower(static_cast<unsigned char>(y));
    });
}

inline std::optional<std::string> FindHeader(const Headers& headers, std::string_view name) {
    for (const auto& [candidate, value] : headers) {
        if (HeaderNameEquals(candidate, name)) {
            return value;
        }
    }
    return std::nullopt;
}

/// Owns an EmuHttpResponse. Header names are looked up case-insensitively.
class Response {
public:
    explicit Response(EmuHttpResponse raw) : raw{raw} {}
    Response(const Response&) = delete;
    Response& operator=(const Response&) = delete;
    ~Response() {
        emu_http_response_free(&raw);
    }

    bool Failed() const {
        return raw.error != nullptr;
    }

    std::string Error() const {
        return raw.error != nullptr ? raw.error : "";
    }

    int Status() const {
        return raw.status;
    }

    std::string Reason() const {
        return raw.reason != nullptr ? raw.reason : "";
    }

    std::string_view Body() const {
        return {reinterpret_cast<const char*>(raw.body), raw.body_len};
    }

    Headers AllHeaders() const {
        Headers headers;
        headers.reserve(raw.header_count);
        for (std::size_t i = 0; i < raw.header_count; i++) {
            headers.emplace_back(raw.headers[i].name, raw.headers[i].value);
        }
        return headers;
    }

    std::optional<std::string> Header(std::string_view name) const {
        return FindHeader(AllHeaders(), name);
    }

private:
    EmuHttpResponse raw;
};

class Client {
public:
    /// `host` is the base URL including the scheme. Returns nullptr when it is not a valid one.
    static std::unique_ptr<Client> Open(const std::string& host, std::uint32_t timeout_seconds,
                                        EmuTlsContext* tls = nullptr) {
        EmuHttp* http = emu_http_open(host.c_str(), timeout_seconds, tls);
        if (http == nullptr) {
            return nullptr;
        }
        return std::unique_ptr<Client>{new Client{http}};
    }

    ~Client() {
        emu_http_close(http);
    }
    Client(const Client&) = delete;
    Client& operator=(const Client&) = delete;

    Response Send(const Request& request) {
        std::vector<EmuHttpHeader> headers;
        headers.reserve(request.headers.size());
        for (const auto& [name, value] : request.headers) {
            headers.push_back({name.c_str(), value.c_str()});
        }
        const EmuHttpRequest raw{
            .method = request.method.c_str(),
            .path = request.path.c_str(),
            .headers = headers.data(),
            .header_count = headers.size(),
            .body = request.provide_body != nullptr
                        ? nullptr
                        : reinterpret_cast<const std::uint8_t*>(request.body.data()),
            .body_len = request.provide_body != nullptr ? request.body_length : request.body.size(),
            .provide_body = request.provide_body,
            .user = request.user,
            .follow_redirects = request.follow_redirects ? 1 : 0,
        };
        return Response{emu_http_request(http, &raw)};
    }

private:
    explicit Client(EmuHttp* http) : http{http} {}

    EmuHttp* http;
};

} // namespace Common::Http
