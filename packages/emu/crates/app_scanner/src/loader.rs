use crate::format::BinaryFormat;
use crate::ncch_format::{
    BLOCK_SIZE, EXEFS_HEADER_SIZE, EXHEADER_SIZE, ExHeader, ExeFsHeader, NCCH_HEADER_SIZE,
    NCSD_HEADER_SIZE, NcchHeader, NcsdHeader,
};
use crate::source::{PlainSource, Source, read_exact_at};
use crate::threedsx::{HEADER_SIZE as THREEDSX_HEADER_SIZE, ThreeDsxHeader};
use crate::title;
use crate::unique;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    Error,
    ErrorInvalidFormat,
    ErrorNotImplemented,
    ErrorNotUsed,
    ErrorEncrypted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileType {
    Unknown,
    Cci,
    Cxi,
    Cia,
    Elf,
    ThreeDsx,
}

pub trait AppLoader {
    fn is_executable(&mut self) -> (Status, bool) {
        (Status::Success, true)
    }

    fn read_program_id(&mut self) -> (Status, u64) {
        (Status::ErrorNotImplemented, 0)
    }

    fn read_icon(&mut self) -> (Status, Vec<u8>) {
        (Status::ErrorNotImplemented, Vec::new())
    }
}

fn magic_at(source: &dyn Source, offset: u64) -> Option<[u8; 4]> {
    read_exact_at(source, offset, 4)?.try_into().ok()
}

fn identify_ncch(source: &dyn Source) -> Option<FileType> {
    match &magic_at(source, 0x100)? {
        b"NCSD" => Some(FileType::Cci),
        b"NCCH" => Some(FileType::Cxi),
        _ => None,
    }
}

fn identify(source: &dyn Source, path: &str) -> FileType {
    if magic_at(source, 0) == Some(*b"3DSX") {
        return FileType::ThreeDsx;
    }
    if magic_at(source, 0) == Some(*b"\x7fELF") {
        return FileType::Elf;
    }
    if magic_at(source, 0x100).is_none() {
        return FileType::Unknown;
    }
    if let Some(file_type) = identify_ncch(source) {
        return file_type;
    }
    unique::open_crypto_source(path)
        .and_then(|crypto| identify_ncch(&crypto))
        .unwrap_or(FileType::Unknown)
}

fn split_extension(path: &str) -> &str {
    let directory_end = path.rfind('/').map_or(0, |index| index + 1);
    match path.rfind('.') {
        Some(index) if index >= directory_end => &path[index..],
        _ => "",
    }
}

fn guess_from_extension(extension: &str) -> FileType {
    match extension.to_lowercase().as_str() {
        ".elf" | ".axf" => FileType::Elf,
        ".cci" => FileType::Cci,
        ".cxi" | ".app" => FileType::Cxi,
        ".3dsx" => FileType::ThreeDsx,
        ".cia" => FileType::Cia,
        _ => FileType::Unknown,
    }
}

pub fn get_loader(path: &str) -> Option<Box<dyn AppLoader>> {
    let source = PlainSource::open(path)?;
    let mut file_type = identify(&source, path);
    let from_extension = guess_from_extension(split_extension(path));
    if file_type != from_extension && file_type == FileType::Unknown {
        file_type = from_extension;
    }
    match file_type {
        FileType::ThreeDsx => Some(Box::new(ThreeDsxLoader { source })),
        FileType::Elf => Some(Box::new(ElfLoader)),
        FileType::Cxi | FileType::Cci => Some(Box::new(NcchLoader {
            container: NcchContainer::new(path),
        })),
        FileType::Unknown | FileType::Cia => None,
    }
}

struct ElfLoader;

impl AppLoader for ElfLoader {}

struct ThreeDsxLoader {
    source: PlainSource,
}

impl AppLoader for ThreeDsxLoader {
    fn read_icon(&mut self) -> (Status, Vec<u8>) {
        let Some(bytes) = read_exact_at(&self.source, 0, THREEDSX_HEADER_SIZE as usize) else {
            return (Status::Error, Vec::new());
        };
        let Ok(header) = ThreeDsxHeader::decode(&bytes) else {
            return (Status::Error, Vec::new());
        };
        if header.header_size != THREEDSX_HEADER_SIZE {
            return (Status::Error, Vec::new());
        }
        if header.smdh_offset == 0 {
            return (Status::ErrorNotUsed, Vec::new());
        }
        match read_exact_at(
            &self.source,
            u64::from(header.smdh_offset),
            header.smdh_size as usize,
        ) {
            Some(smdh) => (Status::Success, smdh),
            None => (Status::Error, Vec::new()),
        }
    }
}

struct NcchLoader {
    container: NcchContainer,
}

impl AppLoader for NcchLoader {
    fn is_executable(&mut self) -> (Status, bool) {
        let status = self.container.load();
        if status != Status::Success {
            return (status, false);
        }
        (Status::Success, self.container.header.is_executable)
    }

    fn read_icon(&mut self) -> (Status, Vec<u8>) {
        self.container.load_section_exefs("icon")
    }

    fn read_program_id(&mut self) -> (Status, u64) {
        let status = self.container.load_header();
        if status != Status::Success {
            return (status, 0);
        }
        if !self.container.has_header {
            return (Status::ErrorNotUsed, 0);
        }
        (Status::Success, self.container.header.program_id)
    }
}

fn mod_id(program_id: u64) -> u64 {
    if program_id & 0x0000_00ff_0000_0000 == title::UPDATE_MASK {
        program_id & !title::UPDATE_MASK
    } else {
        program_id
    }
}

fn align_up(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

struct NcchContainer {
    path: String,
    file: Option<Box<dyn Source>>,
    ncch_offset: u64,
    partition: usize,
    header: NcchHeader,
    exefs_header: ExeFsHeader,
    exheader: ExHeader,
    has_header: bool,
    has_exheader: bool,
    has_exefs: bool,
    has_romfs: bool,
    is_proto: bool,
    is_tainted: bool,
    is_loaded: bool,
    exefs_offset: u64,
    exefs_file: Option<Box<dyn Source>>,
}

impl NcchContainer {
    fn new(path: &str) -> Self {
        Self {
            path: path.to_owned(),
            file: PlainSource::open(path).map(|source| Box::new(source) as Box<dyn Source>),
            ncch_offset: 0,
            partition: 0,
            header: NcchHeader::default(),
            exefs_header: ExeFsHeader::default(),
            exheader: ExHeader::default(),
            has_header: false,
            has_exheader: false,
            has_exefs: false,
            has_romfs: false,
            is_proto: false,
            is_tainted: false,
            is_loaded: false,
            exefs_offset: 0,
            exefs_file: None,
        }
    }

    fn open_crypto(&self) -> Option<Box<dyn Source>> {
        unique::open_crypto_source(&self.path).map(|source| Box::new(source) as Box<dyn Source>)
    }

    fn open_matching(&self) -> Option<Box<dyn Source>> {
        if self.file.as_deref().is_some_and(|file| file.is_crypto()) {
            self.open_crypto()
        } else {
            PlainSource::open(&self.path).map(|source| Box::new(source) as Box<dyn Source>)
        }
    }

    fn read_header_block(&mut self) -> Option<[u8; 4]> {
        let file = self.file.as_deref()?;
        let mut block = read_exact_at(file, self.ncch_offset, NCCH_HEADER_SIZE)?;
        if block[0x100..0x104] == *b"NCSD" {
            if let Some(ncsd) = read_exact_at(file, self.ncch_offset, NCSD_HEADER_SIZE)
                .and_then(|bytes| NcsdHeader::decode(&bytes).ok())
            {
                self.ncch_offset = u64::from(ncsd.partitions[self.partition].offset) * BLOCK_SIZE;
            }
            if let Some(next) = read_exact_at(file, self.ncch_offset, NCCH_HEADER_SIZE) {
                block = next;
            }
        }
        let magic: [u8; 4] = block[0x100..0x104].try_into().ok()?;
        if magic == *b"NCCH"
            && let Ok(header) = NcchHeader::decode(&block)
        {
            self.header = header;
        }
        Some(magic)
    }

    fn read_valid_header(&mut self) -> Status {
        for attempt in 0..2 {
            let Some(magic) = self.read_header_block() else {
                return Status::Error;
            };
            if magic != *b"NCCH" {
                if attempt == 0 {
                    self.file = self.open_crypto();
                } else {
                    return Status::ErrorInvalidFormat;
                }
            }
        }
        Status::Success
    }

    fn load_header(&mut self) -> Status {
        if self.has_header {
            return Status::Success;
        }
        let status = self.read_valid_header();
        if status != Status::Success {
            return status;
        }
        self.has_header = true;
        Status::Success
    }

    fn read_exheader(source: &dyn Source, offset: u64) -> Option<ExHeader> {
        ExHeader::decode(&read_exact_at(source, offset, EXHEADER_SIZE)?).ok()
    }

    fn load(&mut self) -> Status {
        if self.is_loaded {
            return Status::Success;
        }
        let mut block_size = BLOCK_SIZE;
        if self.file.is_some() {
            let status = self.read_valid_header();
            if status != Status::Success {
                return status;
            }
            let Some(file) = self.file.as_deref() else {
                return Status::Error;
            };
            let file_size = file.size();
            self.has_header = true;

            if u64::from(self.header.content_size) == file_size {
                self.is_proto = true;
                block_size = 1;
            }
            if !self.header.no_crypto {
                return Status::ErrorEncrypted;
            }

            if self.header.extended_header_size != 0 || self.is_proto {
                let Some(exheader) = Self::read_exheader(file, self.ncch_offset + BLOCK_SIZE)
                else {
                    return Status::Error;
                };
                self.exheader = exheader;

                let mods_path = format!(
                    "{}mods/{:016X}/",
                    title::load_dir(),
                    mod_id(self.header.program_id)
                );
                for override_path in [format!("{mods_path}exheader.bin"), format!("{}.exheader", self.path)] {
                    let overridden = PlainSource::open(&override_path)
                        .and_then(|source| Self::read_exheader(&source, 0));
                    if let Some(exheader) = overridden {
                        self.exheader = exheader;
                        self.is_tainted = true;
                        break;
                    }
                }
                self.has_exheader = true;
            }

            if self.header.exefs_size != 0 {
                self.exefs_offset = u64::from(self.header.exefs_offset) * block_size;
                let Some(bytes) = read_exact_at(
                    file,
                    self.exefs_offset + self.ncch_offset,
                    EXEFS_HEADER_SIZE,
                ) else {
                    return Status::Error;
                };
                let Ok(exefs_header) = ExeFsHeader::decode(&bytes) else {
                    return Status::Error;
                };
                self.exefs_header = exefs_header;
                self.exefs_file = if file.is_crypto() {
                    unique::open_crypto_source(&self.path)
                        .map(|source| Box::new(source) as Box<dyn Source>)
                } else {
                    PlainSource::open(&self.path).map(|source| Box::new(source) as Box<dyn Source>)
                };
                self.has_exefs = true;
            }

            if self.header.romfs_offset != 0 && self.header.romfs_size != 0 {
                self.has_romfs = true;
            }
        }

        self.load_overrides();

        if !(self.has_exefs || self.has_romfs || self.is_tainted) {
            return Status::Error;
        }
        self.is_loaded = true;
        Status::Success
    }

    fn load_overrides(&mut self) {
        if emu_storage::exists(&format!("{}.romfs", self.path)) {
            self.is_tainted = true;
        }

        let exefs_override = format!("{}.exefs", self.path);
        let exefs_directory = format!("{}.exefsdir", self.path);
        if emu_storage::exists(&exefs_override) {
            let overridden = PlainSource::open(&exefs_override);
            let header = overridden
                .as_ref()
                .and_then(|source| read_exact_at(source, 0, EXEFS_HEADER_SIZE))
                .and_then(|bytes| ExeFsHeader::decode(&bytes).ok());
            match (overridden, header) {
                (Some(source), Some(header)) => {
                    self.exefs_file = Some(Box::new(source));
                    self.exefs_header = header;
                    self.exefs_offset = 0;
                    self.is_tainted = true;
                    self.has_exefs = true;
                }
                (source, _) => {
                    self.exefs_file = match self.open_matching() {
                        Some(file) => Some(file),
                        None => source.map(|source| Box::new(source) as Box<dyn Source>),
                    };
                }
            }
        } else if emu_storage::exists(&exefs_directory) && emu_storage::is_directory(&exefs_directory)
        {
            self.is_tainted = true;
        }
    }

    fn load_override_icon(&self) -> (Status, Vec<u8>) {
        let mods_path = format!(
            "{}mods/{:016X}/",
            title::load_dir(),
            mod_id(self.header.program_id)
        );
        let candidates = [
            format!("{mods_path}exefs/icon.bin"),
            format!("{mods_path}icon.bin"),
            format!("{}.exefsdir/icon.bin", self.path),
        ];
        for candidate in candidates {
            if let Some(source) = PlainSource::open(&candidate)
                && let Some(buffer) = read_exact_at(&source, 0, source.size() as usize)
            {
                return (Status::Success, buffer);
            }
        }
        (Status::ErrorNotUsed, Vec::new())
    }

    fn load_section_exefs(&mut self, name: &str) -> (Status, Vec<u8>) {
        let status = self.load();
        if status != Status::Success {
            return (status, Vec::new());
        }

        let (result, buffer) = self.load_override_icon();
        if result == Status::Success || !self.has_exefs {
            return (result, buffer);
        }

        let Some(exefs_file) = self.exefs_file.as_deref() else {
            return (Status::Error, Vec::new());
        };
        for section in &self.exefs_header.sections {
            if section.name != name {
                continue;
            }
            let (offset, size) = if self.is_proto {
                (
                    u64::from(section.offset),
                    align_up(u64::from(section.size), 0x10),
                )
            } else {
                (
                    u64::from(section.offset)
                        + self.exefs_offset
                        + EXEFS_HEADER_SIZE as u64
                        + self.ncch_offset,
                    u64::from(section.size),
                )
            };
            return match read_exact_at(exefs_file, offset, size as usize) {
                Some(buffer) => (Status::Success, buffer),
                None => (Status::Error, Vec::new()),
            };
        }
        (Status::ErrorNotUsed, Vec::new())
    }
}
