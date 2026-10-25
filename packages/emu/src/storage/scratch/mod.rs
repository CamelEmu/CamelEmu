//! The anonymous files the overlay keeps its contents in. How such a file is made depends on what
//! the platform lets an application reopen through `/proc/self/fd`, so each platform has a module
//! of its own that exposes the same `create`.

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "android")]
pub(super) use android::create;
#[cfg(target_os = "linux")]
pub(super) use linux::create;
