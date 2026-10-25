pub mod api;
pub mod format;
pub mod ncch_format;
pub mod platform;
pub mod smdh;
pub mod threedsx;
pub mod tmd;
pub mod unique;

mod icon;
mod loader;
mod regions;
mod scan;
mod source;
mod title;

pub use api::{Game, ScanRequest, scan_games};
pub use scan::scan_games_for;
