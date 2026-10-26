use std::ptr;
use std::slice;

use crate::time_stretch::TimeStretcher;

pub struct EmuTimeStretcher(TimeStretcher);

#[unsafe(no_mangle)]
pub extern "C" fn emu_time_stretcher_new() -> *mut EmuTimeStretcher {
    match TimeStretcher::new() {
        Ok(stretcher) => Box::into_raw(Box::new(EmuTimeStretcher(stretcher))),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_time_stretcher_free(handle: *mut EmuTimeStretcher) {
    if !handle.is_null() {
        drop(unsafe { Box::from_raw(handle) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_time_stretcher_set_output_sample_rate(
    handle: *mut EmuTimeStretcher,
    sample_rate: u32,
) -> i32 {
    let Some(stretcher) = (unsafe { handle.as_mut() }) else {
        return -1;
    };
    match stretcher.0.set_output_sample_rate(sample_rate) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_time_stretcher_process(
    handle: *mut EmuTimeStretcher,
    input: *const i16,
    num_in: usize,
    out: *mut i16,
    num_out: usize,
) -> usize {
    let Some(stretcher) = (unsafe { handle.as_mut() }) else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    let stretcher = &mut stretcher.0;
    let channels = stretcher.channels();
    let input = if input.is_null() {
        &[]
    } else {
        unsafe { slice::from_raw_parts(input, num_in * channels) }
    };
    let out = unsafe { slice::from_raw_parts_mut(out, num_out * channels) };
    stretcher.process(input, num_in, out, num_out).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_time_stretcher_clear(handle: *mut EmuTimeStretcher) {
    if let Some(stretcher) = unsafe { handle.as_mut() } {
        stretcher.0.clear();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_time_stretcher_flush(handle: *mut EmuTimeStretcher) -> i32 {
    let Some(stretcher) = (unsafe { handle.as_mut() }) else {
        return -1;
    };
    match stretcher.0.flush() {
        Ok(()) => 0,
        Err(_) => -1,
    }
}
