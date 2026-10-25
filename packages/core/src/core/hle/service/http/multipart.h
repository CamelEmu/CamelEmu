// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include <string>
#include <string_view>
#include <vector>

namespace Service::HTTP::Multipart {

struct Item {
    std::string name;
    std::string content;
    std::string content_type;
};

std::string MakeBoundary();

std::string ContentType(const std::string& boundary);

std::string SerializeItemBegin(const Item& item, const std::string& boundary);

std::string SerializeItemEnd();

std::string SerializeFinish(const std::string& boundary);

std::string Serialize(const std::vector<Item>& items, const std::string& boundary);

/// Percent-encodes a query component, leaving the characters RFC 3986 and the 3DS agree on as is.
std::string EncodeQueryParam(std::string_view value);

} // namespace Service::HTTP::Multipart
