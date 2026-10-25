// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <cctype>
#include <random>
#include "core/hle/service/http/multipart.h"

namespace Service::HTTP::Multipart {

std::string MakeBoundary() {
    constexpr std::string_view chars = "0123456789abcdefghijklmnopqrstuvwxyz";
    std::random_device device;
    std::mt19937 engine{device()};
    std::uniform_int_distribution<std::size_t> pick{0, chars.size() - 1};

    std::string boundary = "--azahar-multipart-data-";
    for (int i = 0; i < 16; i++) {
        boundary += chars[pick(engine)];
    }
    return boundary;
}

std::string ContentType(const std::string& boundary) {
    return "multipart/form-data; boundary=" + boundary;
}

std::string SerializeItemBegin(const Item& item, const std::string& boundary) {
    std::string out = "--" + boundary + "\r\n";
    out += "Content-Disposition: form-data; name=\"" + item.name + "\"\r\n";
    if (!item.content_type.empty()) {
        out += "Content-Type: " + item.content_type + "\r\n";
    }
    out += "\r\n";
    return out;
}

std::string SerializeItemEnd() {
    return "\r\n";
}

std::string SerializeFinish(const std::string& boundary) {
    return "--" + boundary + "--\r\n";
}

std::string Serialize(const std::vector<Item>& items, const std::string& boundary) {
    std::string out;
    for (const auto& item : items) {
        out += SerializeItemBegin(item, boundary) + item.content + SerializeItemEnd();
    }
    out += SerializeFinish(boundary);
    return out;
}

std::string EncodeQueryParam(std::string_view value) {
    constexpr std::string_view hex = "0123456789ABCDEF";
    std::string out;
    for (const unsigned char c : value) {
        if (std::isalnum(c) || c == '-' || c == '_' || c == '.' || c == '~' || c == '!' ||
            c == '*' || c == '\'' || c == '(' || c == ')') {
            out += static_cast<char>(c);
        } else {
            out += '%';
            out += hex[c >> 4];
            out += hex[c & 0xF];
        }
    }
    return out;
}

} // namespace Service::HTTP::Multipart
