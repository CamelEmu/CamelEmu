use std::fs;
use std::io::Read;

use emu_storage::{OpenMode, create_dir_all, list_entries, open_file, write_file};

const READ: OpenMode = OpenMode {
    read: true,
    write: false,
    append: false,
    truncate: false,
};

#[test]
fn entries_carry_their_name_path_and_kind() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("folder")).unwrap();
    fs::write(dir.path().join("file.txt"), b"x").unwrap();

    let mut entries = list_entries(dir.path().to_str().unwrap()).unwrap();
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "file.txt");
    assert!(!entries[0].is_directory);
    assert_eq!(entries[0].path, dir.path().join("file.txt").to_str().unwrap());
    assert_eq!(entries[1].name, "folder");
    assert!(entries[1].is_directory);
}

#[test]
fn a_missing_folder_has_no_entries() {
    let dir = tempfile::tempdir().unwrap();

    assert!(list_entries(dir.path().join("missing").to_str().unwrap()).is_none());
}

#[test]
fn folders_are_created_with_their_parents() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("a/b/c");

    assert!(create_dir_all(nested.to_str().unwrap()));
    assert!(create_dir_all(nested.to_str().unwrap()));
    assert!(nested.is_dir());
}

#[test]
fn paths_with_the_file_scheme_reach_the_file_system() {
    let dir = tempfile::tempdir().unwrap();
    let nested = format!("file://{}/x/y", dir.path().display());

    assert!(create_dir_all(&nested));
    assert!(write_file(&format!("{nested}/data.bin"), b"payload"));

    let mut contents = Vec::new();
    open_file(&format!("{nested}/data.bin"), READ)
        .unwrap()
        .read_to_end(&mut contents)
        .unwrap();
    assert_eq!(contents, b"payload");
    assert_eq!(fs::read(dir.path().join("x/y/data.bin")).unwrap(), b"payload");
}

#[test]
fn writing_replaces_the_previous_contents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("file.bin");
    let path = path.to_str().unwrap();

    assert!(write_file(path, b"a long first version"));
    assert!(write_file(path, b"short"));

    assert_eq!(fs::read(path).unwrap(), b"short");
}
