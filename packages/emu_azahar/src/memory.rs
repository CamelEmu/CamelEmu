use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering};

use emu_mem::abi::PageTableView;
use emu_mem::page_table::{PAGE_MASK, PageTable, PageType};
use emu_mem::region::{GuestMemory, Region};
use emu_pica::Gpu;
use emu_pica::memory::PhysicalMemory;

pub use emu_pica::FlushMode;

pub const IO_AREA_PADDR: u32 = 0x1010_0000;
pub const VRAM_PADDR: u32 = 0x1800_0000;
pub const VRAM_SIZE: u32 = 0x0060_0000;
pub const N3DS_EXTRA_RAM_PADDR: u32 = 0x1F00_0000;
pub const N3DS_EXTRA_RAM_SIZE: u32 = 0x0040_0000;
pub const DSP_RAM_PADDR: u32 = 0x1FF0_0000;
pub const DSP_RAM_SIZE: u32 = 0x0008_0000;
pub const FCRAM_PADDR: u32 = 0x2000_0000;
pub const FCRAM_N3DS_SIZE: u32 = 0x1000_0000;

pub const LINEAR_HEAP_VADDR: u32 = 0x1400_0000;
pub const LINEAR_HEAP_SIZE: u32 = 0x0800_0000;
pub const LINEAR_HEAP_VADDR_END: u32 = LINEAR_HEAP_VADDR + LINEAR_HEAP_SIZE;
pub const VRAM_VADDR: u32 = 0x1F00_0000;
pub const VRAM_VADDR_END: u32 = VRAM_VADDR + VRAM_SIZE;
pub const NEW_LINEAR_HEAP_VADDR: u32 = 0x3000_0000;
pub const NEW_LINEAR_HEAP_SIZE: u32 = 0x1000_0000;
pub const NEW_LINEAR_HEAP_VADDR_END: u32 = NEW_LINEAR_HEAP_VADDR + NEW_LINEAR_HEAP_SIZE;

type PcSource = Rc<Cell<Option<*const u32>>>;

fn read_pc(pc: &PcSource) -> u32 {
    pc.get().map_or(0, |pc| unsafe { pc.read_volatile() })
}

pub struct Memory {
    guest: GuestMemory,
    page_table: PageTable,
    view: PageTableView,
    gpu: Option<Gpu>,
    pc: PcSource,
}

/// The physical memory as the PICA reads and writes it, which `MemorySystem::GetPhysicalPointer`
/// returns in the core.
struct PhysicalMemoryView {
    fcram: *mut u8,
    vram: *mut u8,
    dsp: Option<*mut u8>,
    n3ds_extra_ram: *mut u8,
    pc: PcSource,
}

impl PhysicalMemory for PhysicalMemoryView {
    fn get_physical_pointer(&self, address: u32) -> *mut u8 {
        let memory_areas = [
            (VRAM_PADDR, VRAM_SIZE, Some(self.vram)),
            (DSP_RAM_PADDR, DSP_RAM_SIZE, self.dsp),
            (FCRAM_PADDR, FCRAM_N3DS_SIZE, Some(self.fcram)),
            (
                N3DS_EXTRA_RAM_PADDR,
                N3DS_EXTRA_RAM_SIZE,
                Some(self.n3ds_extra_ram),
            ),
        ];

        // Note: the region end check is inclusive because the user can pass in an address that
        // represents an open right bound
        let area = memory_areas
            .iter()
            .find(|(start, size, _)| address >= *start && address <= start + size);

        let Some(&(start, _, target_mem)) = area else {
            log::error!(
                target: "HW_Memory",
                "Unknown GetPhysicalPointer @ {:#08X} at PC {:#08X}",
                address,
                read_pc(&self.pc)
            );
            return std::ptr::null_mut();
        };

        let offset_into_region = address - start;
        target_mem.map_or(std::ptr::null_mut(), |target_mem| unsafe {
            target_mem.add(offset_into_region as usize)
        })
    }
}

macro_rules! read_fn {
    ($name:ident, $ty:ty) => {
        pub fn $name(&mut self, vaddr: u32) -> $ty {
            let page_pointer = unsafe { self.view.page_pointer(vaddr) };
            if !page_pointer.is_null() {
                // NOTE: Avoid adding any extra logic to this fast-path block
                return unsafe {
                    page_pointer
                        .add((vaddr & PAGE_MASK) as usize)
                        .cast::<$ty>()
                        .read_unaligned()
                };
            }

            // Custom Luma3ds mapping
            // Is there a more efficient way to do this?
            if vaddr & (1 << 31) != 0 {
                let paddr = vaddr & !(1 << 31);
                if paddr & 0xF000_0000 == FCRAM_PADDR {
                    // Check FCRAM region
                    return unsafe {
                        self.fcram_pointer(paddr - FCRAM_PADDR)
                            .cast::<$ty>()
                            .read_unaligned()
                    };
                } else if paddr & 0xF000_0000 == 0x1000_0000 && paddr >= IO_AREA_PADDR {
                    // Check MMIO region
                    return self.read_gpu_reg(paddr - IO_AREA_PADDR + 0x1EC0_0000) as $ty;
                }
            }

            match unsafe { self.view.page_type(vaddr) } {
                PageType::Unmapped => {
                    log::error!(
                        target: "HW_Memory",
                        "unmapped Read{} @ 0x{:08X} at PC 0x{:08X}",
                        size_of::<$ty>() * 8,
                        vaddr,
                        self.pc()
                    );
                    0
                }
                PageType::Memory => {
                    panic!("Mapped memory page without a pointer @ {vaddr:08X}")
                }
                PageType::RasterizerCachedMemory => {
                    self.rasterizer_flush_virtual_region(
                        vaddr,
                        size_of::<$ty>() as u32,
                        FlushMode::Flush,
                    );
                    unsafe {
                        self.pointer_for_rasterizer_cache(vaddr)
                            .cast::<$ty>()
                            .read_unaligned()
                    }
                }
            }
        }
    };
}

macro_rules! write_fn {
    ($name:ident, $ty:ty) => {
        pub fn $name(&mut self, vaddr: u32, data: $ty) {
            let page_pointer = unsafe { self.view.page_pointer(vaddr) };
            if !page_pointer.is_null() {
                // NOTE: Avoid adding any extra logic to this fast-path block
                unsafe {
                    page_pointer
                        .add((vaddr & PAGE_MASK) as usize)
                        .cast::<$ty>()
                        .write_unaligned(data)
                };
                return;
            }

            // Custom Luma3ds mapping
            // Is there a more efficient way to do this?
            if vaddr & (1 << 31) != 0 {
                let paddr = vaddr & !(1 << 31);
                if paddr & 0xF000_0000 == FCRAM_PADDR {
                    // Check FCRAM region
                    unsafe {
                        self.fcram_pointer(paddr - FCRAM_PADDR)
                            .cast::<$ty>()
                            .write_unaligned(data)
                    };
                    return;
                } else if paddr & 0xF000_0000 == 0x1000_0000 && paddr >= IO_AREA_PADDR {
                    // Check MMIO region
                    assert_eq!(size_of::<$ty>(), size_of::<u32>());
                    self.write_gpu_reg(paddr - IO_AREA_PADDR + 0x1EC0_0000, data as u32);
                    return;
                }
            }

            match unsafe { self.view.page_type(vaddr) } {
                PageType::Unmapped => {
                    log::error!(
                        target: "HW_Memory",
                        "unmapped Write{} 0x{:08X} @ 0x{:08X} at PC 0x{:08X}",
                        size_of::<$ty>() * 8,
                        data as u32,
                        vaddr,
                        self.pc()
                    );
                }
                PageType::Memory => {
                    panic!("Mapped memory page without a pointer @ {vaddr:08X}")
                }
                PageType::RasterizerCachedMemory => {
                    self.rasterizer_flush_virtual_region(
                        vaddr,
                        size_of::<$ty>() as u32,
                        FlushMode::Invalidate,
                    );
                    unsafe {
                        self.pointer_for_rasterizer_cache(vaddr)
                            .cast::<$ty>()
                            .write_unaligned(data)
                    };
                }
            }
        }
    };
}

macro_rules! write_exclusive_fn {
    ($name:ident, $ty:ty, $atomic:ty) => {
        pub fn $name(&mut self, vaddr: u32, data: $ty, expected: $ty) -> bool {
            let page_pointer = unsafe { self.view.page_pointer(vaddr) };
            if !page_pointer.is_null() {
                let atomic = unsafe {
                    &*page_pointer
                        .add((vaddr & PAGE_MASK) as usize)
                        .cast::<$atomic>()
                };
                return atomic
                    .compare_exchange(expected, data, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok();
            }

            match unsafe { self.view.page_type(vaddr) } {
                PageType::Unmapped => {
                    log::error!(
                        target: "HW_Memory",
                        "unmapped Write{} 0x{:08X} @ 0x{:08X} at PC 0x{:08X}",
                        size_of::<$ty>() * 8,
                        data as u32,
                        vaddr,
                        self.pc()
                    );
                    true
                }
                PageType::Memory => {
                    panic!("Mapped memory page without a pointer @ {vaddr:08X}")
                }
                PageType::RasterizerCachedMemory => {
                    self.rasterizer_flush_virtual_region(
                        vaddr,
                        size_of::<$ty>() as u32,
                        FlushMode::Invalidate,
                    );
                    let atomic =
                        unsafe { &*self.pointer_for_rasterizer_cache(vaddr).cast::<$atomic>() };
                    atomic
                        .compare_exchange(expected, data, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                }
            }
        }
    };
}

impl Memory {
    pub fn new() -> Option<Self> {
        let guest = GuestMemory::new()?;
        let page_table = PageTable::new()?;
        let view = page_table.view();
        Some(Self {
            guest,
            page_table,
            view,
            gpu: None,
            pc: PcSource::default(),
        })
    }

    /// The physical memory for the GPU, which stays valid while the memory lives.
    pub fn physical_memory(&self) -> Rc<dyn PhysicalMemory> {
        Rc::new(PhysicalMemoryView {
            fcram: self.region_pointer(Region::Fcram),
            vram: self.region_pointer(Region::Vram),
            dsp: self.guest.region_ptr(Region::Dsp),
            n3ds_extra_ram: self.region_pointer(Region::N3ds),
            pc: self.pc.clone(),
        })
    }

    pub fn set_gpu(&mut self, gpu: Gpu) {
        self.gpu = Some(gpu);
    }

    pub fn page_table_view(&self) -> PageTableView {
        self.view
    }

    pub fn page_table_key(&self) -> *const PageTable {
        &self.page_table
    }

    pub fn gpu(&mut self) -> Option<&mut Gpu> {
        self.gpu.as_mut()
    }

    pub fn set_pc_source(&mut self, pc: *const u32) {
        self.pc.set(Some(pc));
    }

    fn pc(&self) -> u32 {
        read_pc(&self.pc)
    }

    read_fn!(read8, u8);
    read_fn!(read16, u16);
    read_fn!(read32, u32);
    read_fn!(read64, u64);

    write_fn!(write8, u8);
    write_fn!(write16, u16);
    write_fn!(write32, u32);
    write_fn!(write64, u64);

    write_exclusive_fn!(write_exclusive8, u8, AtomicU8);
    write_exclusive_fn!(write_exclusive16, u16, AtomicU16);
    write_exclusive_fn!(write_exclusive32, u32, AtomicU32);
    write_exclusive_fn!(write_exclusive64, u64, AtomicU64);

    fn region_pointer(&self, region: Region) -> *mut u8 {
        self.guest
            .region_ptr(region)
            .expect("the region is owned by the memory")
    }

    fn fcram_pointer(&self, offset: u32) -> *mut u8 {
        assert!(offset <= FCRAM_N3DS_SIZE);
        unsafe { self.region_pointer(Region::Fcram).add(offset as usize) }
    }

    fn pointer_for_rasterizer_cache(&self, addr: u32) -> *mut u8 {
        if (LINEAR_HEAP_VADDR..LINEAR_HEAP_VADDR_END).contains(&addr) {
            return unsafe {
                self.region_pointer(Region::Fcram)
                    .add((addr - LINEAR_HEAP_VADDR) as usize)
            };
        }
        if (NEW_LINEAR_HEAP_VADDR..NEW_LINEAR_HEAP_VADDR_END).contains(&addr) {
            return unsafe {
                self.region_pointer(Region::Fcram)
                    .add((addr - NEW_LINEAR_HEAP_VADDR) as usize)
            };
        }
        if (VRAM_VADDR..VRAM_VADDR_END).contains(&addr) {
            return unsafe {
                self.region_pointer(Region::Vram)
                    .add((addr - VRAM_VADDR) as usize)
            };
        }
        unreachable!()
    }

    fn read_gpu_reg(&self, vaddr: u32) -> u32 {
        self.gpu.as_ref().map_or(0, |gpu| gpu.read_reg(vaddr))
    }

    fn write_gpu_reg(&mut self, vaddr: u32, data: u32) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.write_reg(vaddr, data);
        }
    }

    pub fn rasterizer_flush_virtual_region(&mut self, start: u32, size: u32, mode: FlushMode) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.rasterizer_flush_virtual_region(start, size, mode);
        }
    }
}
