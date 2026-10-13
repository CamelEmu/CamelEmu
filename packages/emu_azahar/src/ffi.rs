use std::ptr;

use crate::{Config, EmuAzahar};

#[unsafe(no_mangle)]
pub extern "C" fn emu_azahar_version() -> u32 {
    crate::VERSION
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_azahar_create(config: *const Config) -> *mut EmuAzahar {
    unsafe { config.as_ref() }.map_or(ptr::null_mut(), |config| {
        Box::into_raw(Box::new(EmuAzahar::new(*config)))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_azahar_destroy(core: *mut EmuAzahar) {
    if !core.is_null() {
        drop(unsafe { Box::from_raw(core) });
    }
}
