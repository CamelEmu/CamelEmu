use std::mem::size_of;
use std::slice;
use std::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

use crate::abi::PageTableView;
use crate::mapping::Mapping;

pub const PAGE_BITS: u32 = 12;
pub const PAGE_SIZE: u32 = 1 << PAGE_BITS;
pub const PAGE_MASK: u32 = PAGE_SIZE - 1;
pub const NUM_ENTRIES: usize = 1 << (32 - PAGE_BITS);

/// The kind of a page, with the values of `Memory::PageType` of the core.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageType {
    Unmapped = 0,
    Memory = 1,
    RasterizerCachedMemory = 2,
}

impl PageType {
    pub fn from_raw(raw: u8) -> Option<Self> {
        match raw {
            0 => Some(Self::Unmapped),
            1 => Some(Self::Memory),
            2 => Some(Self::RasterizerCachedMemory),
            _ => None,
        }
    }
}

/// The page table of a process. Every entry is null and every page is unmapped at the start, since
/// the zeroed pages of the mapping are both.
pub struct PageTable {
    entries: Mapping,
    attributes: Mapping,
}

impl PageTable {
    pub fn new() -> Option<Self> {
        Some(Self {
            entries: Mapping::new(NUM_ENTRIES * size_of::<AtomicPtr<u8>>())?,
            attributes: Mapping::new(NUM_ENTRIES * size_of::<AtomicU8>())?,
        })
    }

    fn entries(&self) -> &[AtomicPtr<u8>] {
        unsafe { slice::from_raw_parts(self.entries.as_ptr().cast(), NUM_ENTRIES) }
    }

    fn attributes(&self) -> &[AtomicU8] {
        unsafe { slice::from_raw_parts(self.attributes.as_ptr().cast(), NUM_ENTRIES) }
    }

    pub fn set_entry(&self, page: usize, pointer: *mut u8) {
        if let Some(entry) = self.entries().get(page) {
            entry.store(pointer, Ordering::Relaxed);
        }
    }

    pub fn set_attribute(&self, page: usize, page_type: PageType) {
        if let Some(attribute) = self.attributes().get(page) {
            attribute.store(page_type as u8, Ordering::Relaxed);
        }
    }

    pub fn clear(&self) {
        for entry in self.entries() {
            entry.store(std::ptr::null_mut(), Ordering::Relaxed);
        }
        for attribute in self.attributes() {
            attribute.store(PageType::Unmapped as u8, Ordering::Relaxed);
        }
    }

    pub fn view(&self) -> PageTableView {
        PageTableView {
            entries: self.entries.as_ptr().cast(),
            attributes: self.attributes.as_ptr().cast(),
        }
    }
}
