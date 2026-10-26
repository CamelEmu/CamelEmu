// Copyright 2025 Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include <atomic>
#include <functional>
#include <memory>
#include <mutex>
#include <vector>
#include "common/common_types.h"
#include "video_core/pica/draw_observer.h"

namespace Core {
class System;
}

namespace VideoCore {

class FrameCapture final : public Pica::DrawObserver {
public:
    using Callback = std::function<void(std::vector<u8> data)>;

    explicit FrameCapture(Core::System& system);
    ~FrameCapture() override;

    bool Request(Callback callback);

    void OnDraw(const Pica::PicaCore& pica, bool is_indexed) override;
    void OnFrameEnd() override;

private:
    enum class State : u32 {
        Idle,
        Armed,
        Collecting,
    };

    struct Snapshot;

    Core::System& system;
    std::mutex mutex;
    std::atomic<State> state{State::Idle};
    Callback callback;
    std::unique_ptr<Snapshot> snapshot;
};

} // namespace VideoCore
