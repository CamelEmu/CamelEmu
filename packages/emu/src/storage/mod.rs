//! Every file the C++ core reads or writes goes through this module, so the core has no platform
//! specific storage code of its own.

mod abi;
#[cfg(target_os = "android")]
mod android_jni;
mod native;
mod overlay;
#[cfg(target_os = "android")]
mod saf;
mod scratch;

use std::ffi::CString;
use std::fs::File;
use std::io;
use std::os::fd::{FromRawFd, RawFd};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use overlay::Overlay;

use crate::error::{AzaharError, Result};

/// Folders the core expects in a user directory before it first runs, since it does not create
/// the folders of its settings and logs by itself.
const USER_DIRECTORIES: [&str; 6] = ["config", "nand", "sdmc", "sysdata", "cheats", "log"];

/// Folders of the user directory whose changes are held in memory during a session. They hold
/// the saves, the system data and the installed titles. Logs and caches are left out, since a
/// log is needed after a crash and a cache is not part of what the user saves or discards.
const OVERLAY_DIRECTORIES: [&str; 3] = ["nand", "sdmc", "sysdata"];

/// High halves of the title ids of updates and downloadable content, whose files always go
/// straight to the storage.
const DIRECT_TITLE_KINDS: [&str; 2] = ["0004000e", "0004008c"];

/// A file system the core can reach. The paths are the ones the core uses, so a backend decides
/// on its own how a path maps to its storage.
pub trait Storage: Send + Sync {
    fn exists(&self, path: &str) -> bool;
    fn is_directory(&self, path: &str) -> bool;
    fn size(&self, path: &str) -> u64;
    fn create_dir(&self, path: &str) -> bool;
    fn remove_file(&self, path: &str) -> bool;
    fn remove_dir(&self, path: &str) -> bool;
    fn rename(&self, from: &str, to: &str) -> bool;
    fn copy(&self, from: &str, to: &str) -> bool;
    fn open(&self, path: &str, mode: OpenMode) -> Option<RawFd>;
    fn list(&self, path: &str) -> Option<Vec<String>>;
}

/// An `fopen` mode, kept as flags because each backend opens files in its own way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpenMode {
    pub read: bool,
    pub write: bool,
    pub append: bool,
    pub truncate: bool,
}

impl OpenMode {
    pub fn parse(mode: &str) -> Option<Self> {
        let update = mode.contains('+');
        match mode.chars().next()? {
            'r' => Some(Self {
                read: true,
                write: update,
                append: false,
                truncate: false,
            }),
            'w' => Some(Self {
                read: update,
                write: true,
                append: false,
                truncate: true,
            }),
            'a' => Some(Self {
                read: update,
                write: true,
                append: true,
                truncate: false,
            }),
            _ => None,
        }
    }

    /// `fopen` creates the file for `w` and `a` but not for `r+`.
    pub fn creates(&self) -> bool {
        self.truncate || self.append
    }
}

/// Where the user directory is. A tree of the Storage Access Framework has no path, so the core
/// addresses it by paths relative to `/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Root {
    Native(PathBuf),
    Tree(String),
}

impl Root {
    pub fn parse(location: &str) -> Option<Self> {
        if let Some(path) = location.strip_prefix("file://") {
            return Some(Self::Native(PathBuf::from(percent_decode(path)?)));
        }
        if location.starts_with("content://") {
            return Some(Self::Tree(location.to_owned()));
        }
        if location.starts_with('/') {
            return Some(Self::Native(PathBuf::from(location)));
        }
        None
    }

    fn user_path(&self) -> String {
        match self {
            Self::Native(path) => {
                let mut path = path.to_string_lossy().into_owned();
                if !path.ends_with('/') {
                    path.push('/');
                }
                path
            }
            Self::Tree(_) => "/".to_owned(),
        }
    }
}

/// Declares, for each platform, the user directory used before one is chosen and the backend
/// that reaches a path. Keeping both in one table keeps the platform differences out of the
/// callers.
macro_rules! platform_storage {
    ($($os:literal => {
        default_root: $default:expr,
        backend: |$path:ident, $root:ident| $select:expr $(,)?
    })+) => {
        $(
            #[cfg(target_os = $os)]
            fn default_root() -> Root {
                $default
            }

            #[cfg(target_os = $os)]
            fn backend($path: &str, $root: &Root) -> &'static dyn Storage {
                $select
            }
        )+
    };
}

platform_storage! {
    "linux" => {
        default_root: Root::Native(native::default_user_directory()),
        backend: |_path, _root| &native::NativeStorage,
    }
    "android" => {
        default_root: Root::Tree(String::new()),
        backend: |path, root| {
            if path.starts_with("content://")
                || (matches!(root, Root::Tree(_)) && path.starts_with('/'))
            {
                &saf::SafStorage
            } else {
                &native::NativeStorage
            }
        },
    }
}

struct State {
    root: Root,
    user_path: CString,
}

impl State {
    fn new(root: Root) -> Self {
        let user_path = CString::new(root.user_path()).unwrap_or_default();
        Self { root, user_path }
    }
}

static STATE: RwLock<Option<State>> = RwLock::new(None);

fn with_state<R>(f: impl FnOnce(&State) -> R) -> R {
    {
        let state = STATE.read().unwrap_or_else(|error| error.into_inner());
        if let Some(state) = state.as_ref() {
            return f(state);
        }
    }
    let mut state = STATE.write().unwrap_or_else(|error| error.into_inner());
    f(state.get_or_insert_with(|| State::new(default_root())))
}

fn storage_for(path: &str) -> &'static dyn Storage {
    with_state(|state| backend(path, &state.root))
}

static OVERLAY: RwLock<Option<Arc<Overlay>>> = RwLock::new(None);

/// The storage that answers for one path: the overlay while a session holds the path in memory,
/// otherwise the backend of the path.
enum Route {
    Overlay(Arc<Overlay>),
    Direct,
    Backend(&'static dyn Storage),
}

impl Route {
    fn storage(&self) -> &dyn Storage {
        match self {
            Self::Overlay(overlay) => overlay.as_ref(),
            Self::Direct => &DIRECT,
            Self::Backend(backend) => *backend,
        }
    }
}

/// The part of `path` below its folder of [`OVERLAY_DIRECTORIES`], or `None` outside them. The
/// user directory is read on every call, because the core may set it after the session started.
fn overlay_directory_rest(path: &str) -> Option<String> {
    let path = overlay::normalize(path);
    with_state(|state| {
        let user_path = state.user_path.to_str().ok()?;
        OVERLAY_DIRECTORIES.iter().find_map(|name| {
            let folder = format!("{user_path}{name}");
            if path == folder {
                return Some(String::new());
            }
            path.strip_prefix(&format!("{folder}/")).map(str::to_owned)
        })
    })
}

fn is_direct_title(rest: &str) -> bool {
    let mut segments = rest.split('/');
    segments.any(|segment| segment == "title")
        && segments.next().is_some_and(|kind| {
            DIRECT_TITLE_KINDS
                .iter()
                .any(|direct| kind.eq_ignore_ascii_case(direct))
        })
}

fn is_held(path: &str) -> bool {
    overlay_directory_rest(path).is_some_and(|rest| !is_direct_title(&rest))
}

fn is_direct(path: &str) -> bool {
    overlay_directory_rest(path).is_some_and(|rest| is_direct_title(&rest))
}

/// The storage of the paths inside an overlay folder that are not held in memory. The folders
/// above a path are created first, since the overlay may hold them without the storage having them.
struct DirectStorage;

impl DirectStorage {
    fn create_parents(path: &str) {
        let path = overlay::normalize(path);
        let user_length = with_state(|state| state.user_path.as_bytes().len());
        let mut missing = Vec::new();
        let mut current = overlay::parent(&path);
        while current.len() > user_length && !storage_for(current).is_directory(current) {
            missing.push(current);
            current = overlay::parent(current);
        }
        for folder in missing.into_iter().rev() {
            storage_for(folder).create_dir(folder);
        }
    }
}

impl Storage for DirectStorage {
    fn exists(&self, path: &str) -> bool {
        storage_for(path).exists(path)
    }

    fn is_directory(&self, path: &str) -> bool {
        storage_for(path).is_directory(path)
    }

    fn size(&self, path: &str) -> u64 {
        storage_for(path).size(path)
    }

    fn create_dir(&self, path: &str) -> bool {
        Self::create_parents(path);
        storage_for(path).create_dir(path)
    }

    fn remove_file(&self, path: &str) -> bool {
        storage_for(path).remove_file(path)
    }

    fn remove_dir(&self, path: &str) -> bool {
        storage_for(path).remove_dir(path)
    }

    fn rename(&self, from: &str, to: &str) -> bool {
        Self::create_parents(to);
        storage_for(from).rename(from, to)
    }

    fn copy(&self, from: &str, to: &str) -> bool {
        Self::create_parents(to);
        storage_for(from).copy(from, to)
    }

    fn open(&self, path: &str, mode: OpenMode) -> Option<RawFd> {
        if mode.creates() {
            Self::create_parents(path);
        }
        storage_for(path).open(path, mode)
    }

    fn list(&self, path: &str) -> Option<Vec<String>> {
        storage_for(path).list(path)
    }
}

static DIRECT: DirectStorage = DirectStorage;

fn route(path: &str) -> Route {
    route_any(&[path])
}

/// Routes an operation on several paths to the overlay when any of them is held in memory, so
/// that a file moved into or out of an overlay folder is still tracked.
fn route_any(paths: &[&str]) -> Route {
    let overlay = OVERLAY
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let Some(overlay) = overlay else {
        return Route::Backend(storage_for(paths[0]));
    };
    if paths.iter().any(|path| is_held(path)) {
        Route::Overlay(overlay)
    } else if paths.iter().any(|path| is_direct(path)) {
        Route::Direct
    } else {
        Route::Backend(storage_for(paths[0]))
    }
}

/// Moves or copies a file. Between the overlay and a path not held in memory it is copied through
/// both storages.
pub fn relocate(from: &str, to: &str, keep_source: bool) -> bool {
    let (source, target) = (route(from), route(to));
    let crosses = matches!(
        (&source, &target),
        (Route::Overlay(_), Route::Direct) | (Route::Direct, Route::Overlay(_))
    );
    if !crosses {
        let route = route_any(&[from, to]);
        return if keep_source {
            route.storage().copy(from, to)
        } else {
            route.storage().rename(from, to)
        };
    }
    let (source, target) = (source.storage(), target.storage());
    if source.is_directory(from) {
        return false;
    }
    let (Some(read), Some(write)) = (
        source.open(from, overlay::READ),
        target.open(to, overlay::WRITE),
    ) else {
        return false;
    };
    let mut read = unsafe { File::from_raw_fd(read) };
    let mut write = unsafe { File::from_raw_fd(write) };
    io::copy(&mut read, &mut write).is_ok() && (keep_source || source.remove_file(from))
}

/// Starts holding the changes to the saves, the system data and the installed titles in memory.
pub fn begin_overlay() {
    *OVERLAY.write().unwrap_or_else(|error| error.into_inner()) =
        Some(Arc::new(Overlay::default()));
}

/// Writes the changes held in memory to the storage. Fails with the paths that could not be
/// written; the other changes are written anyway.
pub fn commit_overlay() -> Result<()> {
    let overlay = OVERLAY
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let Some(overlay) = overlay else {
        return Ok(());
    };
    let failed = overlay.commit();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(AzaharError::StorageCommit(failed))
    }
}

/// Stops holding changes in memory and drops the ones not written, so that every later access
/// reaches the storage again.
pub fn end_overlay() {
    *OVERLAY.write().unwrap_or_else(|error| error.into_inner()) = None;
}

fn set_root(location: &str) -> Result<()> {
    let root = Root::parse(location).ok_or(AzaharError::InvalidPath)?;
    *STATE.write().unwrap_or_else(|error| error.into_inner()) = Some(State::new(root));
    Ok(())
}

/// Creates the folders the core expects in the user directory at `location`, which does not have
/// to be the current one, since a profile is prepared before it is switched to.
pub fn initialize(location: &str) -> Result<()> {
    let root = Root::parse(location).ok_or(AzaharError::InvalidPath)?;
    let base = match &root {
        Root::Native(_) => root.user_path(),
        Root::Tree(uri) => format!("{}/", uri.trim_end_matches('/')),
    };
    if let Root::Native(path) = &root {
        std::fs::create_dir_all(path)
            .map_err(|error| AzaharError::Storage(format!("{}: {error}", path.display())))?;
    }
    for name in USER_DIRECTORIES {
        let path = format!("{base}{name}");
        if !backend(&path, &root).create_dir(&path) {
            return Err(AzaharError::Storage(path));
        }
    }
    Ok(())
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}
