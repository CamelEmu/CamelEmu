//! The black-box game session.

use std::sync::Arc;

use crate::api::audio::AudioEngine;
#[cfg(target_os = "android")]
use crate::audio::AudioOutput;
use crate::capture::{CaptureBroker, CoreCapture, ScreenImage};
use crate::error::{AzaharError, SessionResult};
use crate::ffi::{CallbackContext, CoreListener, RawScreenshot, SessionHandle};
use memory::Memory3ds;

pub use crate::api::session::{SessionEvent, SessionOptions, SessionState, ShaderStage};

/// Audio output opened by this crate. Only Oboe is played here; the other engines are owned by
/// the core.
#[cfg(not(target_os = "android"))]
struct AudioOutput;

#[cfg(not(target_os = "android"))]
impl AudioOutput {
    fn push(&self, _frames: &[i16]) {}
    fn pause(&self) {}
    fn resume(&self) {}
    fn set_volume(&self, _volume: f32) {}
}

/// Callback receiving the events of one session.
pub type EventSink = Arc<dyn Fn(SessionEvent) + Send + Sync>;

struct Listener {
    events: EventSink,
    audio: Option<Arc<AudioOutput>>,
    captures: Arc<CaptureBroker>,
}

impl CoreListener for Listener {
    fn on_screenshot(&self, request_id: u64, screenshot: Option<RawScreenshot<'_>>) {
        self.captures.deliver(
            request_id,
            CoreCapture::Screenshot(screenshot.map(ScreenImage::from)),
        );
    }

    fn on_capture_3d(&self, request_id: u64, data: &[u8]) {
        self.captures
            .deliver(request_id, CoreCapture::Data(data.to_vec()));
    }

    fn on_event(&self, event: SessionEvent) {
        (self.events)(event);
    }

    fn on_audio(&self, frames: &[i16]) {
        if let Some(audio) = &self.audio {
            audio.push(frames);
        }
    }
}

/// One running game together with everything it owns.
///
/// The fields are declared in release order. Dropping the session first
/// destroys the core handle, which joins the core threads and frees the 3DS
/// memory, so no callback can run afterwards. Only then are the callback
/// context and the audio output released.
pub struct GameSession {
    handle: SessionHandle,
    _context: Box<CallbackContext>,
    audio: Option<Arc<AudioOutput>>,
    events: EventSink,
    captures: Arc<CaptureBroker>,
    state: SessionState,
}

impl GameSession {
    /// Creates the session and starts emulation and shader preparation.
    pub fn start(game_path: &str, options: &SessionOptions, events: EventSink) -> SessionResult<Self> {
        let audio = open_audio(options.audio_engine.effective(), &events);
        let captures = Arc::new(CaptureBroker::default());
        let context = CallbackContext::new(Arc::new(Listener {
            events: Arc::clone(&events),
            audio: audio.clone(),
            captures: Arc::clone(&captures),
        }));
        let handle = SessionHandle::create(game_path, options, &context)?;
        handle.start()?;

        let session = Self {
            handle,
            _context: context,
            audio,
            events,
            captures,
            state: SessionState::Running,
        };
        session.notify_state();
        Ok(session)
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn pause(&mut self) -> SessionResult<()> {
        if self.state != SessionState::Running {
            return Err(AzaharError::InvalidState);
        }
        self.handle.pause()?;
        if let Some(audio) = &self.audio {
            audio.pause();
        }
        self.state = SessionState::Paused;
        self.notify_state();
        Ok(())
    }

    pub fn resume(&mut self) -> SessionResult<()> {
        if self.state != SessionState::Paused {
            return Err(AzaharError::InvalidState);
        }
        self.handle.resume()?;
        if let Some(audio) = &self.audio {
            audio.resume();
        }
        self.state = SessionState::Running;
        self.notify_state();
        Ok(())
    }

    pub fn captures(&self) -> Arc<CaptureBroker> {
        Arc::clone(&self.captures)
    }

    pub fn request_screenshot(&self, request_id: u64, bottom: bool) -> SessionResult<()> {
        self.handle.request_screenshot(request_id, bottom)
    }

    pub fn request_capture_3d(&self, request_id: u64) -> SessionResult<()> {
        self.handle.request_capture_3d(request_id)
    }

    pub fn answer_artic_country(&self, country: Option<u32>) {
        self.handle.answer_artic_country(country);
    }

    pub fn set_volume(&self, volume: f32) {
        if let Some(audio) = &self.audio {
            audio.set_volume(volume);
        }
    }

    /// Read-only access to the 3DS memory of this session.
    pub fn memory(&self) -> Memory3ds<'_> {
        Memory3ds::new(&self.handle)
    }

    /// Stops the game and blocks until every resource has been released.
    pub fn stop(self) {
        drop(self);
    }

    fn notify_state(&self) {
        (self.events)(SessionEvent::StateChanged { state: self.state });
    }
}

/// Opens the audio output this crate plays for `engine`, reporting a failure as an event.
///
/// Returns `None` when the engine is played by the core or the output could not be opened.
#[cfg(target_os = "android")]
fn open_audio(engine: AudioEngine, events: &EventSink) -> Option<Arc<AudioOutput>> {
    if engine != AudioEngine::Oboe {
        return None;
    }
    match AudioOutput::new() {
        Ok(audio) => Some(Arc::new(audio)),
        Err(error) => {
            events(SessionEvent::Error {
                message: error.to_string(),
            });
            None
        }
    }
}

/// Opens the audio output this crate plays for `engine`. Every engine outside Android is played
/// by the core, so there is never one.
#[cfg(not(target_os = "android"))]
fn open_audio(_engine: AudioEngine, _events: &EventSink) -> Option<Arc<AudioOutput>> {
    None
}
