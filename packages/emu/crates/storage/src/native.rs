use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::os::fd::{IntoRawFd, RawFd};
use std::path::{Path, PathBuf};

use super::{DirEntry, FILE_SCHEME, OpenMode, Storage};

/// Reaches paths of the file system directly.
pub struct NativeStorage;

fn plain(path: &str) -> &str {
    path.strip_prefix(FILE_SCHEME).unwrap_or(path)
}

impl Storage for NativeStorage {
    fn exists(&self, path: &str) -> bool {
        Path::new(plain(path)).exists()
    }

    fn is_directory(&self, path: &str) -> bool {
        Path::new(plain(path)).is_dir()
    }

    fn size(&self, path: &str) -> u64 {
        fs::metadata(plain(path))
            .ok()
            .filter(|metadata| metadata.is_file())
            .map_or(0, |metadata| metadata.len())
    }

    fn create_dir(&self, path: &str) -> bool {
        match fs::create_dir(plain(path)) {
            Ok(()) => true,
            Err(error) => error.kind() == ErrorKind::AlreadyExists && self.is_directory(path),
        }
    }

    fn remove_file(&self, path: &str) -> bool {
        fs::remove_file(plain(path)).is_ok()
    }

    fn remove_dir(&self, path: &str) -> bool {
        fs::remove_dir(plain(path)).is_ok()
    }

    fn rename(&self, from: &str, to: &str) -> bool {
        fs::rename(plain(from), plain(to)).is_ok()
    }

    fn copy(&self, from: &str, to: &str) -> bool {
        fs::copy(plain(from), plain(to)).is_ok()
    }

    fn open(&self, path: &str, mode: OpenMode) -> Option<RawFd> {
        OpenOptions::new()
            .read(mode.read)
            .write(mode.write && !mode.append)
            .append(mode.append)
            .truncate(mode.truncate)
            .create(mode.creates())
            .open(plain(path))
            .ok()
            .map(IntoRawFd::into_raw_fd)
    }

    fn list(&self, path: &str) -> Option<Vec<String>> {
        let entries = fs::read_dir(plain(path)).ok()?;
        Some(
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect(),
        )
    }

    fn list_entries(&self, path: &str) -> Option<Vec<DirEntry>> {
        let entries = fs::read_dir(plain(path)).ok()?;
        Some(
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| {
                    let entry_path = entry.path();
                    DirEntry {
                        name: entry.file_name().to_string_lossy().into_owned(),
                        is_directory: entry_path.is_dir(),
                        path: entry_path.to_string_lossy().into_owned(),
                    }
                })
                .collect(),
        )
    }
}

/// Follows the XDG base directory specification like the other Linux applications do.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn default_user_directory() -> PathBuf {
    let data_home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    data_home.join("azahar")
}
