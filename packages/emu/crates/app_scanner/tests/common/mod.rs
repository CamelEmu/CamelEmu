#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use emu_app_scanner::format::BinaryFormat;
use emu_app_scanner::ncch_format::{
    ExHeader, ExeFsHeader, ExeFsSection, NcchHeader, NcsdHeader, NcsdPartition,
};
use emu_app_scanner::smdh::{ENGLISH, SMDH_SIZE, Smdh};
use emu_app_scanner::threedsx::ThreeDsxHeader;
use emu_app_scanner::tmd::{ContentChunk, Tmd};
use emu_app_scanner::unique::{KeysFile, OtpBin, certificate_body, encrypt_file_contents};
use emu_app_scanner::platform::ScanPlatform;
use emu_app_scanner::{Game, ScanRequest, scan_games_for};
use emu_crypto::ecc;
use tempfile::TempDir;

static LOCK: Mutex<()> = Mutex::new(());

pub const ID: &str = "00000000000000000000000000000000";

pub const ROOT_PRIVATE_KEY: &str =
    "3b887ebdb67af31480c9bc39de42c6c9c1eed0bdd792cf9ec3fa01bf403557f3";
const SIGNING_NONCE: &str = "1234567890abcdef1234567890abcdef1234567890abcdef";

#[macro_export]
macro_rules! on_each_platform {
    ($($name:ident($env:ident) $body:block)*) => {
        $(
            mod $name {
                use super::*;

                fn run<P: ScanPlatform>($env: &Env) $body

                #[test]
                fn on_linux() {
                    run::<LinuxPlatform>(&Env::new());
                }

                #[test]
                fn on_android() {
                    run::<AndroidPlatform>(&Env::new());
                }
            }
        )*
    };
}

pub fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}

pub struct Env {
    _guard: MutexGuard<'static, ()>,
    _dir: TempDir,
    pub user: PathBuf,
    pub games: PathBuf,
    pub icons: PathBuf,
}

impl Env {
    pub fn new() -> Self {
        let guard = LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user");
        let games = dir.path().join("games");
        let icons = dir.path().join("icons");
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(&games).unwrap();
        emu_storage::set_user_directory(user.to_str().unwrap()).unwrap();
        Self {
            _guard: guard,
            _dir: dir,
            user,
            games,
            icons,
        }
    }

    pub fn scan<P: ScanPlatform>(&self) -> Vec<Game> {
        scan_games_for::<P>(ScanRequest {
            games_directory: Some(self.games.to_string_lossy().into_owned()),
            icons_directory: self.icons.to_string_lossy().into_owned(),
        })
    }

    pub fn write(&self, path: impl AsRef<Path>, bytes: &[u8]) -> PathBuf {
        let path = path.as_ref().to_path_buf();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }

    pub fn game_file(&self, relative: &str, bytes: &[u8]) -> PathBuf {
        self.write(self.games.join(relative), bytes)
    }

    pub fn sdmc_content(&self, high: &str, low: &str) -> PathBuf {
        self.user
            .join(format!("sdmc/Nintendo 3DS/{ID}/{ID}/title/{high}/{low}/content"))
    }

    pub fn nand_content(&self, high: &str, low: &str) -> PathBuf {
        self.user.join(format!("nand/{ID}/title/{high}/{low}/content"))
    }

    pub fn find<'a>(&self, games: &'a [Game], filename: &str) -> &'a Game {
        games
            .iter()
            .find(|game| game.filename == filename)
            .unwrap_or_else(|| panic!("{filename} is not listed in {games:#?}"))
    }

    pub fn install_console(&self) -> OtpBin {
        let root_private = hex(ROOT_PRIVATE_KEY);
        let keys = KeysFile {
            otp_key: *b"0123456789abcdef",
            otp_iv: *b"fedcba9876543210",
            root_public_xy: ecc::public_key(&root_private).unwrap(),
        };
        let mut otp = OtpBin {
            device_id: 0x1234_ABCD,
            otp_version: 5,
            system_type: 0,
            ct_expiry: 1_234_567u32.to_le_bytes(),
            ct_private_key: [0x42; 0x20],
            ..Default::default()
        };
        let public_key = ecc::public_key(&otp.ct_private_key).unwrap();
        otp.ct_signature = ecc::sign(
            &certificate_body(&otp, &public_key),
            &root_private,
            &hex(SIGNING_NONCE),
        )
        .unwrap();
        self.write(self.user.join("sysdata/keys.txt"), &keys.encode());
        self.write(self.user.join("sysdata/otp.bin"), &otp.encode_encrypted(&keys));
        otp
    }
}

pub fn mock_smdh(title: &str, publisher: &str, region_lockout: u32, flags: u32, seed: u16) -> Smdh {
    let mut smdh = Smdh::default();
    smdh.titles[0].long = format!("{title} (JP)");
    smdh.titles[0].publisher = format!("{publisher} (JP)");
    smdh.titles[ENGLISH].short = title.chars().take(10).collect();
    smdh.titles[ENGLISH].long = title.to_owned();
    smdh.titles[ENGLISH].publisher = publisher.to_owned();
    smdh.region_lockout = region_lockout;
    smdh.flags = flags;
    for (index, pixel) in smdh.large_icon.iter_mut().enumerate() {
        *pixel = (index as u16).wrapping_mul(2654).wrapping_add(seed);
    }
    smdh
}

pub fn threedsx(smdh: Option<&Smdh>) -> Vec<u8> {
    let mut bytes = match smdh {
        Some(_) => ThreeDsxHeader::with_smdh(0x80, SMDH_SIZE as u32).encode(),
        None => ThreeDsxHeader {
            header_size: emu_app_scanner::threedsx::HEADER_SIZE,
            ..Default::default()
        }
        .encode(),
    };
    bytes.resize(0x80, 0);
    if let Some(smdh) = smdh {
        bytes.extend(smdh.encode());
    }
    bytes
}

pub fn elf() -> Vec<u8> {
    let mut bytes = b"\x7fELF".to_vec();
    bytes.resize(0x100, 0);
    bytes
}

#[derive(Clone)]
pub struct NcchSpec {
    pub program_id: u64,
    pub icon: Option<Smdh>,
    pub no_crypto: bool,
    pub is_executable: bool,
}

impl NcchSpec {
    pub fn new(program_id: u64, icon: Option<Smdh>) -> Self {
        Self {
            program_id,
            icon,
            no_crypto: true,
            is_executable: true,
        }
    }
}

const NCCH_EXEFS_OFFSET: usize = 0xA00;

pub fn ncch(spec: &NcchSpec) -> Vec<u8> {
    let mut sections = Vec::new();
    let mut body = Vec::new();
    if let Some(icon) = &spec.icon {
        sections.push(ExeFsSection {
            name: "icon".into(),
            offset: 0,
            size: SMDH_SIZE as u32,
        });
        body.extend(icon.encode());
    }
    sections.push(ExeFsSection {
        name: ".code".into(),
        offset: body.len() as u32,
        size: 0x10,
    });
    body.extend([0xC0; 0x10]);

    let exefs_length = (0x200 + body.len()).next_multiple_of(0x200);
    let total = NCCH_EXEFS_OFFSET + exefs_length;
    let header = NcchHeader {
        content_size: (total / 0x200) as u32,
        program_id: spec.program_id,
        extended_header_size: 0x400,
        is_executable: spec.is_executable,
        no_crypto: spec.no_crypto,
        exefs_offset: (NCCH_EXEFS_OFFSET / 0x200) as u32,
        exefs_size: (exefs_length / 0x200) as u32,
        ..Default::default()
    };
    let mut bytes = vec![0; total];
    bytes[..0x200].copy_from_slice(&header.encode());
    bytes[0x200..0xA00].copy_from_slice(
        &ExHeader {
            program_id: spec.program_id,
            jump_id: spec.program_id,
            ..Default::default()
        }
        .encode(),
    );
    let exefs = ExeFsHeader { sections }.encode();
    bytes[NCCH_EXEFS_OFFSET..NCCH_EXEFS_OFFSET + 0x200].copy_from_slice(&exefs);
    bytes[NCCH_EXEFS_OFFSET + 0x200..NCCH_EXEFS_OFFSET + 0x200 + body.len()].copy_from_slice(&body);
    bytes
}

pub fn ncsd(partition: &[u8]) -> Vec<u8> {
    const PARTITION_BLOCK: u32 = 0x20;
    let mut header = NcsdHeader::default();
    header.partitions[0] = NcsdPartition {
        offset: PARTITION_BLOCK,
        size: (partition.len() / 0x200) as u32,
    };
    let mut bytes = header.encode();
    bytes.resize(PARTITION_BLOCK as usize * 0x200, 0);
    bytes.extend_from_slice(partition);
    bytes
}

pub fn tmd(title_id: u64, content_ids: &[u32]) -> Vec<u8> {
    Tmd {
        title_id,
        chunks: content_ids
            .iter()
            .enumerate()
            .map(|(index, id)| ContentChunk {
                id: *id,
                index: index as u16,
                size: 0x1000,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
    .encode()
}

pub fn encrypted_for_console(otp: &OtpBin, plain: &[u8]) -> Vec<u8> {
    encrypt_file_contents(otp, plain).unwrap()
}

pub fn expected_rgba(smdh: &Smdh) -> Vec<u8> {
    smdh.large_icon
        .iter()
        .flat_map(|pixel| {
            let r = ((pixel >> 11) & 0x1F) as u8;
            let g = ((pixel >> 5) & 0x3F) as u8;
            let b = (pixel & 0x1F) as u8;
            [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2), 255]
        })
        .collect()
}

pub fn decode_png(path: &str) -> (u32, u32, Vec<u8>) {
    let decoder = png::Decoder::new(std::io::BufReader::new(fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buffer).unwrap();
    buffer.truncate(info.buffer_size());
    (info.width, info.height, buffer)
}
