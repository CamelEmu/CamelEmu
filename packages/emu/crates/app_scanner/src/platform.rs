use crate::loader::Status;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    GamesDirectory,
    Installed,
}

pub trait ScanPlatform {
    const ACCEPTS_ARCHIVES: bool;
    const INSTALLED_DEPTH: u32;
    const USES_UPDATE_SMDH: bool;
    const SANITIZES_TITLE: bool;
    const DESCRIBES_WITH_PATH: bool;
    const ENCRYPTED_REGIONS: &'static str;

    fn icons_directory(requested: &str) -> String;

    fn keeps(origin: Origin, loader_found: bool, status: Status, executable: bool) -> bool;
}

pub struct LinuxPlatform;

impl ScanPlatform for LinuxPlatform {
    const ACCEPTS_ARCHIVES: bool = false;
    const INSTALLED_DEPTH: u32 = 5;
    const USES_UPDATE_SMDH: bool = false;
    const SANITIZES_TITLE: bool = false;
    const DESCRIBES_WITH_PATH: bool = false;
    const ENCRYPTED_REGIONS: &'static str = "Encrypted";

    fn icons_directory(_requested: &str) -> String {
        format!("{}icons", emu_storage::user_path())
    }

    fn keeps(_origin: Origin, loader_found: bool, status: Status, executable: bool) -> bool {
        loader_found && (executable || status == Status::ErrorEncrypted)
    }
}

pub struct AndroidPlatform;

impl ScanPlatform for AndroidPlatform {
    const ACCEPTS_ARCHIVES: bool = true;
    const INSTALLED_DEPTH: u32 = u32::MAX;
    const USES_UPDATE_SMDH: bool = true;
    const SANITIZES_TITLE: bool = true;
    const DESCRIBES_WITH_PATH: bool = true;
    const ENCRYPTED_REGIONS: &'static str = "Unsupported encrypted application";

    fn icons_directory(requested: &str) -> String {
        requested.to_owned()
    }

    fn keeps(origin: Origin, loader_found: bool, status: Status, executable: bool) -> bool {
        match origin {
            Origin::GamesDirectory => true,
            Origin::Installed => loader_found && status == Status::Success && executable,
        }
    }
}

#[cfg(target_os = "android")]
pub type CurrentPlatform = AndroidPlatform;

#[cfg(not(target_os = "android"))]
pub type CurrentPlatform = LinuxPlatform;
