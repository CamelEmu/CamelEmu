use std::fs::File;
use std::os::fd::{FromRawFd, RawFd};

/// Creates an anonymous file in memory.
pub fn create() -> Option<File> {
    let name = c"azahar-storage";
    let fd = unsafe { libc::syscall(libc::SYS_memfd_create, name.as_ptr(), libc::MFD_CLOEXEC) };
    if fd < 0 {
        return None;
    }
    Some(unsafe { File::from_raw_fd(fd as RawFd) })
}
