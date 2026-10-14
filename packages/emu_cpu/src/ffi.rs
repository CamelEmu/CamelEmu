use std::cell::UnsafeCell;
use std::ptr::{self, NonNull};
use std::sync::atomic::Ordering;

use emu_mem::abi::PageTableView;

use crate::cpu::{Cpu, Shared};
use crate::exec::{self, Exit};
use crate::host::HostCallbacks;
use crate::monitor::ExclusiveMonitor;
use crate::state::CpuState;

/// A core as the C ABI hands it out. The CPU runs on the thread of the emulator, while the
/// shared part may be touched from any thread.
pub struct Core {
    shared: Box<Shared>,
    state: Box<UnsafeCell<CpuState>>,
    cpu: UnsafeCell<Cpu>,
}

impl Core {
    /// # Safety
    /// Only the thread of the emulator calls it, and never while the CPU runs.
    unsafe fn cpu(&self) -> &mut Cpu {
        unsafe { &mut *self.cpu.get() }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_cpu_monitor_create(core_count: u32) -> *mut ExclusiveMonitor {
    Box::into_raw(Box::new(ExclusiveMonitor::new(core_count as usize)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_monitor_destroy(monitor: *mut ExclusiveMonitor) {
    if !monitor.is_null() {
        drop(unsafe { Box::from_raw(monitor) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_monitor_clear(monitor: *const ExclusiveMonitor, core: u32) {
    if let Some(monitor) = unsafe { monitor.as_ref() } {
        monitor.clear_core(core as usize);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_monitor_mark(
    monitor: *const ExclusiveMonitor,
    core: u32,
    address: u32,
    value: u64,
) {
    if let Some(monitor) = unsafe { monitor.as_ref() } {
        monitor.mark(core as usize, address, value);
    }
}

/// Releases the reservation and writes the value that was read to `value`. Returns whether the
/// core held the reservation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_monitor_check_and_clear(
    monitor: *const ExclusiveMonitor,
    core: u32,
    address: u32,
    value: *mut u64,
) -> bool {
    let Some(monitor) = (unsafe { monitor.as_ref() }) else {
        return false;
    };
    match monitor.check_and_clear(core as usize, address) {
        Some(saved) => {
            if !value.is_null() {
                unsafe { value.write(saved) };
            }
            true
        }
        None => false,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_create(
    monitor: *mut ExclusiveMonitor,
    core_id: u32,
    host: *const HostCallbacks,
) -> *mut Core {
    let (Some(monitor), Some(host)) = (NonNull::new(monitor), unsafe { host.as_ref() }) else {
        return ptr::null_mut();
    };
    let shared = Box::new(Shared::default());
    let state = Box::new(UnsafeCell::new(CpuState::default()));
    let cpu = Cpu::new(
        NonNull::new(state.get()).unwrap(),
        NonNull::from(&*shared),
        monitor,
        *host,
        core_id as usize,
    );
    Box::into_raw(Box::new(Core {
        shared,
        state,
        cpu: UnsafeCell::new(cpu),
    }))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_destroy(core: *mut Core) {
    if !core.is_null() {
        drop(unsafe { Box::from_raw(core) });
    }
}

/// The registers, which the core reads and writes directly on the thread of the emulator.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_state(core: *const Core) -> *mut CpuState {
    unsafe { core.as_ref() }.map_or(ptr::null_mut(), |core| core.state.get())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_set_page_table(
    core: *const Core,
    view: PageTableView,
    key: *const std::ffi::c_void,
) {
    if let Some(core) = unsafe { core.as_ref() } {
        core.shared.request(|requests| requests.page_table = Some((view, key as usize)));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_run(core: *const Core) -> u32 {
    let Some(core) = (unsafe { core.as_ref() }) else {
        return Exit::Halted as u32;
    };
    exec::run(unsafe { core.cpu() }) as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_step(core: *const Core) -> u32 {
    let Some(core) = (unsafe { core.as_ref() }) else {
        return Exit::Halted as u32;
    };
    exec::step(unsafe { core.cpu() }) as u32
}

/// Asks a running CPU to return after the block it runs. It does nothing when the CPU is not
/// running, as dynarmic only halts a running JIT.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_halt(core: *const Core) {
    if let Some(core) = unsafe { core.as_ref() } {
        core.shared.halt_if_executing();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_is_executing(core: *const Core) -> bool {
    unsafe { core.as_ref() }.is_some_and(|core| core.shared.executing.load(Ordering::Acquire))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_invalidate_range(core: *const Core, start: u32, length: usize) {
    if let Some(core) = unsafe { core.as_ref() } {
        let length = length.min(u32::MAX as usize) as u32;
        core.shared.request(|requests| requests.invalidate.push((start, length)));
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_clear_cache(core: *const Core) {
    if let Some(core) = unsafe { core.as_ref() } {
        core.shared.request(|requests| requests.clear_cache = true);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_cpu_core_clear_exclusive(core: *const Core) {
    if let Some(core) = unsafe { core.as_ref() } {
        core.shared.exclusive_local.store(false, Ordering::Relaxed);
    }
}
