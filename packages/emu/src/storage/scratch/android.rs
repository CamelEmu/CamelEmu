use std::ffi::CString;
use std::fs::File;
use std::os::fd::{FromRawFd, RawFd};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use jni::objects::JString;

use crate::storage::android_jni::call;

static CACHE_DIRECTORY: OnceLock<String> = OnceLock::new();
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn cache_directory() -> Option<&'static str> {
    if let Some(directory) = CACHE_DIRECTORY.get() {
        return Some(directory);
    }
    let directory = call(|env, class| {
        let value = env
            .call_static_method(class, "getCacheDirectory", "()Ljava/lang/String;", &[])?
            .l()?;
        Ok(String::from(env.get_string(&JString::from(value))?))
    })?;
    Some(CACHE_DIRECTORY.get_or_init(|| directory))
}

/// Creates a file in the cache directory of the application and removes its name at once.
///
/// The SELinux policy of Android denies reopening a memory file through `/proc/self/fd`, while it
/// allows it for the files of the application.
pub fn create() -> Option<File> {
    let directory = cache_directory()?;
    let name = format!(
        "{directory}/azahar-scratch-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let path = CString::new(name).ok()?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return None;
    }
    unsafe { libc::unlink(path.as_ptr()) };
    Some(unsafe { File::from_raw_fd(fd as RawFd) })
}
