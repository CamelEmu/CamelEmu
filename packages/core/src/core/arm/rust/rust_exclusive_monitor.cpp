// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <emu_cpu.h>
#include "common/assert.h"
#include "core/arm/rust/rust_exclusive_monitor.h"
#include "core/memory.h"

namespace Core {

RustExclusiveMonitor::RustExclusiveMonitor(Memory::MemorySystem& memory_, std::size_t core_count)
    : monitor{emu_cpu_monitor_create(static_cast<u32>(core_count))}, memory{memory_} {
    ASSERT(monitor);
}

RustExclusiveMonitor::~RustExclusiveMonitor() {
    emu_cpu_monitor_destroy(monitor);
}

template <typename T>
T RustExclusiveMonitor::Read(std::size_t core_index, VAddr addr, T value) {
    emu_cpu_monitor_mark(monitor, static_cast<u32>(core_index), addr, value);
    return value;
}

template <typename T>
bool RustExclusiveMonitor::Write(std::size_t core_index, VAddr vaddr, T value,
                                 bool (Memory::MemorySystem::*write)(VAddr, T, T)) {
    u64 expected = 0;
    if (!emu_cpu_monitor_check_and_clear(monitor, static_cast<u32>(core_index), vaddr,
                                         &expected)) {
        return false;
    }
    return (memory.*write)(vaddr, value, static_cast<T>(expected));
}

u8 RustExclusiveMonitor::ExclusiveRead8(std::size_t core_index, VAddr addr) {
    return Read<u8>(core_index, addr, memory.Read8(addr));
}

u16 RustExclusiveMonitor::ExclusiveRead16(std::size_t core_index, VAddr addr) {
    return Read<u16>(core_index, addr, memory.Read16(addr));
}

u32 RustExclusiveMonitor::ExclusiveRead32(std::size_t core_index, VAddr addr) {
    return Read<u32>(core_index, addr, memory.Read32(addr));
}

u64 RustExclusiveMonitor::ExclusiveRead64(std::size_t core_index, VAddr addr) {
    return Read<u64>(core_index, addr, memory.Read64(addr));
}

void RustExclusiveMonitor::ClearExclusive(std::size_t core_index) {
    emu_cpu_monitor_clear(monitor, static_cast<u32>(core_index));
}

bool RustExclusiveMonitor::ExclusiveWrite8(std::size_t core_index, VAddr vaddr, u8 value) {
    return Write<u8>(core_index, vaddr, value, &Memory::MemorySystem::WriteExclusive8);
}

bool RustExclusiveMonitor::ExclusiveWrite16(std::size_t core_index, VAddr vaddr, u16 value) {
    return Write<u16>(core_index, vaddr, value, &Memory::MemorySystem::WriteExclusive16);
}

bool RustExclusiveMonitor::ExclusiveWrite32(std::size_t core_index, VAddr vaddr, u32 value) {
    return Write<u32>(core_index, vaddr, value, &Memory::MemorySystem::WriteExclusive32);
}

bool RustExclusiveMonitor::ExclusiveWrite64(std::size_t core_index, VAddr vaddr, u64 value) {
    return Write<u64>(core_index, vaddr, value, &Memory::MemorySystem::WriteExclusive64);
}

} // namespace Core
