// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#pragma once

#include <memory>
#include <set>
#include "common/common_types.h"
#include "core/arm/arm_interface.h"
#include "core/hle/kernel/svc.h"

struct EmuCpuCore;
struct EmuCpuState;

namespace Memory {
struct PageTable;
class MemorySystem;
} // namespace Memory

namespace Core {

class ExclusiveMonitor;
class System;

/// A core of the emu_cpu crate, which translates, times and runs the code, and reaches the memory
/// of the emu_mem crate without calling into the core.
class ARM_Rust final : public ARM_Interface {
public:
    explicit ARM_Rust(Core::System& system_, Memory::MemorySystem& memory_, u32 core_id_,
                      std::shared_ptr<Core::Timing::Timer> timer,
                      Core::ExclusiveMonitor& exclusive_monitor_);
    ~ARM_Rust() override;

    void Run() override;
    void Step() override;

    void SetPC(u32 pc) override;
    u32 GetPC() const override;
    u32 GetReg(int index) const override;
    void SetReg(int index, u32 value) override;
    u32 GetVFPReg(int index) const override;
    void SetVFPReg(int index, u32 value) override;
    u32 GetVFPSystemReg(VFPSystemRegister reg) const override;
    void SetVFPSystemReg(VFPSystemRegister reg, u32 value) override;
    u32 GetCPSR() const override;
    void SetCPSR(u32 cpsr) override;
    u32 GetCP15Register(CP15Register reg) const override;
    void SetCP15Register(CP15Register reg, u32 value) override;

    void SaveContext(ThreadContext& ctx) override;
    void LoadContext(const ThreadContext& ctx) override;

    void PrepareReschedule() override;

    void ClearInstructionCache() override;
    void InvalidateCacheRange(u32 start_address, std::size_t length) override;
    void ClearExclusiveState() override;
    void SetPageTable(const std::shared_ptr<Memory::PageTable>& page_table) override;

protected:
    std::shared_ptr<Memory::PageTable> GetPageTable() const override;

private:
    struct Callbacks;

    void ServeBreak();

    Core::System& system;
    Memory::MemorySystem& memory;
    Kernel::SVCContext svc_context;
    EmuCpuCore* core = nullptr;
    EmuCpuState* state = nullptr;

    std::shared_ptr<Memory::PageTable> current_page_table = nullptr;
    // The translated code is cached per page table, so the page tables stay alive as long as the
    // core to keep their addresses unique, as the JITs of ARM_Dynarmic keep them.
    std::set<std::shared_ptr<Memory::PageTable>> page_tables;
};

} // namespace Core
