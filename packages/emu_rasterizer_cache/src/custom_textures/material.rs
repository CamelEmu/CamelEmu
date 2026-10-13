use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::custom_textures::custom_format::{CustomFileFormat, CustomPixelFormat};

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MapType {
    #[default]
    Color = 0,
    Normal = 1,
    MapCount = 2,
}

pub const MAX_MAPS: usize = MapType::MapCount as usize;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DecodeState {
    #[default]
    None = 0,
    Pending = 1,
    Decoded = 2,
    Failed = 3,
}

#[derive(Debug, Default)]
pub struct CustomTexture {
    pub path: String,
    pub width: u32,
    pub height: u32,
    pub hashes: Vec<u64>,
    pub format: CustomPixelFormat,
    pub file_format: CustomFileFormat,
    pub data: Vec<u8>,
    pub ty: MapType,
}

impl CustomTexture {
    pub fn is_parsed(&self) -> bool {
        self.file_format != CustomFileFormat::None && !self.hashes.is_empty()
    }

    pub fn is_loaded(&self) -> bool {
        !self.data.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct Material {
    pub width: u32,
    pub height: u32,
    pub size: u64,
    pub hash: u64,
    pub format: CustomPixelFormat,
    pub textures: [Option<Arc<CustomTexture>>; MAX_MAPS],
    pub state: AtomicU32,
}

impl Material {
    pub fn map(&self, ty: MapType) -> Option<&CustomTexture> {
        self.textures[ty as usize].as_deref()
    }

    fn state(&self) -> u32 {
        self.state.load(Ordering::SeqCst)
    }

    pub fn is_pending(&self) -> bool {
        self.state() == DecodeState::Pending as u32
    }

    pub fn is_failed(&self) -> bool {
        self.state() == DecodeState::Failed as u32
    }

    pub fn is_decoded(&self) -> bool {
        self.state() == DecodeState::Decoded as u32
    }

    pub fn is_unloaded(&self) -> bool {
        self.state() == DecodeState::None as u32
    }
}
