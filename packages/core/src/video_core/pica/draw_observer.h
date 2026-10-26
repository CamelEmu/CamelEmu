// Copyright 2025 Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

namespace Pica {

class PicaCore;

class DrawObserver {
public:
    virtual ~DrawObserver() = default;

    virtual void OnDraw(const PicaCore& pica, bool is_indexed) = 0;

    virtual void OnFrameEnd() = 0;
};

} // namespace Pica
