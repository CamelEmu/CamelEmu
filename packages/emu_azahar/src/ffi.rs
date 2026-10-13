#[unsafe(no_mangle)]
pub extern "C" fn emu_azahar_version() -> u32 {
    crate::VERSION
}
