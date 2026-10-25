mod common;

use common::*;
use emu_app_scanner::format::BinaryFormat;
use emu_app_scanner::platform::AndroidPlatform;

const APP_TITLE: u64 = 0x0004_0000_0012_3400;
const UPDATE_TITLE: u64 = 0x0004_000e_0012_3400;

#[test]
fn archives_are_listed_so_that_the_list_can_warn_about_them() {
    let env = Env::new();
    env.game_file("pack.zip", b"PK");
    env.game_file("pack.7z", b"7z");
    env.game_file("notes.txt", b"ignored");
    env.game_file("cover.png", b"ignored");

    let games = env.scan::<AndroidPlatform>();

    let mut names: Vec<_> = games.iter().map(|game| game.filename.as_str()).collect();
    names.sort();
    assert_eq!(names, ["pack.7z", "pack.zip"]);
    assert_eq!(games[0].regions, "Invalid region");
    assert_eq!(games[0].title_id, 0);
}

#[test]
fn files_the_loader_cannot_read_stay_in_the_list() {
    let env = Env::new();
    env.game_file("broken.cxi", b"not a game");
    env.game_file("empty.cci", b"");

    let games = env.scan::<AndroidPlatform>();

    for name in ["broken.cxi", "empty.cci"] {
        let game = env.find(&games, name);
        assert_eq!(game.title, name);
        assert_eq!(game.regions, "Invalid region");
        assert_eq!(game.title_id, 0);
    }
}

#[test]
fn files_that_are_not_executable_stay_in_the_list() {
    let env = Env::new();
    let mut manual = NcchSpec::new(APP_TITLE, Some(mock_smdh("Manual", "Maker", 1, 0, 1)));
    manual.is_executable = false;
    env.game_file("manual.cxi", &ncch(&manual));

    assert_eq!(env.scan::<AndroidPlatform>()[0].title, "Manual");
}

#[test]
fn the_description_is_the_path_and_titles_are_kept_on_one_line() {
    let env = Env::new();
    let smdh = mock_smdh("Line one\r\nLine\ttwo", "Maker", 1, 0, 1);
    let path = env.game_file("lines.3dsx", &threedsx(Some(&smdh)));

    let game = env.scan::<AndroidPlatform>().remove(0);

    assert_eq!(game.description, path.to_str().unwrap());
    assert_eq!(game.title, "Line one Line two");
}

#[test]
fn installed_encrypted_applications_are_not_listed() {
    let env = Env::new();
    let mut spec = NcchSpec::new(APP_TITLE, Some(mock_smdh("Locked", "Maker", 1, 0, 1)));
    spec.no_crypto = false;
    env.write(
        env.sdmc_content("00040000", "00123400").join("00000000.app"),
        &ncch(&spec),
    );

    assert!(env.scan::<AndroidPlatform>().is_empty());
}

#[test]
fn installed_applications_are_searched_at_any_depth() {
    let env = Env::new();
    let root = env.user.join(format!(
        "sdmc/Nintendo 3DS/{ID}/{ID}/title/00040000"
    ));
    let plain = ncch(&NcchSpec::new(APP_TITLE, Some(mock_smdh("Deep", "Maker", 1, 0, 1))));
    env.write(root.join("a/b/c/d/e/f/g/h/deep.app"), &plain);

    assert_eq!(env.scan::<AndroidPlatform>().len(), 1);
}

#[test]
fn an_installed_update_supplies_the_smdh() {
    let env = Env::new();
    let base = mock_smdh("Base Title", "Maker", 0b0000_0001, 0, 1);
    let update = mock_smdh("Updated Title", "Maker", 0b0000_0010, 0, 2);
    env.write(
        env.sdmc_content("00040000", "00123400").join("00000000.app"),
        &ncch(&NcchSpec::new(APP_TITLE, Some(base))),
    );
    let update_content = env.sdmc_content("0004000e", "00123400");
    env.write(update_content.join("00000000.tmd"), &tmd(UPDATE_TITLE, &[0x1]));
    env.write(
        update_content.join("00000001.app"),
        &ncch(&NcchSpec::new(UPDATE_TITLE, Some(update))),
    );

    let game = env.scan::<AndroidPlatform>().remove(0);

    assert_eq!(game.title, "Updated Title");
    assert_eq!(game.regions, "North America");
}

#[test]
fn an_update_without_content_leaves_the_base_smdh() {
    let env = Env::new();
    let base = mock_smdh("Base Title", "Maker", 0b0000_0001, 0, 1);
    env.write(
        env.sdmc_content("00040000", "00123400").join("00000000.app"),
        &ncch(&NcchSpec::new(APP_TITLE, Some(base))),
    );
    let update_content = env.sdmc_content("0004000e", "00123400");
    env.write(update_content.join("00000000.tmd"), &tmd(UPDATE_TITLE, &[0x1]));

    assert_eq!(env.scan::<AndroidPlatform>()[0].title, "Base Title");
}

#[test]
fn the_dlc_folder_is_used_when_the_second_content_is_optional() {
    let env = Env::new();
    let base = mock_smdh("Base Title", "Maker", 1, 0, 1);
    let update = mock_smdh("Updated Title", "Maker", 1, 0, 2);
    env.write(
        env.sdmc_content("00040000", "00123400").join("00000000.app"),
        &ncch(&NcchSpec::new(APP_TITLE, Some(base))),
    );
    let update_content = env.sdmc_content("0004000e", "00123400");
    let mut metadata = emu_app_scanner::tmd::Tmd::decode(&tmd(UPDATE_TITLE, &[0x1, 0x2])).unwrap();
    metadata.chunks[1].kind = emu_app_scanner::tmd::CONTENT_OPTIONAL;
    env.write(update_content.join("00000000.tmd"), &metadata.encode());
    env.write(
        update_content.join("00000000/00000001.app"),
        &ncch(&NcchSpec::new(UPDATE_TITLE, Some(update))),
    );

    assert_eq!(env.scan::<AndroidPlatform>()[0].title, "Updated Title");
}

#[test]
fn the_smallest_tmd_is_the_base_metadata() {
    let env = Env::new();
    let base = mock_smdh("Base Title", "Maker", 1, 0, 1);
    env.write(
        env.sdmc_content("00040000", "00123400").join("00000000.app"),
        &ncch(&NcchSpec::new(APP_TITLE, Some(base))),
    );
    let update_content = env.sdmc_content("0004000e", "00123400");
    env.write(update_content.join("00000000.tmd"), &tmd(UPDATE_TITLE, &[0x7]));
    env.write(update_content.join("00000001.tmd"), &tmd(UPDATE_TITLE, &[0x9]));
    env.write(
        update_content.join("00000007.app"),
        &ncch(&NcchSpec::new(UPDATE_TITLE, Some(mock_smdh("From Base TMD", "Maker", 1, 0, 2)))),
    );
    env.write(
        update_content.join("00000009.app"),
        &ncch(&NcchSpec::new(UPDATE_TITLE, Some(mock_smdh("From Newest TMD", "Maker", 1, 0, 3)))),
    );

    assert_eq!(env.scan::<AndroidPlatform>()[0].title, "From Base TMD");
}

#[test]
fn the_icons_are_stored_in_the_requested_directory() {
    let env = Env::new();
    env.game_file("icon.3dsx", &threedsx(Some(&mock_smdh("Icon Game", "Maker", 1, 0, 8))));

    let icon = env.scan::<AndroidPlatform>().remove(0).icon_path.unwrap();

    assert!(icon.starts_with(env.icons.to_str().unwrap()));
}
