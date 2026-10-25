use std::sync::Mutex;

use emu_storage::DirEntry;

use crate::api::{Game, ScanRequest};
use crate::format::BinaryFormat;
use crate::icon;
use crate::loader::{AppLoader, Status, get_loader};
use crate::platform::{Origin, ScanPlatform};
use crate::regions;
use crate::smdh::{ENGLISH, Smdh};
use crate::title;
use crate::unique;

const GAMES_DIRECTORY_DEPTH: u32 = 3;

const EXTENSIONS: [&str; 6] = ["3dsx", "elf", "axf", "cci", "cxi", "app"];
const BAD_EXTENSIONS: [&str; 6] = ["rar", "zip", "7z", "torrent", "tar", "gz"];

const SYSTEM_TITLE_HIGH: u32 = 0x0004_0010;
const APPLICATION_TITLES: std::ops::RangeInclusive<u64> =
    0x0004_0000_0000_0000..=0x0004_0000_FFFF_FFFF;

static SCAN: Mutex<()> = Mutex::new(());

pub fn scan_games_for<P: ScanPlatform>(request: ScanRequest) -> Vec<Game> {
    let _guard = SCAN.lock().unwrap_or_else(|error| error.into_inner());
    let icons_directory = P::icons_directory(&request.icons_directory);
    unique::reset();
    let games = scan::<P>(
        request.games_directory.as_deref(),
        icons_directory.trim_end_matches('/'),
    );
    unique::reset();
    games
}

fn scan<P: ScanPlatform>(games_directory: Option<&str>, icons_directory: &str) -> Vec<Game> {
    let mut games = Vec::new();
    if let Some(directory) = games_directory.filter(|directory| !directory.is_empty()) {
        scan_directory::<P>(
            directory,
            GAMES_DIRECTORY_DEPTH,
            Origin::GamesDirectory,
            icons_directory,
            &mut games,
        );
    }
    for root in [
        format!("{}00040000", title::sdmc_title_root()),
        format!("{}00040010", title::nand_title_root()),
    ] {
        scan_directory::<P>(
            &root,
            P::INSTALLED_DEPTH,
            Origin::Installed,
            icons_directory,
            &mut games,
        );
    }
    games
}

fn has_scannable_extension<P: ScanPlatform>(name: &str) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    let extension = extension.to_lowercase();
    EXTENSIONS.contains(&extension.as_str())
        || (P::ACCEPTS_ARCHIVES && BAD_EXTENSIONS.contains(&extension.as_str()))
}

fn scan_directory<P: ScanPlatform>(
    directory: &str,
    depth: u32,
    origin: Origin,
    icons_directory: &str,
    games: &mut Vec<Game>,
) {
    if depth == 0 {
        return;
    }
    let Some(entries) = emu_storage::list_entries(directory) else {
        return;
    };
    for entry in entries {
        if entry.is_directory {
            scan_directory::<P>(&entry.path, depth - 1, origin, icons_directory, games);
        } else if has_scannable_extension::<P>(&entry.name) {
            games.extend(build_game::<P>(&entry, origin, icons_directory));
        }
    }
}

fn read_smdh<P: ScanPlatform>(loader: &mut dyn AppLoader, program_id: u64) -> (Vec<u8>, bool) {
    let (status, original) = loader.read_icon();
    if status == Status::ErrorEncrypted {
        return (original, true);
    }
    if !P::USES_UPDATE_SMDH || !APPLICATION_TITLES.contains(&program_id) {
        return (original, false);
    }
    let Some(update_path) = title::content_path(program_id + title::UPDATE_MASK) else {
        return (original, false);
    };
    if !emu_storage::exists(&update_path) {
        return (original, false);
    }
    match get_loader(&update_path) {
        Some(mut update) => (update.read_icon().1, false),
        None => (original, false),
    }
}

fn single_line(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_break = false;
    for character in text.chars() {
        if matches!(character, '\t' | '\n' | '\r') {
            if !in_break {
                result.push(' ');
            }
            in_break = true;
        } else {
            result.push(character);
            in_break = false;
        }
    }
    result
}

fn display_title<P: ScanPlatform>(text: &str) -> String {
    if P::SANITIZES_TITLE {
        single_line(text)
    } else {
        text.to_owned()
    }
}

fn build_game<P: ScanPlatform>(
    entry: &DirEntry,
    origin: Origin,
    icons_directory: &str,
) -> Option<Game> {
    let mut loader = get_loader(&entry.path);
    let (executable_status, executable) = match loader.as_mut() {
        Some(loader) => loader.is_executable(),
        None => (Status::Error, false),
    };
    if !P::keeps(origin, loader.is_some(), executable_status, executable) {
        return None;
    }

    let program_id = loader
        .as_mut()
        .map_or(0, |loader| loader.read_program_id().1);
    let (smdh_bytes, icon_encrypted) = match loader.as_mut() {
        Some(loader) => read_smdh::<P>(loader.as_mut(), program_id),
        None => (Vec::new(), false),
    };
    let encrypted = icon_encrypted || executable_status == Status::ErrorEncrypted;

    let mut game = Game {
        title: display_title::<P>(&entry.name),
        description: if P::DESCRIBES_WITH_PATH {
            entry.path.replace('\n', " ")
        } else {
            String::new()
        },
        path: entry.path.clone(),
        title_id: program_id as i64,
        company: String::new(),
        regions: if encrypted {
            P::ENCRYPTED_REGIONS.to_owned()
        } else {
            regions::INVALID_REGION.to_owned()
        },
        is_installed: origin == Origin::Installed,
        is_system_title: (program_id >> 32) as u32 == SYSTEM_TITLE_HIGH,
        is_visible_system_title: false,
        filename: entry.name.clone(),
        icon_path: None,
    };

    if Smdh::is_valid(&smdh_bytes)
        && let Ok(smdh) = Smdh::decode(&smdh_bytes)
    {
        let english = smdh.title(ENGLISH);
        game.title = display_title::<P>(&english.long);
        game.company = english.publisher.clone();
        game.regions = regions::regions_string(smdh.region_lockout);
        game.is_visible_system_title = smdh.is_visible();
        game.icon_path = icon::write_icon(icons_directory, &entry.path, &smdh.large_icon);
    }
    Some(game)
}
