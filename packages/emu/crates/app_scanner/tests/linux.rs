mod common;

use common::*;
use emu_app_scanner::platform::LinuxPlatform;

const APP_TITLE: u64 = 0x0004_0000_0012_3400;

#[test]
fn archives_and_other_files_are_not_listed() {
    let env = Env::new();
    env.game_file("pack.zip", b"PK");
    env.game_file("pack.7z", b"7z");
    env.game_file("notes.txt", b"ignored");

    assert!(env.scan::<LinuxPlatform>().is_empty());
}

#[test]
fn files_the_loader_cannot_read_are_not_listed() {
    let env = Env::new();
    env.game_file("broken.cxi", b"not a game");
    env.game_file("empty.cci", b"");

    assert!(env.scan::<LinuxPlatform>().is_empty());
}

#[test]
fn files_that_are_not_executable_are_not_listed() {
    let env = Env::new();
    let mut manual = NcchSpec::new(APP_TITLE, Some(mock_smdh("Manual", "Maker", 1, 0, 1)));
    manual.is_executable = false;
    env.game_file("manual.cxi", &ncch(&manual));

    assert!(env.scan::<LinuxPlatform>().is_empty());
}

#[test]
fn the_description_is_empty_and_the_title_is_kept_as_it_is() {
    let env = Env::new();
    let smdh = mock_smdh("Line one\r\nLine\ttwo", "Maker", 1, 0, 1);
    env.game_file("lines.3dsx", &threedsx(Some(&smdh)));

    let game = env.scan::<LinuxPlatform>().remove(0);

    assert_eq!(game.description, "");
    assert_eq!(game.title, "Line one\r\nLine\ttwo");
}

#[test]
fn an_installed_update_does_not_replace_the_smdh() {
    let env = Env::new();
    let base = mock_smdh("Base Title", "Maker", 0b0000_0001, 0, 1);
    let update = mock_smdh("Updated Title", "Maker", 0b0000_0010, 0, 2);
    env.write(
        env.sdmc_content("00040000", "00123400").join("00000000.app"),
        &ncch(&NcchSpec::new(APP_TITLE, Some(base))),
    );
    let update_content = env.sdmc_content("0004000e", "00123400");
    env.write(update_content.join("00000000.tmd"), &tmd(0x0004_000e_0012_3400, &[0x1]));
    env.write(
        update_content.join("00000001.app"),
        &ncch(&NcchSpec::new(0x0004_000e_0012_3400, Some(update))),
    );

    assert_eq!(env.scan::<LinuxPlatform>()[0].title, "Base Title");
}

#[test]
fn installed_encrypted_applications_are_listed_as_encrypted() {
    let env = Env::new();
    let mut spec = NcchSpec::new(APP_TITLE, Some(mock_smdh("Locked", "Maker", 1, 0, 1)));
    spec.no_crypto = false;
    env.write(
        env.sdmc_content("00040000", "00123400").join("00000000.app"),
        &ncch(&spec),
    );

    let game = env.scan::<LinuxPlatform>().remove(0);

    assert!(game.is_installed);
    assert_eq!(game.regions, "Encrypted");
    assert_eq!(game.title, "00000000.app");
}

#[test]
fn the_icons_are_stored_in_the_user_directory() {
    let env = Env::new();
    env.game_file("icon.3dsx", &threedsx(Some(&mock_smdh("Icon Game", "Maker", 1, 0, 8))));

    let icon = env.scan::<LinuxPlatform>().remove(0).icon_path.unwrap();

    assert!(icon.starts_with(env.user.join("icons").to_str().unwrap()));
}

#[test]
fn installed_applications_are_searched_five_levels_deep() {
    let env = Env::new();
    let root = env.user.join(format!(
        "sdmc/Nintendo 3DS/{ID}/{ID}/title/00040000"
    ));
    let plain = ncch(&NcchSpec::new(APP_TITLE, Some(mock_smdh("Deep", "Maker", 1, 0, 1))));
    env.write(root.join("a/b/c/d/within.app"), &plain);
    env.write(root.join("a/b/c/d/e/beyond.app"), &plain);

    let games = env.scan::<LinuxPlatform>();

    assert_eq!(games.len(), 1);
    assert_eq!(games[0].filename, "within.app");
}
