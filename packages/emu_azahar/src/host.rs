use std::ffi::c_void;
use std::ptr::NonNull;

use emu_cpu::HostCallbacks;
use emu_kernel::{Kernel, Timer};

use crate::cpu::CpuState;
use crate::memory::Memory;

pub struct System {
    pub memory: Memory,
    pub kernel: Kernel,
}

const UNDEFINED_INSTRUCTION: u32 = 0;
const UNPREDICTABLE_INSTRUCTION: u32 = 1;
const DECODE_ERROR: u32 = 2;
const SEND_EVENT: u32 = 3;
const SEND_EVENT_LOCAL: u32 = 4;
const WAIT_FOR_INTERRUPT: u32 = 5;
const WAIT_FOR_EVENT: u32 = 6;
const YIELD: u32 = 7;
const BREAKPOINT: u32 = 8;
const PRELOAD_DATA: u32 = 9;
const PRELOAD_DATA_WITH_INTENT_TO_WRITE: u32 = 10;
const PRELOAD_INSTRUCTION: u32 = 11;
const NO_EXECUTE_FAULT: u32 = 12;

pub struct Host {
    system: NonNull<System>,
    pub timer: Timer,
    pub state: *mut CpuState,
}

impl Host {
    pub fn new(system: NonNull<System>, timer: Timer) -> Self {
        Self {
            system,
            timer,
            state: std::ptr::null_mut(),
        }
    }

    fn system(&mut self) -> &mut System {
        unsafe { self.system.as_mut() }
    }

    fn memory(&mut self) -> &mut Memory {
        &mut self.system().memory
    }

    pub fn callbacks(&mut self) -> HostCallbacks {
        HostCallbacks {
            user: (self as *mut Self).cast(),
            call_svc,
            exception_raised,
            add_ticks,
            get_ticks_remaining,
            read8,
            read16,
            read32,
            read64,
            write8,
            write16,
            write32,
            write64,
            write_exclusive8,
            write_exclusive16,
            write_exclusive32,
            write_exclusive64,
        }
    }

    fn reg(&self, index: usize) -> u32 {
        if self.state.is_null() {
            return 0;
        }
        unsafe { (*self.state).regs[index] }
    }
}

unsafe fn host<'a>(user: *mut c_void) -> &'a mut Host {
    unsafe { &mut *user.cast::<Host>() }
}

unsafe extern "C" fn call_svc(user: *mut c_void, swi: u32) {
    unsafe { host(user) }.system().kernel.call_svc(swi);
}

unsafe extern "C" fn exception_raised(user: *mut c_void, pc: u32, raw_exception: u32) {
    let host = unsafe { host(user) };
    match raw_exception {
        UNDEFINED_INSTRUCTION | UNPREDICTABLE_INSTRUCTION | DECODE_ERROR | NO_EXECUTE_FAULT => {}
        BREAKPOINT => {}
        SEND_EVENT
        | SEND_EVENT_LOCAL
        | WAIT_FOR_INTERRUPT
        | WAIT_FOR_EVENT
        | YIELD
        | PRELOAD_DATA
        | PRELOAD_DATA_WITH_INTENT_TO_WRITE
        | PRELOAD_INSTRUCTION => return,
        _ => {}
    }
    for i in 0..16 {
        log::error!(target: "Debug", "r{:02} = {:08X}", i, host.reg(i));
    }
    let code = host.memory().read32(pc);
    panic!("ExceptionRaised(exception = {raw_exception}, pc = {pc:08X}, code = {code:08X})");
}

unsafe extern "C" fn add_ticks(user: *mut c_void, ticks: u64) {
    unsafe { host(user) }.timer.add_ticks(ticks);
}

unsafe extern "C" fn get_ticks_remaining(user: *mut c_void) -> u64 {
    let ticks = unsafe { host(user) }.timer.downcount();
    if ticks <= 0 { 0 } else { ticks as u64 }
}

unsafe extern "C" fn read8(user: *mut c_void, vaddr: u32) -> u8 {
    unsafe { host(user) }.memory().read8(vaddr)
}

unsafe extern "C" fn read16(user: *mut c_void, vaddr: u32) -> u16 {
    unsafe { host(user) }.memory().read16(vaddr)
}

unsafe extern "C" fn read32(user: *mut c_void, vaddr: u32) -> u32 {
    unsafe { host(user) }.memory().read32(vaddr)
}

unsafe extern "C" fn read64(user: *mut c_void, vaddr: u32) -> u64 {
    unsafe { host(user) }.memory().read64(vaddr)
}

unsafe extern "C" fn write8(user: *mut c_void, vaddr: u32, value: u8) {
    unsafe { host(user) }.memory().write8(vaddr, value);
}

unsafe extern "C" fn write16(user: *mut c_void, vaddr: u32, value: u16) {
    unsafe { host(user) }.memory().write16(vaddr, value);
}

unsafe extern "C" fn write32(user: *mut c_void, vaddr: u32, value: u32) {
    unsafe { host(user) }.memory().write32(vaddr, value);
}

unsafe extern "C" fn write64(user: *mut c_void, vaddr: u32, value: u64) {
    unsafe { host(user) }.memory().write64(vaddr, value);
}

unsafe extern "C" fn write_exclusive8(
    user: *mut c_void,
    vaddr: u32,
    value: u8,
    expected: u8,
) -> bool {
    unsafe { host(user) }
        .memory()
        .write_exclusive8(vaddr, value, expected)
}

unsafe extern "C" fn write_exclusive16(
    user: *mut c_void,
    vaddr: u32,
    value: u16,
    expected: u16,
) -> bool {
    unsafe { host(user) }
        .memory()
        .write_exclusive16(vaddr, value, expected)
}

unsafe extern "C" fn write_exclusive32(
    user: *mut c_void,
    vaddr: u32,
    value: u32,
    expected: u32,
) -> bool {
    unsafe { host(user) }
        .memory()
        .write_exclusive32(vaddr, value, expected)
}

unsafe extern "C" fn write_exclusive64(
    user: *mut c_void,
    vaddr: u32,
    value: u64,
    expected: u64,
) -> bool {
    unsafe { host(user) }
        .memory()
        .write_exclusive64(vaddr, value, expected)
}
