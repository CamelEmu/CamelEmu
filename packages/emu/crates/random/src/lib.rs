/// # Safety
///
/// `out` must be valid for writes of `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_random_fill(out: *mut u8, len: usize) {
    if out.is_null() || len == 0 {
        return;
    }
    let buf = unsafe { std::slice::from_raw_parts_mut(out, len) };
    if getrandom::fill(buf).is_err() {
        std::process::abort();
    }
}
