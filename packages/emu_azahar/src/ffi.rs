use std::ffi::{CStr, c_char};
use std::ptr;

use crate::session::{ResultStatus, Session, SessionConfig};
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_azahar_session_create(config: *const SessionConfig) -> *mut Session {
    crate::init_logger();
    unsafe { config.as_ref() }
        .and_then(|config| Session::new(*config))
        .map_or(ptr::null_mut(), Box::into_raw)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_azahar_session_destroy(session: *mut Session) {
    if !session.is_null() {
        drop(unsafe { Box::from_raw(session) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_azahar_session_load(
    session: *mut Session,
    path: *const c_char,
) -> u32 {
    let (Some(session), false) = (unsafe { session.as_mut() }, path.is_null()) else {
        return ResultStatus::ErrorNotInitialized as u32;
    };
    let path = unsafe { CStr::from_ptr(path) }.to_string_lossy();
    session.load(&path) as u32
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_azahar_session_start(session: *mut Session) -> u32 {
    unsafe { session.as_mut() }.map_or(ResultStatus::ErrorNotInitialized as u32, |session| {
        session.start() as u32
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_azahar_session_pause(session: *mut Session) {
    if let Some(session) = unsafe { session.as_mut() } {
        session.pause();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_azahar_session_stop(session: *mut Session) {
    if let Some(session) = unsafe { session.as_mut() } {
        session.stop();
    }
}
