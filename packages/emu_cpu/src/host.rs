use std::ffi::c_void;

/// The functions of the core that the CPU calls. It is `EmuCpuHostCallbacks` of `emu_cpu.h`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostCallbacks {
    pub user: *mut c_void,
    pub call_svc: unsafe extern "C" fn(*mut c_void, u32),
    pub exception_raised: unsafe extern "C" fn(*mut c_void, u32, u32),
    pub add_ticks: unsafe extern "C" fn(*mut c_void, u64),
    pub get_ticks_remaining: unsafe extern "C" fn(*mut c_void) -> u64,
    pub read8: unsafe extern "C" fn(*mut c_void, u32) -> u8,
    pub read16: unsafe extern "C" fn(*mut c_void, u32) -> u16,
    pub read32: unsafe extern "C" fn(*mut c_void, u32) -> u32,
    pub read64: unsafe extern "C" fn(*mut c_void, u32) -> u64,
    pub write8: unsafe extern "C" fn(*mut c_void, u32, u8),
    pub write16: unsafe extern "C" fn(*mut c_void, u32, u16),
    pub write32: unsafe extern "C" fn(*mut c_void, u32, u32),
    pub write64: unsafe extern "C" fn(*mut c_void, u32, u64),
    pub write_exclusive8: unsafe extern "C" fn(*mut c_void, u32, u8, u8) -> bool,
    pub write_exclusive16: unsafe extern "C" fn(*mut c_void, u32, u16, u16) -> bool,
    pub write_exclusive32: unsafe extern "C" fn(*mut c_void, u32, u32, u32) -> bool,
    pub write_exclusive64: unsafe extern "C" fn(*mut c_void, u32, u64, u64) -> bool,
}

/// The values of `Dynarmic::A32::Exception`, which the core already handles.
#[allow(dead_code)]
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exception {
    UndefinedInstruction = 0,
    UnpredictableInstruction = 1,
    DecodeError = 2,
    SendEvent = 3,
    SendEventLocal = 4,
    WaitForInterrupt = 5,
    WaitForEvent = 6,
    Yield = 7,
    Breakpoint = 8,
    PreloadData = 9,
    PreloadDataWithIntentToWrite = 10,
    PreloadInstruction = 11,
    NoExecuteFault = 12,
}
