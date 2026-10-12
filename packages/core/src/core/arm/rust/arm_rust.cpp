// Copyright Citra Emulator Project / Azahar Emulator Project
// Licensed under GPLv2 or any later version
// Refer to the license.txt file included.

#include <algorithm>
#include <emu_cpu.h>
#include "common/assert.h"
#include "common/logging/log.h"
#include "common/microprofile.h"
#include "core/arm/rust/arm_rust.h"
#include "core/arm/rust/rust_exclusive_monitor.h"
#include "core/core.h"
#include "core/core_timing.h"
#include "core/gdbstub/gdbstub.h"
#include "core/hle/kernel/kernel.h"
#include "core/hle/kernel/thread.h"
#include "core/memory.h"

namespace Core {

namespace {

// The values of Dynarmic::A32::Exception, which emu_cpu reports.
enum class Exception : u32 {
    UndefinedInstruction,
    UnpredictableInstruction,
    DecodeError,
    SendEvent,
    SendEventLocal,
    WaitForInterrupt,
    WaitForEvent,
    Yield,
    Breakpoint,
    PreloadData,
    PreloadDataWithIntentToWrite,
    PreloadInstruction,
    NoExecuteFault,
};

} // Anonymous namespace

struct ARM_Rust::Callbacks {
    static ARM_Rust& Parent(void* user) {
        return *static_cast<ARM_Rust*>(user);
    }

    static void CallSVC(void* user, u32 swi) {
        Parent(user).svc_context.CallSVC(swi);
    }

    static void ExceptionRaised(void* user, u32 pc, u32 raw_exception) {
        auto& parent = Parent(user);
        const auto exception = static_cast<Exception>(raw_exception);
        switch (exception) {
        case Exception::UndefinedInstruction:
        case Exception::UnpredictableInstruction:
        case Exception::DecodeError:
        case Exception::NoExecuteFault:
            break;
        case Exception::Breakpoint:
            if (GDBStub::IsConnected()) {
                emu_cpu_core_halt(parent.core);
                parent.SetPC(pc);
                parent.ServeBreak();
                return;
            }
            break;
        case Exception::SendEvent:
        case Exception::SendEventLocal:
        case Exception::WaitForInterrupt:
        case Exception::WaitForEvent:
        case Exception::Yield:
        case Exception::PreloadData:
        case Exception::PreloadDataWithIntentToWrite:
        case Exception::PreloadInstruction:
            return;
        }
        for (int i = 0; i < 16; i++) {
            LOG_CRITICAL(Debug, "r{:02d} = {:08X}", i, parent.GetReg(i));
        }
        ASSERT_MSG(false, "ExceptionRaised(exception = {}, pc = {:08X}, code = {:08X})",
                   raw_exception, pc, parent.memory.Read32(pc));
    }

    static void AddTicks(void* user, u64 ticks) {
        Parent(user).GetTimer().AddTicks(ticks);
    }

    static u64 GetTicksRemaining(void* user) {
        const s64 ticks = Parent(user).GetTimer().GetDowncount();
        return static_cast<u64>(ticks <= 0 ? 0 : ticks);
    }

    static u8 Read8(void* user, u32 vaddr) {
        return Parent(user).memory.Read8(vaddr);
    }
    static u16 Read16(void* user, u32 vaddr) {
        return Parent(user).memory.Read16(vaddr);
    }
    static u32 Read32(void* user, u32 vaddr) {
        return Parent(user).memory.Read32(vaddr);
    }
    static u64 Read64(void* user, u32 vaddr) {
        return Parent(user).memory.Read64(vaddr);
    }

    static void Write8(void* user, u32 vaddr, u8 value) {
        Parent(user).memory.Write8(vaddr, value);
    }
    static void Write16(void* user, u32 vaddr, u16 value) {
        Parent(user).memory.Write16(vaddr, value);
    }
    static void Write32(void* user, u32 vaddr, u32 value) {
        Parent(user).memory.Write32(vaddr, value);
    }
    static void Write64(void* user, u32 vaddr, u64 value) {
        Parent(user).memory.Write64(vaddr, value);
    }

    static bool WriteExclusive8(void* user, u32 vaddr, u8 value, u8 expected) {
        return Parent(user).memory.WriteExclusive8(vaddr, value, expected);
    }
    static bool WriteExclusive16(void* user, u32 vaddr, u16 value, u16 expected) {
        return Parent(user).memory.WriteExclusive16(vaddr, value, expected);
    }
    static bool WriteExclusive32(void* user, u32 vaddr, u32 value, u32 expected) {
        return Parent(user).memory.WriteExclusive32(vaddr, value, expected);
    }
    static bool WriteExclusive64(void* user, u32 vaddr, u64 value, u64 expected) {
        return Parent(user).memory.WriteExclusive64(vaddr, value, expected);
    }
};

ARM_Rust::ARM_Rust(Core::System& system_, Memory::MemorySystem& memory_, u32 core_id_,
                   std::shared_ptr<Core::Timing::Timer> timer_,
                   Core::ExclusiveMonitor& exclusive_monitor_)
    : ARM_Interface(core_id_, timer_), system(system_), memory(memory_), svc_context(system_) {
    ASSERT_MSG(memory.IsEmuMemEnabled(), "ARM_Rust needs the memory of emu_mem");
    const EmuCpuHostCallbacks callbacks{
        .user = this,
        .call_svc = &Callbacks::CallSVC,
        .exception_raised = &Callbacks::ExceptionRaised,
        .add_ticks = &Callbacks::AddTicks,
        .get_ticks_remaining = &Callbacks::GetTicksRemaining,
        .read8 = &Callbacks::Read8,
        .read16 = &Callbacks::Read16,
        .read32 = &Callbacks::Read32,
        .read64 = &Callbacks::Read64,
        .write8 = &Callbacks::Write8,
        .write16 = &Callbacks::Write16,
        .write32 = &Callbacks::Write32,
        .write64 = &Callbacks::Write64,
        .write_exclusive8 = &Callbacks::WriteExclusive8,
        .write_exclusive16 = &Callbacks::WriteExclusive16,
        .write_exclusive32 = &Callbacks::WriteExclusive32,
        .write_exclusive64 = &Callbacks::WriteExclusive64,
    };
    auto& monitor = dynamic_cast<RustExclusiveMonitor&>(exclusive_monitor_);
    core = emu_cpu_core_create(monitor.GetMonitor(), core_id_, &callbacks);
    ASSERT_MSG(core, "emu_cpu could not create a core");
    state = emu_cpu_core_state(core);
    SetPageTable(memory.GetCurrentPageTable());
}

ARM_Rust::~ARM_Rust() {
    emu_cpu_core_destroy(core);
}

MICROPROFILE_DEFINE(ARM_Rust, "ARM Rust", "ARM Rust", MP_RGB(255, 128, 64));

void ARM_Rust::Run() {
    ASSERT(memory.GetCurrentPageTable() == current_page_table);
    MICROPROFILE_SCOPE(ARM_Rust);

    emu_cpu_core_run(core);
}

void ARM_Rust::Step() {
    emu_cpu_core_step(core);

    if (GDBStub::IsConnected()) {
        ServeBreak();
    }
}

void ARM_Rust::SetPC(u32 pc) {
    state->regs[15] = pc;
}

u32 ARM_Rust::GetPC() const {
    return state->regs[15];
}

u32 ARM_Rust::GetReg(int index) const {
    return state->regs[index];
}

void ARM_Rust::SetReg(int index, u32 value) {
    state->regs[index] = value;
}

u32 ARM_Rust::GetVFPReg(int index) const {
    return state->ext_regs[index];
}

void ARM_Rust::SetVFPReg(int index, u32 value) {
    state->ext_regs[index] = value;
}

u32 ARM_Rust::GetVFPSystemReg(VFPSystemRegister reg) const {
    switch (reg) {
    case VFP_FPSCR:
        return state->fpscr;
    case VFP_FPEXC:
        return state->fpexc;
    default:
        UNREACHABLE_MSG("Unknown VFP system register: {}", reg);
    }

    return UINT_MAX;
}

void ARM_Rust::SetVFPSystemReg(VFPSystemRegister reg, u32 value) {
    switch (reg) {
    case VFP_FPSCR:
        state->fpscr = value;
        return;
    case VFP_FPEXC:
        state->fpexc = value;
        return;
    default:
        UNREACHABLE_MSG("Unknown VFP system register: {}", reg);
    }
}

u32 ARM_Rust::GetCPSR() const {
    return state->cpsr;
}

void ARM_Rust::SetCPSR(u32 cpsr) {
    state->cpsr = cpsr;
}

u32 ARM_Rust::GetCP15Register(CP15Register reg) const {
    switch (reg) {
    case CP15_THREAD_UPRW:
        return state->cp15_thread_uprw;
    case CP15_THREAD_URO:
        return state->cp15_thread_uro;
    default:
        UNREACHABLE_MSG("Unknown CP15 register: {}", reg);
    }

    return 0;
}

void ARM_Rust::SetCP15Register(CP15Register reg, u32 value) {
    switch (reg) {
    case CP15_THREAD_UPRW:
        state->cp15_thread_uprw = value;
        return;
    case CP15_THREAD_URO:
        state->cp15_thread_uro = value;
        return;
    default:
        UNREACHABLE_MSG("Unknown CP15 register: {}", reg);
    }
}

void ARM_Rust::SaveContext(ThreadContext& ctx) {
    std::copy(std::begin(state->regs), std::end(state->regs), ctx.cpu_registers.begin());
    ctx.cpsr = state->cpsr;
    std::copy(std::begin(state->ext_regs), std::end(state->ext_regs), ctx.fpu_registers.begin());
    ctx.fpscr = state->fpscr;
    ctx.fpexc = state->fpexc;
}

void ARM_Rust::LoadContext(const ThreadContext& ctx) {
    std::copy(ctx.cpu_registers.begin(), ctx.cpu_registers.end(), std::begin(state->regs));
    state->cpsr = ctx.cpsr;
    std::copy(ctx.fpu_registers.begin(), ctx.fpu_registers.end(), std::begin(state->ext_regs));
    state->fpscr = ctx.fpscr;
    state->fpexc = ctx.fpexc;
}

void ARM_Rust::PrepareReschedule() {
    emu_cpu_core_halt(core);
}

void ARM_Rust::ClearInstructionCache() {
    emu_cpu_core_clear_cache(core);
}

void ARM_Rust::InvalidateCacheRange(u32 start_address, std::size_t length) {
    emu_cpu_core_invalidate_range(core, start_address, length);
}

void ARM_Rust::ClearExclusiveState() {
    emu_cpu_core_clear_exclusive(core);
}

std::shared_ptr<Memory::PageTable> ARM_Rust::GetPageTable() const {
    return current_page_table;
}

void ARM_Rust::SetPageTable(const std::shared_ptr<Memory::PageTable>& page_table) {
    current_page_table = page_table;
    EmuMemPageTableView view{};
    if (page_table) {
        page_tables.insert(page_table);
        ASSERT_MSG(page_table->GetEmuMem(), "The page table was not created by emu_mem");
        view = emu_mem_pt_view(page_table->GetEmuMem());
    }
    emu_cpu_core_set_page_table(core, view, page_table.get());
}

void ARM_Rust::ServeBreak() {
    Kernel::Thread* thread = system.Kernel().GetCurrentThreadManager().GetCurrentThread();
    SaveContext(thread->context);
    GDBStub::Break();
    GDBStub::SendTrap(thread, 5);
}

} // namespace Core
