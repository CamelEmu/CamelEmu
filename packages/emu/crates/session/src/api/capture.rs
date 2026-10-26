use std::sync::mpsc::Receiver;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flutter_rust_bridge::frb;
use screenshot_3d::{Notification, Progress, STAGE_COUNT, Screenshot3dPipeline, Stage};

use crate::api::error::AzaharError;
use crate::capture::{CaptureBroker, CaptureSettings, CoreCapture, RawCapture, Screen, ScreenImage};
use crate::registry;

#[frb(unignore)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureKind {
    Screenshot,
    Screenshot3d,
}

#[frb(unignore)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureTarget {
    Both,
    Top,
    Bottom,
}

#[frb(unignore)]
#[derive(Debug, Clone)]
pub struct CaptureResult {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub mime_type: String,
}

#[frb(unignore)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureProgress {
    pub fraction: f64,
}

#[frb(non_opaque)]
#[derive(Debug, Clone)]
pub enum CaptureEvent {
    Progress(CaptureProgress),
    Finished(CaptureResult),
}

struct Forward<'a>(&'a (dyn Fn(CaptureProgress) + Sync));

impl Notification for Forward<'_> {
    fn notify(&self, stage: &dyn Stage, done: u64, total: u64) {
        let within = if total == 0 {
            1.0
        } else {
            done as f64 / total as f64
        };
        (self.0)(CaptureProgress {
            fraction: (f64::from(stage.order()) + within) / f64::from(STAGE_COUNT),
        });
    }
}

#[frb(ignore)]
pub fn capture_with(
    kind: CaptureKind,
    target: CaptureTarget,
    on_progress: &(dyn Fn(CaptureProgress) + Sync),
) -> Result<CaptureResult, AzaharError> {
    let broker = registry::with_session(|session| Ok(session.captures()))?;
    let settings = CaptureSettings::default();
    match kind {
        CaptureKind::Screenshot => screenshot(&broker, target, &settings),
        CaptureKind::Screenshot3d => screenshot_3d(&broker, target, &settings, on_progress),
    }
}

fn screenshot(
    broker: &CaptureBroker,
    target: CaptureTarget,
    settings: &CaptureSettings,
) -> Result<CaptureResult, AzaharError> {
    let images = target
        .screens()
        .into_iter()
        .map(|screen| take_screen(broker, screen, settings))
        .collect::<Result<Vec<_>, _>>()?;
    let image = images
        .into_iter()
        .reduce(|top, bottom| ScreenImage::from((top, bottom)))
        .ok_or_else(|| AzaharError::Capture("no screen was selected".into()))?;
    Ok(result(
        image.encode_png()?,
        settings.screenshot_prefix,
        settings.png_extension,
        settings.png_mime_type,
    ))
}

fn screenshot_3d(
    broker: &CaptureBroker,
    target: CaptureTarget,
    settings: &CaptureSettings,
    on_progress: &(dyn Fn(CaptureProgress) + Sync),
) -> Result<CaptureResult, AzaharError> {
    let (id, receiver) = broker.register();
    if let Err(error) = registry::with_session(|session| session.request_capture_3d(id)) {
        broker.cancel(id);
        return Err(error);
    }
    let received = receiver.recv().map_err(|_| {
        broker.cancel(id);
        AzaharError::Capture("the capture was cancelled".into())
    })?;
    let CoreCapture::Data(data) = received else {
        return Err(AzaharError::Capture("the core sent an unexpected result".into()));
    };
    let raw = RawCapture::parse(&data)
        .ok_or_else(|| AzaharError::Capture("the draw data is malformed".into()))?;
    let contexts = target
        .screens()
        .into_iter()
        .map(|screen| raw.context_for(screen))
        .collect();
    let forward = Forward(on_progress);
    let mut progress = Progress::new(&forward);
    let pipeline = Screenshot3dPipeline::build(contexts, &mut progress)
        .ok_or_else(|| AzaharError::Capture("no screen was selected".into()))?;
    let glb = pipeline
        .to_glb_with(&mut progress)
        .map_err(|error| AzaharError::Capture(error.to_string()))?;
    Ok(result(
        glb,
        settings.screenshot_3d_prefix,
        settings.glb_extension,
        settings.glb_mime_type,
    ))
}

fn take_screen(
    broker: &CaptureBroker,
    screen: &dyn Screen,
    settings: &CaptureSettings,
) -> Result<ScreenImage, AzaharError> {
    let (id, receiver) = broker.register();
    let bottom = screen.is_bottom();
    if let Err(error) = registry::with_session(|session| session.request_screenshot(id, bottom)) {
        broker.cancel(id);
        return Err(error);
    }
    match wait(broker, id, &receiver, settings.timeout)? {
        CoreCapture::Screenshot(Some(image)) => Ok(image),
        _ => Err(AzaharError::Capture("the screenshot failed".into())),
    }
}

fn wait(
    broker: &CaptureBroker,
    id: u64,
    receiver: &Receiver<CoreCapture>,
    timeout: Duration,
) -> Result<CoreCapture, AzaharError> {
    receiver.recv_timeout(timeout).map_err(|_| {
        broker.cancel(id);
        AzaharError::Capture("no frame arrived in time".into())
    })
}

fn result(bytes: Vec<u8>, prefix: &str, extension: &str, mime_type: &str) -> CaptureResult {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    CaptureResult {
        bytes,
        file_name: format!("{prefix}{seconds}.{extension}"),
        mime_type: mime_type.into(),
    }
}
