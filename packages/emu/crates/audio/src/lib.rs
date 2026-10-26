#![deny(unsafe_code)]

mod aa_filter;
mod error;
mod fifo;
mod fir;
mod rate_transposer;
mod sound_touch;
mod td_stretch;
mod time_stretch;

#[allow(unsafe_code)]
mod abi;

pub use error::{AudioError, Result};
pub use sound_touch::SoundTouch;
pub use time_stretch::{TimeStretchSettings, TimeStretcher};
