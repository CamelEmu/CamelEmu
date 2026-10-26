//! Rust-owned 3DS game session, audio output and file storage. The memory of a session is read
//! through the `memory` crate.
//!
//! The C++ core is reached only through the C ABI declared in
//! `packages/native/include/emu_session.h`, and it reaches the storage of the crate through the one declared
//! in `packages/native/include/emu_storage.h`. The unsafe surface is confined to [`ffi`] and the storage ABI.

pub mod api;
#[cfg(target_os = "android")]
pub mod audio;
pub mod capture;
pub mod error;
mod ffi;
pub mod json;
mod library;
mod raw;
pub mod registry;
pub mod session;
