use session::api::capture::{CaptureEvent, CaptureKind, CaptureTarget, capture_with};
use session::api::error::AzaharError;

use crate::frb_generated::StreamSink;

pub fn capture(
    kind: CaptureKind,
    target: CaptureTarget,
    events: StreamSink<CaptureEvent>,
) -> Result<(), AzaharError> {
    let result = capture_with(kind, target, &|progress| {
        let _ = events.add(CaptureEvent::Progress(progress));
    })?;
    let _ = events.add(CaptureEvent::Finished(result));
    Ok(())
}
