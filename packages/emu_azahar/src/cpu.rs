use std::ffi::c_void;
use std::ptr::NonNull;

use emu_cpu::HostCallbacks;
use emu_mem::abi::PageTableView;

#[repr(C)]
pub struct CpuState {
    pub regs: [u32; 16],
    pub cpsr: u32,
    pub ext_regs: [u32; 64],
    pub fpscr: u32,
    pub fpexc: u32,
    pub cp15_thread_uprw: u32,
    pub cp15_thread_uro: u32,
}

unsafe extern "C" {
    fn emu_cpu_monitor_create(core_count: u32) -> *mut c_void;
    fn emu_cpu_monitor_destroy(monitor: *mut c_void);
    fn emu_cpu_core_create(
        monitor: *mut c_void,
        core_id: u32,
        host: *const HostCallbacks,
    ) -> *mut c_void;
    fn emu_cpu_core_destroy(core: *mut c_void);
    fn emu_cpu_core_state(core: *const c_void) -> *mut CpuState;
    fn emu_cpu_core_set_page_table(core: *const c_void, view: PageTableView, key: *const c_void);
    fn emu_cpu_core_run(core: *const c_void) -> u32;
    fn emu_cpu_core_halt(core: *const c_void);
}

pub struct Monitor(NonNull<c_void>);

impl Monitor {
    pub fn new(core_count: u32) -> Option<Self> {
        NonNull::new(unsafe { emu_cpu_monitor_create(core_count) }).map(Self)
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        unsafe { emu_cpu_monitor_destroy(self.0.as_ptr()) };
    }
}

pub struct CpuCore(NonNull<c_void>);

unsafe impl Send for CpuCore {}
unsafe impl Sync for CpuCore {}

impl CpuCore {
    pub fn new(monitor: &Monitor, core_id: u32, host: &HostCallbacks) -> Option<Self> {
        NonNull::new(unsafe { emu_cpu_core_create(monitor.0.as_ptr(), core_id, host) }).map(Self)
    }

    pub fn state(&self) -> *mut CpuState {
        unsafe { emu_cpu_core_state(self.0.as_ptr()) }
    }

    pub fn set_page_table(&self, view: PageTableView, key: *const c_void) {
        unsafe { emu_cpu_core_set_page_table(self.0.as_ptr(), view, key) };
    }

    pub fn run(&self) -> u32 {
        unsafe { emu_cpu_core_run(self.0.as_ptr()) }
    }

    pub fn halt(&self) {
        unsafe { emu_cpu_core_halt(self.0.as_ptr()) };
    }
}

impl Drop for CpuCore {
    fn drop(&mut self) {
        unsafe { emu_cpu_core_destroy(self.0.as_ptr()) };
    }
}
