use std::ffi::c_void;
use std::sync::Mutex;

static WINDOWS: Mutex<[usize; 2]> = Mutex::new([0; 2]);

pub fn window(is_secondary: bool) -> *mut c_void {
    WINDOWS.lock().map_or(std::ptr::null_mut(), |windows| {
        windows[usize::from(is_secondary)] as *mut c_void
    })
}

#[cfg(target_os = "android")]
mod android {
    use std::ffi::c_void;

    use super::WINDOWS;

    #[link(name = "android")]
    unsafe extern "C" {
        fn ANativeWindow_fromSurface(env: *mut c_void, surface: *mut c_void) -> *mut c_void;
        fn ANativeWindow_release(window: *mut c_void);
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "system" fn Java_com_karasu256_azahar_1reloaded_lib_azahar_1for_1flutter_EmuAzaharLibrary_surfaceChanged(
        env: *mut c_void,
        _class: *mut c_void,
        surface: *mut c_void,
        is_secondary: u8,
    ) {
        let window = if surface.is_null() {
            std::ptr::null_mut()
        } else {
            unsafe { ANativeWindow_fromSurface(env, surface) }
        };
        let Ok(mut windows) = WINDOWS.lock() else {
            return;
        };
        let slot = &mut windows[usize::from(is_secondary != 0)];
        if *slot != 0 {
            unsafe { ANativeWindow_release(*slot as *mut c_void) };
        }
        *slot = window as usize;
    }
}
