#ifndef EMU_CPU_H
#define EMU_CPU_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "emu_mem.h"

#ifdef __cplusplus
extern "C" {
#endif

/** An ARM11 core. It runs on the thread of the emulator, except for the functions that say so. */
typedef struct EmuCpuCore EmuCpuCore;

/** The global monitor of the exclusive accesses, shared by the cores. */
typedef struct EmuCpuMonitor EmuCpuMonitor;

/** The registers of a core, which the core reads and writes directly. */
typedef struct EmuCpuState {
  uint32_t regs[16];
  uint32_t cpsr;
  /** S0-S31, which overlap D0-D15, followed by D16-D31. */
  uint32_t ext_regs[64];
  uint32_t fpscr;
  uint32_t fpexc;
  uint32_t cp15_thread_uprw;
  uint32_t cp15_thread_uro;
} EmuCpuState;

/**
 * The functions of the core that the CPU calls. The exceptions have the values of
 * `Dynarmic::A32::Exception`. The memory functions are only called for the pages without a
 * pointer, and the exclusive writes return whether they stored.
 */
typedef struct EmuCpuHostCallbacks {
  void* user;
  void (*call_svc)(void* user, uint32_t swi);
  void (*exception_raised)(void* user, uint32_t pc, uint32_t exception);
  void (*add_ticks)(void* user, uint64_t ticks);
  uint64_t (*get_ticks_remaining)(void* user);
  uint8_t (*read8)(void* user, uint32_t vaddr);
  uint16_t (*read16)(void* user, uint32_t vaddr);
  uint32_t (*read32)(void* user, uint32_t vaddr);
  uint64_t (*read64)(void* user, uint32_t vaddr);
  void (*write8)(void* user, uint32_t vaddr, uint8_t value);
  void (*write16)(void* user, uint32_t vaddr, uint16_t value);
  void (*write32)(void* user, uint32_t vaddr, uint32_t value);
  void (*write64)(void* user, uint32_t vaddr, uint64_t value);
  bool (*write_exclusive8)(void* user, uint32_t vaddr, uint8_t value, uint8_t expected);
  bool (*write_exclusive16)(void* user, uint32_t vaddr, uint16_t value, uint16_t expected);
  bool (*write_exclusive32)(void* user, uint32_t vaddr, uint32_t value, uint32_t expected);
  bool (*write_exclusive64)(void* user, uint32_t vaddr, uint64_t value, uint64_t expected);
} EmuCpuHostCallbacks;

/** Why `emu_cpu_core_run` returned. */
enum {
  EMU_CPU_EXIT_TICKS = 0,
  EMU_CPU_EXIT_HALTED = 1,
  EMU_CPU_EXIT_EXCEPTION = 2,
};

EmuCpuMonitor* emu_cpu_monitor_create(uint32_t core_count);
void emu_cpu_monitor_destroy(EmuCpuMonitor* monitor);
/** May be called from any thread. */
void emu_cpu_monitor_clear(const EmuCpuMonitor* monitor, uint32_t core);
/** May be called from any thread. */
void emu_cpu_monitor_mark(const EmuCpuMonitor* monitor, uint32_t core, uint32_t address,
                          uint64_t value);
/** May be called from any thread. Returns whether the core held the reservation. */
bool emu_cpu_monitor_check_and_clear(const EmuCpuMonitor* monitor, uint32_t core,
                                     uint32_t address, uint64_t* value);

/** The callbacks are copied. */
EmuCpuCore* emu_cpu_core_create(EmuCpuMonitor* monitor, uint32_t core_id,
                                const EmuCpuHostCallbacks* host);
void emu_cpu_core_destroy(EmuCpuCore* core);
EmuCpuState* emu_cpu_core_state(const EmuCpuCore* core);
/**
 * Switches the page table, from the next block when the CPU runs. The key identifies the page
 * table for the cache of the translated code.
 */
void emu_cpu_core_set_page_table(const EmuCpuCore* core, EmuMemPageTableView view,
                                 const void* key);
uint32_t emu_cpu_core_run(const EmuCpuCore* core);
uint32_t emu_cpu_core_step(const EmuCpuCore* core);
/** May be called from any thread. */
void emu_cpu_core_halt(const EmuCpuCore* core);
/** May be called from any thread. */
bool emu_cpu_core_is_executing(const EmuCpuCore* core);
/** May be called from any thread. */
void emu_cpu_core_invalidate_range(const EmuCpuCore* core, uint32_t start, size_t length);
/** May be called from any thread. */
void emu_cpu_core_clear_cache(const EmuCpuCore* core);
/** May be called from any thread. */
void emu_cpu_core_clear_exclusive(const EmuCpuCore* core);

#ifdef __cplusplus
}
#endif

#endif
