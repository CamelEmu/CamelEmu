// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <memory>
#include <vector>
#include "core/3ds.h"
#include "core/frontend/framebuffer_layout.h"
#include "video_core/capture/screen_capture.h"
#include "video_core/renderer_base.h"

namespace VideoCore {

namespace {

constexpr u32 kBytesPerPixel = 4;

}

bool RequestScreenCapture(RendererBase& renderer, bool bottom, ScreenCaptureCallback callback) {
    if (renderer.IsScreenshotPending()) {
        return false;
    }
    const u32 scale = renderer.GetResolutionScaleFactor();
    const u32 width = (bottom ? Core::kScreenBottomWidth : Core::kScreenTopWidth) * scale;
    const u32 height = (bottom ? Core::kScreenBottomHeight : Core::kScreenTopHeight) * scale;
    auto pixels = std::make_shared<std::vector<u8>>(std::size_t{width} * height * kBytesPerPixel);
    const auto layout = Layout::SeparateWindowsLayout(width, height, bottom, false);
    renderer.RequestScreenshot(
        pixels->data(),
        [pixels, width, height, callback = std::move(callback)](bool succeeded) {
            callback(width, height, succeeded ? pixels->data() : nullptr);
        },
        layout);
    return true;
}

} // namespace VideoCore
