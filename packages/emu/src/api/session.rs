use std::sync::Arc;

use session::api::session::{SessionEvent, SessionOptions};
use session::error::AzaharError;

use crate::frb_generated::StreamSink;

/// Starts a game and streams its events (shader progress, textures, state
/// changes, errors) to Dart.
///
/// It lives here because `StreamSink` is generated into the crate that holds the bindings.
pub fn start_game(
    game_path: String,
    options: SessionOptions,
    events: StreamSink<SessionEvent>,
) -> Result<(), AzaharError> {
    session::registry::start(
        &game_path,
        &options,
        Arc::new(move |event| {
            let _ = events.add(event);
        }),
    )
}
