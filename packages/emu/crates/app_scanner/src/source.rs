use std::fs::File;
use std::os::unix::fs::FileExt;

use emu_storage::OpenMode;

pub trait Source {
    fn size(&self) -> u64;

    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> usize;

    fn is_crypto(&self) -> bool {
        false
    }
}

pub struct PlainSource {
    file: File,
    size: u64,
}

impl PlainSource {
    pub fn open(path: &str) -> Option<Self> {
        let file = emu_storage::open_file(
            path,
            OpenMode {
                read: true,
                write: false,
                append: false,
                truncate: false,
            },
        )?;
        let size = file.metadata().ok()?.len();
        Some(Self { file, size })
    }
}

impl Source for PlainSource {
    fn size(&self) -> u64 {
        self.size
    }

    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> usize {
        read_fully(&self.file, offset, buffer)
    }
}

pub fn read_fully(file: &File, offset: u64, buffer: &mut [u8]) -> usize {
    let mut filled = 0;
    while filled < buffer.len() {
        match file.read_at(&mut buffer[filled..], offset + filled as u64) {
            Ok(0) | Err(_) => break,
            Ok(read) => filled += read,
        }
    }
    filled
}

pub fn read_exact_at(source: &dyn Source, offset: u64, length: usize) -> Option<Vec<u8>> {
    if offset.checked_add(length as u64)? > source.size() {
        return None;
    }
    let mut buffer = vec![0; length];
    (source.read_at(offset, &mut buffer) == length).then_some(buffer)
}

pub fn read_whole_file(path: &str) -> Option<Vec<u8>> {
    let source = PlainSource::open(path)?;
    read_exact_at(&source, 0, usize::try_from(source.size()).ok()?)
}
