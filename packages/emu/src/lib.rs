//! The single entry point of the Rust side. It only re-exports the public API of the other crates and
//! is the one crate built as a static library, so that the C ABI of every crate ends up in one archive.

pub mod api;
mod frb_generated;

pub use emu_app_scanner;
pub use emu_crypto;
pub use emu_http;
pub use emu_random;
pub use emu_storage;
pub use memory;
pub use session;
