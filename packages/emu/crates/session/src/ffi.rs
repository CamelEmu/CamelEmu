//! The only module that calls into the C++ core.
//!
//! Every call runs on one dedicated thread that belongs to the session. The thread loads the
//! core, creates the session, runs every operation and, when the session is dropped, destroys it
//! and unloads the core before it ends. Nothing that belongs to the core outlives that thread.

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::Arc;
use std::sync::mpsc::{Sender, SyncSender, channel, sync_channel};
use std::thread::JoinHandle;

use memory::{MemoryError, MemorySource, Result as MemoryResult};

use crate::error::{AzaharError, SessionResult};
use crate::library::{CoreLibrary, close};
use crate::api::audio::AudioEngine;
use crate::raw::{RawCallbacks, RawOptions, RawSession, STATUS_INVALID_ADDRESS, STATUS_OK};
use crate::session::{SessionEvent, SessionOptions, ShaderStage};

/// Receiver of everything the core reports from its own threads.
pub trait CoreListener: Send + Sync {
    fn on_event(&self, event: SessionEvent);
    fn on_audio(&self, frames: &[i16]);
    fn on_screenshot(&self, request_id: u64, screenshot: Option<RawScreenshot<'_>>);
    fn on_capture_3d(&self, request_id: u64, data: &[u8]);
}

pub struct RawScreenshot<'a> {
    pub secondary: bool,
    pub width: u32,
    pub height: u32,
    pub bgra: &'a [u8],
}

/// Heap allocated state the core keeps a raw pointer to.
///
/// It must outlive the [`SessionHandle`] that was created with it.
pub struct CallbackContext {
    listener: Arc<dyn CoreListener>,
}

impl CallbackContext {
    pub fn new(listener: Arc<dyn CoreListener>) -> Box<Self> {
        Box::new(Self { listener })
    }
}

unsafe fn context<'a>(user: *mut c_void) -> &'a CallbackContext {
    unsafe { &*(user as *const CallbackContext) }
}

unsafe extern "C" fn trampoline_shader_progress(
    user: *mut c_void,
    stage: i32,
    progress: u64,
    max: u64,
) {
    let stage = match stage {
        0 => ShaderStage::Prepare,
        1 => ShaderStage::Decompile,
        2 => ShaderStage::Build,
        _ => ShaderStage::Complete,
    };
    unsafe { context(user) }
        .listener
        .on_event(SessionEvent::ShaderProgress {
            stage,
            progress,
            max,
        });
}

unsafe extern "C" fn trampoline_texture(user: *mut c_void, texture_id: i64, secondary: i32) {
    unsafe { context(user) }
        .listener
        .on_event(SessionEvent::Texture {
            texture_id,
            secondary: secondary != 0,
        });
}

unsafe extern "C" fn trampoline_error(user: *mut c_void, message: *const c_char) {
    let message = if message.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    };
    unsafe { context(user) }
        .listener
        .on_event(SessionEvent::Failed { message });
}

unsafe extern "C" fn trampoline_audio(user: *mut c_void, frames: *const i16, frame_count: usize) {
    if frames.is_null() || frame_count == 0 {
        return;
    }
    let samples = unsafe { std::slice::from_raw_parts(frames, frame_count * 2) };
    unsafe { context(user) }.listener.on_audio(samples);
}

unsafe extern "C" fn trampoline_shutdown_requested(user: *mut c_void) {
    unsafe { context(user) }
        .listener
        .on_event(SessionEvent::ShutdownRequested);
}

unsafe extern "C" fn trampoline_artic_country_mismatch(
    user: *mut c_void,
    region: u32,
    country: u32,
) {
    unsafe { context(user) }
        .listener
        .on_event(SessionEvent::ArticCountryMismatch { region, country });
}

unsafe extern "C" fn trampoline_screenshot(
    user: *mut c_void,
    request_id: u64,
    secondary: i32,
    width: i32,
    height: i32,
    pixels: *const u8,
) {
    let screenshot = match (u32::try_from(width), u32::try_from(height)) {
        (Ok(width), Ok(height)) if !pixels.is_null() => {
            let len = width as usize * height as usize * 4;
            Some(RawScreenshot {
                secondary: secondary != 0,
                width,
                height,
                bgra: unsafe { std::slice::from_raw_parts(pixels, len) },
            })
        }
        _ => None,
    };
    unsafe { context(user) }
        .listener
        .on_screenshot(request_id, screenshot);
}

unsafe extern "C" fn trampoline_capture_3d(
    user: *mut c_void,
    request_id: u64,
    data: *const u8,
    length: usize,
) {
    let data = if data.is_null() {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(data, length) }
    };
    unsafe { context(user) }
        .listener
        .on_capture_3d(request_id, data);
}

type Job = Box<dyn FnOnce(&CoreLibrary) + Send>;

/// The thread that owns the core library and runs every call into it.
///
/// The thread ends with the library detached, and the library is unloaded after the thread was
/// joined.
struct Worker {
    sender: Option<Sender<Job>>,
    thread: Option<JoinHandle<usize>>,
}

impl Worker {
    fn spawn() -> SessionResult<Self> {
        let (sender, receiver) = channel::<Job>();
        let (ready_sender, ready) = sync_channel::<SessionResult<()>>(1);
        let thread = std::thread::Builder::new()
            .name("azahar-core".into())
            .spawn(move || {
                let library = match CoreLibrary::open() {
                    Ok(library) => {
                        let _ = ready_sender.send(Ok(()));
                        library
                    }
                    Err(failure) => {
                        let _ = ready_sender.send(Err(failure.error));
                        return failure.handle;
                    }
                };
                while let Ok(job) = receiver.recv() {
                    job(&library);
                }
                library.handle()
            })
            .map_err(|error| AzaharError::LibraryLoad(error.to_string()))?;

        match ready.recv() {
            Ok(Ok(())) => Ok(Self {
                sender: Some(sender),
                thread: Some(thread),
            }),
            Ok(Err(error)) => {
                let handle = thread.join().unwrap_or(0);
                close(handle);
                Err(error)
            }
            Err(_) => {
                let handle = thread.join().unwrap_or(0);
                close(handle);
                Err(AzaharError::LibraryLoad(
                    "the core thread ended early".into(),
                ))
            }
        }
    }

    /// Runs `job` on the core thread and returns what it returned.
    fn call<R: Send + 'static>(&self, job: impl FnOnce(&CoreLibrary) -> R + Send + 'static) -> R {
        let (result_sender, result): (SyncSender<R>, _) = sync_channel(1);
        let sender = self.sender.as_ref().expect("the core thread is running");
        sender
            .send(Box::new(move |library| {
                let _ = result_sender.send(job(library));
            }))
            .expect("the core thread is running");
        result.recv().expect("the core thread answers")
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let handle = thread.join().unwrap_or(0);
            close(handle);
        }
    }
}

/// Owning handle to one core session.
///
/// Dropping it destroys the session, which joins every core thread and frees the emulated 3DS
/// memory, and then unloads the core before returning.
pub struct SessionHandle {
    worker: Worker,
    raw: usize,
}

impl SessionHandle {
    /// Loads the core and creates a session that reports through `context`.
    ///
    /// The caller keeps `context` alive for at least as long as the handle.
    pub fn create(
        game_path: &str,
        options: &SessionOptions,
        context: &CallbackContext,
    ) -> SessionResult<Self> {
        let path = CString::new(game_path).map_err(|_| AzaharError::InvalidPath)?;
        let raw_options = RawOptions {
            primary_width: options.primary_width,
            primary_height: options.primary_height,
            secondary_width: options.secondary_width,
            secondary_height: options.secondary_height,
            dual_screen: options.dual_screen as i32,
        };
        let plays_audio = options.audio_engine.effective() == AudioEngine::Oboe;
        let user = context as *const CallbackContext as usize;

        let worker = Worker::spawn()?;
        let raw = worker.call(move |library| {
            let callbacks = RawCallbacks {
                user: user as *mut c_void,
                on_shader_progress: Some(trampoline_shader_progress),
                on_texture: Some(trampoline_texture),
                on_error: Some(trampoline_error),
                on_audio: plays_audio.then_some(trampoline_audio as _),
                on_shutdown_requested: Some(trampoline_shutdown_requested),
                on_artic_country_mismatch: Some(trampoline_artic_country_mismatch),
                on_screenshot: Some(trampoline_screenshot),
                on_capture_3d: Some(trampoline_capture_3d),
            };
            unsafe { (library.api.create)(path.as_ptr(), &raw_options, &callbacks) as usize }
        });
        if raw == 0 {
            return Err(AzaharError::CreateFailed);
        }
        Ok(Self { worker, raw })
    }

    pub fn start(&self) -> SessionResult<()> {
        let raw = self.raw;
        check(
            self.worker
                .call(move |library| unsafe { (library.api.start)(raw as *mut RawSession) }),
        )
    }

    pub fn answer_artic_country(&self, country: Option<u32>) {
        let raw = self.raw;
        let value = country.map_or(-1, |country| country as i32);
        self.worker.call(move |library| unsafe {
            (library.api.answer_artic_country)(raw as *mut RawSession, value)
        });
    }

    pub fn request_screenshot(&self, request_id: u64, secondary: bool) -> SessionResult<()> {
        let raw = self.raw;
        check(self.worker.call(move |library| unsafe {
            (library.api.request_screenshot)(raw as *mut RawSession, request_id, secondary as i32)
        }))
    }

    pub fn request_capture_3d(&self, request_id: u64) -> SessionResult<()> {
        let raw = self.raw;
        check(self.worker.call(move |library| unsafe {
            (library.api.request_capture_3d)(raw as *mut RawSession, request_id)
        }))
    }

    pub fn pause(&self) -> SessionResult<()> {
        let raw = self.raw;
        check(
            self.worker
                .call(move |library| unsafe { (library.api.pause)(raw as *mut RawSession) }),
        )
    }

    pub fn resume(&self) -> SessionResult<()> {
        let raw = self.raw;
        check(
            self.worker
                .call(move |library| unsafe { (library.api.resume)(raw as *mut RawSession) }),
        )
    }

}

impl MemorySource for SessionHandle {
    fn fcram_size(&self) -> usize {
        let raw = self.raw;
        self.worker
            .call(move |library| unsafe { (library.api.fcram_size)(raw as *const RawSession) })
    }

    fn read_memory(&self, address: u32, len: usize) -> MemoryResult<Vec<u8>> {
        let raw = self.raw;
        self.worker.call(move |library| {
            let mut buffer = vec![0u8; len];
            let status = unsafe {
                (library.api.read_memory)(
                    raw as *mut RawSession,
                    address,
                    buffer.as_mut_ptr(),
                    len,
                )
            };
            if status == STATUS_INVALID_ADDRESS {
                return Err(MemoryError::InvalidAddress {
                    address,
                    len: len as u32,
                });
            }
            memory_status(status).map(|()| buffer)
        })
    }

    fn read_fcram(&self, offset: usize, len: usize) -> MemoryResult<Vec<u8>> {
        let raw = self.raw;
        self.worker.call(move |library| {
            let mut buffer = vec![0u8; len];
            let status = unsafe {
                (library.api.read_fcram)(raw as *mut RawSession, offset, buffer.as_mut_ptr(), len)
            };
            memory_status(status).map(|()| buffer)
        })
    }
}

impl Drop for SessionHandle {
    fn drop(&mut self) {
        let raw = self.raw;
        self.worker.call(move |library| unsafe {
            (library.api.destroy)(raw as *mut RawSession);
        });
    }
}

fn check(status: i32) -> SessionResult<()> {
    if status == STATUS_OK {
        Ok(())
    } else {
        Err(AzaharError::Core(status))
    }
}

fn memory_status(status: i32) -> MemoryResult<()> {
    if status == STATUS_OK {
        Ok(())
    } else {
        Err(MemoryError::Core(status))
    }
}
