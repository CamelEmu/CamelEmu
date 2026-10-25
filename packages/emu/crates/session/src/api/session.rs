use flutter_rust_bridge::frb;

use crate::error::AzaharError;
use crate::registry;
pub use crate::session::{SessionEvent, SessionOptions, SessionState, ShaderStage};

#[frb(init)]
pub fn init_app() {
    flutter_rust_bridge::setup_backtrace();
}

pub fn pause_game() -> Result<(), AzaharError> {
    registry::with_session(|session| session.pause())
}

pub fn resume_game() -> Result<(), AzaharError> {
    registry::with_session(|session| session.resume())
}

/// Stops the game and returns after all of its resources have been freed.
///
/// With `persist`, the saves, system data and titles the game changed are
/// written to the storage; otherwise the changes are discarded.
pub fn stop_game(persist: bool) -> Result<(), AzaharError> {
    registry::stop(persist)
}

pub fn answer_artic_country(country: Option<u32>) -> Result<(), AzaharError> {
    registry::with_session(|session| {
        session.answer_artic_country(country);
        Ok(())
    })
}

pub fn set_volume(volume: f32) -> Result<(), AzaharError> {
    registry::with_session(|session| {
        session.set_volume(volume);
        Ok(())
    })
}

/// Reads `len` bytes of 3DS memory starting at the virtual address `address`.
pub fn read_memory(address: u32, len: u32) -> Result<Vec<u8>, AzaharError> {
    registry::with_session(|session| Ok(session.memory().read_block(address, len as usize)?))
}

pub fn read_u32(address: u32) -> Result<u32, AzaharError> {
    registry::with_session(|session| Ok(session.memory().read_u32(address)?))
}

pub fn fcram_size() -> Result<u64, AzaharError> {
    registry::with_session(|session| Ok(session.memory().fcram_size() as u64))
}

pub fn dump_fcram() -> Result<Vec<u8>, AzaharError> {
    registry::with_session(|session| Ok(session.memory().dump_fcram()?))
}
