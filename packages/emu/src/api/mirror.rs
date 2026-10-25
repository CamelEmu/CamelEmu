//! Declarations of the types of the `session` crate that cross the Dart boundary.
//!
//! The code generator cannot read the definition of a type that lives in another crate and would expose
//! it as an opaque handle, so each one is mirrored here. The mirrors must match the originals.

use flutter_rust_bridge::frb;
pub use session::api::audio::AudioEngine;
pub use session::error::AzaharError;
pub use session::session::{SessionEvent, SessionOptions, SessionState, ShaderStage};

/// Engine that plays the audio of a session.
#[frb(mirror(AudioEngine))]
pub enum _AudioEngine {
    /// The OpenAL sink of the core.
    OpenAl,
    /// Oboe, driven by this crate. Only available on Android.
    Oboe,
}

/// Errors surfaced by the session API.
#[frb(mirror(AzaharError))]
pub enum _AzaharError {
    /// A session is still alive, so a second one cannot be started.
    SessionActive,
    /// The operation needs a session but none is active.
    NoSession,
    /// The game path contains an interior NUL byte.
    InvalidPath,
    /// The C++ core refused to create the session.
    CreateFailed,
    /// The operation is not valid in the current session state.
    InvalidState,
    /// The core returned a non-zero status code.
    Core(i32),
    /// The requested 3DS address range is not mapped.
    InvalidAddress { address: u32, len: u32 },
    /// The core library could not be loaded or initialized.
    LibraryLoad(String),
    /// The audio output could not be opened.
    Audio(String),
    /// A folder of a user directory could not be created.
    Storage(String),
    /// Some of the files the game changed could not be written to the storage.
    StorageCommit(Vec<String>),
}

/// Phase of the shader cache preparation reported by the core.
#[frb(mirror(ShaderStage))]
pub enum _ShaderStage {
    Prepare,
    Decompile,
    Build,
    Complete,
}

/// Lifecycle state of a session.
#[frb(mirror(SessionState))]
pub enum _SessionState {
    Idle,
    Running,
    Paused,
}

/// Notification delivered from the session to the application.
#[frb(mirror(SessionEvent))]
pub enum _SessionEvent {
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
#[frb(mirror(SessionOptions))]
pub struct _SessionOptions {
    pub primary_width: i32,
    pub primary_height: i32,
    pub secondary_width: i32,
    pub secondary_height: i32,
    pub dual_screen: bool,
    pub audio_engine: AudioEngine,
}
