use crate::format::BinaryFormat;
use crate::source::read_whole_file;
use crate::tmd::{CONTENT_OPTIONAL, Tmd};

pub const SYSTEM_ID: &str = "00000000000000000000000000000000";
pub const SDCARD_ID: &str = "00000000000000000000000000000000";

pub const UPDATE_MASK: u64 = 0x0000_000e_0000_0000;

pub fn sdmc_dir() -> String {
    format!("{}sdmc/", emu_storage::user_path())
}

pub fn nand_dir() -> String {
    format!("{}nand/", emu_storage::user_path())
}

pub fn load_dir() -> String {
    format!("{}load/", emu_storage::user_path())
}

pub fn sdmc_title_root() -> String {
    format!("{}Nintendo 3DS/{SYSTEM_ID}/{SDCARD_ID}/title/", sdmc_dir())
}

pub fn nand_title_root() -> String {
    format!("{}{SYSTEM_ID}/title/", nand_dir())
}

fn title_path(title_id: u64) -> String {
    format!(
        "{}{:08x}/{:08x}/",
        sdmc_title_root(),
        (title_id >> 32) as u32,
        title_id as u32
    )
}

fn metadata_path(title_id: u64) -> String {
    let content = format!("{}content/", title_path(title_id));
    let mut base_id = u32::MAX;
    for entry in emu_storage::list_entries(content.trim_end_matches('/')).unwrap_or_default() {
        let Some((stem, "tmd")) = entry.name.rsplit_once('.') else {
            continue;
        };
        let Ok(id) = u32::from_str_radix(stem, 16) else {
            continue;
        };
        base_id = base_id.min(id);
    }
    if base_id == u32::MAX {
        base_id = 0;
    }
    format!("{content}{base_id:08x}.tmd")
}

pub fn content_path(title_id: u64) -> Option<String> {
    let mut content = format!("{}content/", title_path(title_id));
    let mut content_id = 0;
    if let Some(tmd) = read_whole_file(&metadata_path(title_id))
        .and_then(|bytes| Tmd::decode(&bytes).ok())
    {
        content_id = tmd.content_id(0)?;
        if tmd.chunks.len() > 1 && tmd.content_type(1)? & CONTENT_OPTIONAL != 0 {
            content.push_str("00000000/");
        }
    }
    Some(format!("{content}{content_id:08x}.app"))
}
