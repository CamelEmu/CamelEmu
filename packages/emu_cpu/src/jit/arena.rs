use std::ptr::{self, NonNull};

/// The memory that holds the native code of a core. It is writable while a block is written into
/// it and executable otherwise.
pub struct Arena {
    base: NonNull<u8>,
    size: usize,
    used: usize,
}

// The code is only written and run on the thread of the emulator.
unsafe impl Send for Arena {}

const PAGE_SIZE: usize = 4096;

impl Arena {
    pub fn new(size: usize) -> Option<Self> {
        let base = unsafe {
            libc::mmap(
                ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_EXEC,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if base == libc::MAP_FAILED {
            return None;
        }
        Some(Self {
            base: NonNull::new(base.cast())?,
            size,
            used: 0,
        })
    }

    /// Drops all the code, which the caller no longer runs.
    pub fn reset(&mut self) {
        self.used = 0;
    }

    /// Copies the code into the arena and returns where it starts, or `None` if it is full.
    pub fn write(&mut self, code: &[u32]) -> Option<*const u8> {
        let bytes = code.len() * 4;
        let start = self.used.next_multiple_of(16);
        if start + bytes > self.size {
            return None;
        }
        let first_page = start / PAGE_SIZE * PAGE_SIZE;
        let end = start + bytes;
        let length = end.next_multiple_of(PAGE_SIZE) - first_page;
        unsafe {
            let pages = self.base.as_ptr().add(first_page);
            if libc::mprotect(pages.cast(), length, libc::PROT_READ | libc::PROT_WRITE) != 0 {
                return None;
            }
            let destination = self.base.as_ptr().add(start);
            ptr::copy_nonoverlapping(code.as_ptr().cast::<u8>(), destination, bytes);
            let executable = libc::mprotect(pages.cast(), length, libc::PROT_READ | libc::PROT_EXEC);
            if executable != 0 {
                return None;
            }
            flush_instruction_cache(destination, bytes);
            self.used = end;
            Some(destination)
        }
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.base.as_ptr().cast(), self.size) };
    }
}

/// Makes the instruction cache see the code that was written through the data cache.
#[cfg(target_arch = "aarch64")]
unsafe fn flush_instruction_cache(start: *mut u8, length: usize) {
    use std::arch::asm;

    let ctr: u64;
    unsafe { asm!("mrs {}, ctr_el0", out(reg) ctr, options(nomem, nostack, preserves_flags)) };
    let data_line = 4usize << ((ctr >> 16) & 0xF);
    let instruction_line = 4usize << (ctr & 0xF);
    let begin = start as usize;
    let end = begin + length;

    let mut address = begin & !(data_line - 1);
    while address < end {
        unsafe { asm!("dc cvau, {}", in(reg) address, options(nostack, preserves_flags)) };
        address += data_line;
    }
    unsafe { asm!("dsb ish", options(nostack, preserves_flags)) };

    let mut address = begin & !(instruction_line - 1);
    while address < end {
        unsafe { asm!("ic ivau, {}", in(reg) address, options(nostack, preserves_flags)) };
        address += instruction_line;
    }
    unsafe { asm!("dsb ish", "isb", options(nostack, preserves_flags)) };
}

#[cfg(not(target_arch = "aarch64"))]
unsafe fn flush_instruction_cache(_start: *mut u8, _length: usize) {}
