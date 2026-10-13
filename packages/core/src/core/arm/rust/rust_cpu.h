// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include "common/arch.h"

// The CPU and the memory of the Rust crates are only built for Android on arm64.
#if defined(ANDROID) && CITRA_ARCH(arm64) && defined(CITRA_RUST_CPU)
#define CITRA_HAS_RUST_CPU 1
#else
#define CITRA_HAS_RUST_CPU 0
#endif
