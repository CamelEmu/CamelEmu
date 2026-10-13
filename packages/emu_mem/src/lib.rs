//! Creates and owns the memory of the emulated 3DS, which is the physical memory regions and the
//! page tables of the processes. The core borrows the pointers through the C ABI in `ffi`, and the
//! CPU in `emu_cpu` reads and writes the memory through a `PageTableView`.

pub mod abi;
#[cfg(feature = "ffi")]
mod ffi;
mod mapping;
pub mod page_table;
pub mod region;
