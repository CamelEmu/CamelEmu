use flutter_rust_bridge::frb;

use crate::api::audio::AudioEngine;
use crate::api::error::AzaharError;
use crate::registry;

/// Phase of the shader cache preparation reported by the core.
#[frb(unignore)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Prepare,
    Decompile,
    Build,
    Complete,
}

/// Lifecycle state of a session.
#[frb(unignore)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Running,
    Paused,
}

/// Notification delivered from the session to the application.
#[frb(non_opaque)]
#[derive(Debug, Clone)]
pub enum SessionEvent {
    ShaderProgress {
        stage: ShaderStage,
        progress: u64,
        max: u64,
    },
    Texture {
        texture_id: i64,
        secondary: bool,
    },
    StateChanged {
        state: SessionState,
    },
    Error {
        message: String,
    },
    ShutdownRequested,
    Failed {
        message: String,
    },
    ArticCountryMismatch {
        region: u32,
        country: u32,
    },
}

/// Size of the screen textures of a session and the engine that plays its audio.
#[derive(Debug, Clone, Copy)]
pub struct SessionOptions {
    pub primary_width: i32,
    pub primary_height: i32,
    pub secondary_width: i32,
    pub secondary_height: i32,
    pub dual_screen: bool,
    pub audio_engine: AudioEngine,
}

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
