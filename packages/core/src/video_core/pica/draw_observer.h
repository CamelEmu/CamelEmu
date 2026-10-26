// Copyright 2025 Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include "video_core/pica/output_vertex.h"
#include "video_core/pica/regs_external.h"

namespace Pica {

class PicaCore;

class DrawObserver {
public:
    virtual ~DrawObserver() = default;

    virtual bool WantsOutputVertices() const = 0;

    virtual void OnDraw(const PicaCore& pica, bool is_indexed) = 0;

    virtual void OnTriangle(const PicaCore& pica, const OutputVertex& v0, const OutputVertex& v1,
                            const OutputVertex& v2) = 0;

    virtual void OnDisplayTransfer(const PicaCore& pica, const DisplayTransferConfig& config) = 0;

    virtual void OnFrameEnd() = 0;
};

} // namespace Pica
