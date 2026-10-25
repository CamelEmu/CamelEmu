mod common;

use common::*;
use emu_app_scanner::format::BinaryFormat;
use emu_app_scanner::platform::{AndroidPlatform, LinuxPlatform, ScanPlatform};

const APP_TITLE: u64 = 0x0004_0000_0012_3400;
const SYSTEM_TITLE: u64 = 0x0004_0010_0002_0000;

on_each_platform! {
    a_3dsx_with_an_smdh_is_listed_with_its_english_titles_and_icon(env) {
        let smdh = mock_smdh("Homebrew Game", "Some Studio", 0b0000_0110, 1, 7);
        env.game_file("game.3dsx", &threedsx(Some(&smdh)));

        let games = env.scan::<P>();

        assert_eq!(games.len(), 1);
        let game = &games[0];
        assert_eq!(game.title, "Homebrew Game");
        assert_eq!(game.company, "Some Studio");
        assert_eq!(game.regions, "North America, Europe");
        assert_eq!(game.filename, "game.3dsx");
        assert!(game.path.ends_with("/game.3dsx"));
        assert!(!game.is_installed);
        assert!(game.is_visible_system_title);
        assert!(!game.is_system_title);
        assert_eq!(game.title_id, 0);
        let (width, height, pixels) = decode_png(game.icon_path.as_deref().unwrap());
        assert_eq!((width, height), (48, 48));
        assert_eq!(pixels, expected_rgba(&smdh));
    }

    a_3dsx_without_an_smdh_is_listed_by_its_file_name(env) {
        env.game_file("plain.3dsx", &threedsx(None));

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "plain.3dsx");
        assert_eq!(game.company, "");
        assert_eq!(game.regions, "Invalid region");
        assert_eq!(game.icon_path, None);
        assert!(!game.is_visible_system_title);
    }

    an_elf_is_listed_by_its_file_name(env) {
        env.game_file("tool.elf", &elf());

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "tool.elf");
        assert_eq!(game.regions, "Invalid region");
    }

    a_cxi_reads_the_smdh_from_the_exefs_and_the_program_id_from_the_header(env) {
        let smdh = mock_smdh("Retail Game", "Publisher", 0x7F, 0, 21);
        env.game_file("retail.cxi", &ncch(&NcchSpec::new(APP_TITLE, Some(smdh.clone()))));

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "Retail Game");
        assert_eq!(game.company, "Publisher");
        assert_eq!(game.regions, "Region free");
        assert_eq!(game.title_id, APP_TITLE as i64);
        assert!(!game.is_system_title);
        assert!(!game.is_visible_system_title);
        let (_, _, pixels) = decode_png(game.icon_path.as_deref().unwrap());
        assert_eq!(pixels, expected_rgba(&smdh));
    }

    a_cci_reads_the_first_partition(env) {
        let smdh = mock_smdh("Cartridge Game", "Maker", 0b0000_0001, 0, 3);
        let partition = ncch(&NcchSpec::new(APP_TITLE, Some(smdh)));
        env.game_file("cart.cci", &ncsd(&partition));

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "Cartridge Game");
        assert_eq!(game.regions, "Japan");
        assert_eq!(game.title_id, APP_TITLE as i64);
    }

    the_file_content_wins_over_the_extension(env) {
        let smdh = mock_smdh("Misnamed", "Maker", 0b0000_0010, 0, 1);
        env.game_file("misnamed.3dsx", &ncch(&NcchSpec::new(APP_TITLE, Some(smdh))));

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "Misnamed");
        assert_eq!(game.title_id, APP_TITLE as i64);
    }

    an_encrypted_ncch_keeps_its_file_name_and_is_marked_encrypted(env) {
        let smdh = mock_smdh("Hidden", "Maker", 0x7F, 0, 1);
        let mut spec = NcchSpec::new(APP_TITLE, Some(smdh));
        spec.no_crypto = false;
        env.game_file("locked.cxi", &ncch(&spec));

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "locked.cxi");
        assert_eq!(game.regions, P::ENCRYPTED_REGIONS);
        assert_eq!(game.icon_path, None);
        assert_eq!(game.title_id, APP_TITLE as i64);
    }

    extensions_are_matched_without_regard_to_case(env) {
        env.game_file("LOUD.3DSX", &threedsx(None));

        assert_eq!(env.scan::<P>().len(), 1);
    }

    the_games_directory_is_searched_three_levels_deep(env) {
        env.game_file("a.3dsx", &threedsx(None));
        env.game_file("one/b.3dsx", &threedsx(None));
        env.game_file("one/two/c.3dsx", &threedsx(None));
        env.game_file("one/two/three/d.3dsx", &threedsx(None));

        let games = env.scan::<P>();

        let mut names: Vec<_> = games.iter().map(|game| game.filename.as_str()).collect();
        names.sort();
        assert_eq!(names, ["a.3dsx", "b.3dsx", "c.3dsx"]);
    }

    a_missing_games_directory_lists_nothing(env) {
        std::fs::remove_dir(&env.games).unwrap();

        assert!(env.scan::<P>().is_empty());
    }

    installed_applications_are_listed_from_the_sd_card(env) {
        let smdh = mock_smdh("Installed Game", "Maker", 0b0000_0100, 0, 9);
        env.write(
            env.sdmc_content("00040000", "00123400").join("00000000.app"),
            &ncch(&NcchSpec::new(APP_TITLE, Some(smdh))),
        );

        let games = env.scan::<P>();

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Installed Game");
        assert_eq!(games[0].regions, "Europe");
        assert!(games[0].is_installed);
        assert_eq!(games[0].filename, "00000000.app");
    }

    installed_system_titles_are_listed_from_the_nand(env) {
        let smdh = mock_smdh("System Settings", "Nintendo", 0x7F, 1, 5);
        env.write(
            env.nand_content("00040010", "00020000").join("00000000.app"),
            &ncch(&NcchSpec::new(SYSTEM_TITLE, Some(smdh))),
        );

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "System Settings");
        assert!(game.is_installed);
        assert!(game.is_system_title);
        assert!(game.is_visible_system_title);
        assert_eq!(game.title_id, SYSTEM_TITLE as i64);
    }

    installed_files_that_are_not_executable_are_skipped(env) {
        let content = env.sdmc_content("00040000", "00123400");
        env.write(content.join("00000000.app"), b"not a game");
        let mut manual = NcchSpec::new(APP_TITLE, Some(mock_smdh("Manual", "Maker", 1, 0, 1)));
        manual.is_executable = false;
        env.write(content.join("00000001.app"), &ncch(&manual));
        env.write(content.join("00000000.tmd"), &tmd(APP_TITLE, &[0, 1]));

        assert!(env.scan::<P>().is_empty());
    }

    an_icon_override_in_the_load_folder_wins_over_the_exefs(env) {
        let inside = mock_smdh("Inside", "Maker", 1, 0, 1);
        let outside = mock_smdh("Override", "Maker", 1, 0, 2);
        env.game_file("game.cxi", &ncch(&NcchSpec::new(APP_TITLE, Some(inside))));
        env.write(
            env.user.join("load/mods/0004000000123400/exefs/icon.bin"),
            &outside.encode(),
        );

        assert_eq!(env.scan::<P>()[0].title, "Override");
    }

    an_exefsdir_icon_override_wins_over_the_exefs(env) {
        let inside = mock_smdh("Inside", "Maker", 1, 0, 1);
        let outside = mock_smdh("Side File", "Maker", 1, 0, 2);
        let path = env.game_file("game.cxi", &ncch(&NcchSpec::new(APP_TITLE, Some(inside))));
        env.write(format!("{}.exefsdir/icon.bin", path.display()), &outside.encode());

        assert_eq!(env.scan::<P>()[0].title, "Side File");
    }

    a_split_exefs_file_replaces_the_exefs(env) {
        let inside = mock_smdh("Inside", "Maker", 1, 0, 1);
        let split = mock_smdh("Split Exefs", "Maker", 1, 0, 2);
        let path = env.game_file("game.cxi", &ncch(&NcchSpec::new(APP_TITLE, Some(inside))));
        let mut exefs = emu_app_scanner::ncch_format::ExeFsHeader {
            sections: vec![emu_app_scanner::ncch_format::ExeFsSection {
                name: "icon".into(),
                offset: 0,
                size: emu_app_scanner::smdh::SMDH_SIZE as u32,
            }],
        }
        .encode();
        exefs.extend(split.encode());
        env.write(format!("{}.exefs", path.display()), &exefs);

        assert_eq!(env.scan::<P>().remove(0).title, "Split Exefs");
    }

    an_exefs_without_an_icon_section_has_no_title(env) {
        env.game_file("game.cxi", &ncch(&NcchSpec::new(APP_TITLE, None)));

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "game.cxi");
        assert_eq!(game.regions, "Invalid region");
        assert_eq!(game.title_id, APP_TITLE as i64);
    }

    applications_stored_as_console_unique_files_are_read(env) {
        let otp = env.install_console();
        let smdh = mock_smdh("Console Bound", "Maker", 0b0000_1000, 0, 4);
        let plain = ncch(&NcchSpec::new(APP_TITLE, Some(smdh)));
        env.write(
            env.sdmc_content("00040000", "00123400").join("00000000.app"),
            &encrypted_for_console(&otp, &plain),
        );

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "Console Bound");
        assert_eq!(game.regions, "Australia");
        assert!(game.is_installed);
    }

    console_unique_files_cannot_be_read_without_the_console_data(env) {
        let otp = env.install_console();
        let smdh = mock_smdh("Console Bound", "Maker", 1, 0, 4);
        let plain = ncch(&NcchSpec::new(APP_TITLE, Some(smdh)));
        env.write(
            env.sdmc_content("00040000", "00123400").join("00000000.app"),
            &encrypted_for_console(&otp, &plain),
        );
        std::fs::remove_file(env.user.join("sysdata/otp.bin")).unwrap();

        assert!(env.scan::<P>().is_empty());
    }

    a_certificate_that_does_not_verify_rejects_console_unique_files(env) {
        let mut otp = env.install_console();
        let smdh = mock_smdh("Console Bound", "Maker", 1, 0, 4);
        let plain = ncch(&NcchSpec::new(APP_TITLE, Some(smdh)));
        env.write(
            env.sdmc_content("00040000", "00123400").join("00000000.app"),
            &encrypted_for_console(&otp, &plain),
        );
        otp.ct_signature[0] ^= 0xFF;
        let keys_text = std::fs::read(env.user.join("sysdata/keys.txt")).unwrap();
        let keys = emu_app_scanner::unique::KeysFile::decode(&keys_text).unwrap();
        env.write(env.user.join("sysdata/otp.bin"), &otp.encode_encrypted(&keys));

        assert!(env.scan::<P>().is_empty());
    }

    console_unique_games_in_the_games_directory_are_read_too(env) {
        let otp = env.install_console();
        let smdh = mock_smdh("Bound Game", "Maker", 0b0001_0000, 0, 6);
        let plain = ncch(&NcchSpec::new(APP_TITLE, Some(smdh)));
        env.game_file("bound.cxi", &encrypted_for_console(&otp, &plain));

        let game = env.scan::<P>().remove(0);

        assert_eq!(game.title, "Bound Game");
        assert_eq!(game.regions, "China");
    }

    the_icons_are_written_once_and_reused(env) {
        let smdh = mock_smdh("Icon Game", "Maker", 1, 0, 8);
        env.game_file("icon.3dsx", &threedsx(Some(&smdh)));

        let first = env.scan::<P>().remove(0).icon_path.unwrap();
        let modified = std::fs::metadata(&first).unwrap().modified().unwrap();
        let second = env.scan::<P>().remove(0).icon_path.unwrap();

        assert_eq!(first, second);
        assert_eq!(std::fs::metadata(&second).unwrap().modified().unwrap(), modified);
    }

    games_found_in_both_places_are_listed_twice(env) {
        let smdh = mock_smdh("Both", "Maker", 1, 0, 1);
        env.game_file("both.cxi", &ncch(&NcchSpec::new(APP_TITLE, Some(smdh.clone()))));
        env.write(
            env.sdmc_content("00040000", "00123400").join("00000000.app"),
            &ncch(&NcchSpec::new(APP_TITLE, Some(smdh))),
        );

        let games = env.scan::<P>();

        assert_eq!(games.len(), 2);
        assert_eq!(games.iter().filter(|game| game.is_installed).count(), 1);
    }
}
