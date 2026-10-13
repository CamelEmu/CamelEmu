// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include "common/common_types.h"
#include "core/arm/exclusive_monitor.h"

struct EmuCpuMonitor;

namespace Memory {
class MemorySystem;
}

namespace Core {

/// The exclusive monitor of the emu_cpu crate, which the cores of the crate share.
class RustExclusiveMonitor final : public ExclusiveMonitor {
public:
    explicit RustExclusiveMonitor(Memory::MemorySystem& memory_, std::size_t core_count);
    ~RustExclusiveMonitor() override;

    u8 ExclusiveRead8(std::size_t core_index, VAddr addr) override;
    u16 ExclusiveRead16(std::size_t core_index, VAddr addr) override;
    u32 ExclusiveRead32(std::size_t core_index, VAddr addr) override;
    u64 ExclusiveRead64(std::size_t core_index, VAddr addr) override;
    void ClearExclusive(std::size_t core_index) override;

    bool ExclusiveWrite8(std::size_t core_index, VAddr vaddr, u8 value) override;
    bool ExclusiveWrite16(std::size_t core_index, VAddr vaddr, u16 value) override;
    bool ExclusiveWrite32(std::size_t core_index, VAddr vaddr, u32 value) override;
    bool ExclusiveWrite64(std::size_t core_index, VAddr vaddr, u64 value) override;

    EmuCpuMonitor* GetMonitor() const {
        return monitor;
    }

private:
    template <typename T>
    T Read(std::size_t core_index, VAddr addr, T value);

    template <typename T>
    bool Write(std::size_t core_index, VAddr vaddr, T value,
               bool (Memory::MemorySystem::*write)(VAddr, T, T));

    EmuCpuMonitor* monitor;
    Memory::MemorySystem& memory;
};

} // namespace Core
