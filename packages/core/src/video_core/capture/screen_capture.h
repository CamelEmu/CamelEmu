// Copyright 2025 Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include <functional>
#include "common/common_types.h"

namespace VideoCore {

class RendererBase;

using ScreenCaptureCallback = std::function<void(u32 width, u32 height, const u8* bgra)>;

bool RequestScreenCapture(RendererBase& renderer, bool bottom, ScreenCaptureCallback callback);

} // namespace VideoCore
