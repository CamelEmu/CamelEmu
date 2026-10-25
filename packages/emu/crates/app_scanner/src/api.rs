use crate::platform::CurrentPlatform;
use crate::scan;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Game {
    pub title: String,
    pub description: String,
    pub path: String,
    pub title_id: i64,
    pub company: String,
    pub regions: String,
    pub is_installed: bool,
    pub is_system_title: bool,
    pub is_visible_system_title: bool,
    pub filename: String,
    pub icon_path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanRequest {
    pub games_directory: Option<String>,
    pub icons_directory: String,
}

pub fn scan_games(request: ScanRequest) -> Vec<Game> {
    scan::scan_games_for::<CurrentPlatform>(request)
}
