use std::ptr;

use crate::abi::PageTableView;
use crate::page_table::{PageTable, PageType};
use crate::region::{GuestMemory, Region};

#[unsafe(no_mangle)]
pub extern "C" fn emu_mem_create() -> *mut GuestMemory {
    GuestMemory::new().map_or(ptr::null_mut(), |memory| Box::into_raw(Box::new(memory)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_mem_destroy(memory: *mut GuestMemory) {
    if !memory.is_null() {
        drop(unsafe { Box::from_raw(memory) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_mem_region_ptr(memory: *const GuestMemory, region: u32) -> *mut u8 {
    let (Some(memory), Some(region)) = (unsafe { memory.as_ref() }, Region::from_raw(region)) else {
        return ptr::null_mut();
    };
    memory.region_ptr(region).unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_mem_region_size(region: u32) -> usize {
    Region::from_raw(region).map_or(0, Region::size)
}

#[unsafe(no_mangle)]
pub extern "C" fn emu_mem_pt_create() -> *mut PageTable {
    PageTable::new().map_or(ptr::null_mut(), |page_table| Box::into_raw(Box::new(page_table)))
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_mem_pt_destroy(page_table: *mut PageTable) {
    if !page_table.is_null() {
        drop(unsafe { Box::from_raw(page_table) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_mem_pt_view(page_table: *const PageTable) -> PageTableView {
    unsafe { page_table.as_ref() }.map_or(PageTableView::null(), PageTable::view)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_mem_pt_clear(page_table: *const PageTable) {
    if let Some(page_table) = unsafe { page_table.as_ref() } {
        page_table.clear();
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_mem_pt_set_entry(
    page_table: *const PageTable,
    page: u32,
    pointer: *mut u8,
) {
    if let Some(page_table) = unsafe { page_table.as_ref() } {
        page_table.set_entry(page as usize, pointer);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_mem_pt_set_attribute(
    page_table: *const PageTable,
    page: u32,
    page_type: u8,
) {
    if let (Some(page_table), Some(page_type)) =
        (unsafe { page_table.as_ref() }, PageType::from_raw(page_type))
    {
        page_table.set_attribute(page as usize, page_type);
    }
}
