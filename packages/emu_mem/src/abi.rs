use std::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

use crate::page_table::{NUM_ENTRIES, PAGE_BITS, PageType};

/// The arrays of a page table, which the core and the CPU index without calling into this crate.
/// It is `EmuMemPageTableView` of `emu_mem.h`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PageTableView {
    pub entries: *mut AtomicPtr<u8>,
    pub attributes: *mut AtomicU8,
}

unsafe impl Send for PageTableView {}
unsafe impl Sync for PageTableView {}

impl PageTableView {
    pub const fn null() -> Self {
        Self {
            entries: std::ptr::null_mut(),
            attributes: std::ptr::null_mut(),
        }
    }

    pub fn is_null(&self) -> bool {
        self.entries.is_null() || self.attributes.is_null()
    }

    /// Returns the host pointer of the page that contains `vaddr`, or null if the page has none.
    ///
    /// # Safety
    /// The view must not be null and its page table must still be alive.
    #[inline(always)]
    pub unsafe fn page_pointer(&self, vaddr: u32) -> *mut u8 {
        let page = (vaddr >> PAGE_BITS) as usize;
        debug_assert!(page < NUM_ENTRIES);
        unsafe { (*self.entries.add(page)).load(Ordering::Relaxed) }
    }

    /// # Safety
    /// The view must not be null and its page table must still be alive.
    #[inline(always)]
    pub unsafe fn page_type(&self, vaddr: u32) -> PageType {
        let page = (vaddr >> PAGE_BITS) as usize;
        debug_assert!(page < NUM_ENTRIES);
        let raw = unsafe { (*self.attributes.add(page)).load(Ordering::Relaxed) };
        PageType::from_raw(raw).unwrap_or(PageType::Unmapped)
    }
}
