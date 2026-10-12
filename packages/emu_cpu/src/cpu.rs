use std::ptr::{self, NonNull};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, AtomicU64, Ordering};

use emu_mem::abi::PageTableView;
use emu_mem::page_table::PAGE_MASK;

use crate::exec::BlockCache;
use crate::fpenv::{self, HostEnvironment};
use crate::host::{Exception, HostCallbacks};
use crate::instruction::Step;
use crate::monitor::ExclusiveMonitor;
use crate::state::{CpuState, psr};

/// What the core asks while it may be running, possibly from another thread. The dispatcher
/// applies it between two blocks.
#[derive(Default)]
pub struct Requests {
    pub clear_cache: bool,
    pub invalidate: Vec<(u32, u32)>,
    pub page_table: Option<(PageTableView, usize)>,
}

/// The part of a core that the core may touch while the CPU runs.
#[derive(Default)]
pub struct Shared {
    pub halt: AtomicBool,
    pub executing: AtomicBool,
    pub exclusive_local: AtomicBool,
    pub has_requests: AtomicBool,
    pub requests: Mutex<Requests>,
}

impl Shared {
    pub fn request(&self, update: impl FnOnce(&mut Requests)) {
        let mut requests = self.requests.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        update(&mut requests);
        self.has_requests.store(true, Ordering::Release);
        self.halt_if_executing();
    }

    pub fn halt_if_executing(&self) {
        if self.executing.load(Ordering::Acquire) {
            self.halt.store(true, Ordering::Release);
        }
    }
}

/// The ticks of the slice: how many the core allowed at the last synchronisation and how many
/// are left.
#[derive(Default)]
pub struct Clock {
    budget: i64,
    remaining: i64,
}

/// A core while it runs on the thread of the emulator.
pub struct Cpu {
    /// The registers live outside of this struct, since the core reads and writes them while the
    /// CPU calls into it.
    state: NonNull<CpuState>,
    shared: NonNull<Shared>,
    monitor: NonNull<ExclusiveMonitor>,
    host: HostCallbacks,
    pub core_id: usize,
    pub page_table: PageTableView,
    pub page_table_key: usize,
    pub clock: Clock,
    host_environment: Option<HostEnvironment>,
    pub cache: BlockCache,
}

impl Cpu {
    pub fn new(
        state: NonNull<CpuState>,
        shared: NonNull<Shared>,
        monitor: NonNull<ExclusiveMonitor>,
        host: HostCallbacks,
        core_id: usize,
    ) -> Self {
        Self {
            state,
            shared,
            monitor,
            host,
            core_id,
            page_table: PageTableView::null(),
            page_table_key: 0,
            clock: Clock::default(),
            host_environment: None,
            cache: BlockCache::default(),
        }
    }

    #[inline(always)]
    pub fn state(&mut self) -> &mut CpuState {
        unsafe { self.state.as_mut() }
    }

    #[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
    pub fn state_ptr(&self) -> *mut CpuState {
        self.state.as_ptr()
    }

    /// The ticks left, which the native code takes the cycles from.
    #[cfg_attr(not(target_arch = "aarch64"), allow(dead_code))]
    pub fn remaining_ptr(&mut self) -> *mut i64 {
        &mut self.clock.remaining
    }

    pub fn shared(&self) -> &Shared {
        unsafe { self.shared.as_ref() }
    }

    fn monitor<'a>(&self) -> &'a ExclusiveMonitor {
        unsafe { self.monitor.as_ref() }
    }

    // Registers

    /// The address of the instruction that runs.
    #[inline(always)]
    pub fn pc(&mut self) -> u32 {
        self.state().regs[15]
    }

    /// Reads a register as an operand, where the PC reads as the address of the instruction plus 8
    /// in ARM state and plus 4 in Thumb state.
    #[inline(always)]
    pub fn reg(&mut self, n: u32) -> u32 {
        let state = self.state();
        if n == 15 {
            state.regs[15].wrapping_add(if state.thumb() { 4 } else { 8 })
        } else {
            state.regs[n as usize]
        }
    }

    /// Writes a register other than the PC.
    #[inline(always)]
    pub fn set_reg(&mut self, n: u32, value: u32) {
        debug_assert!(n != 15);
        self.state().regs[n as usize] = value;
    }

    /// Writes a branch target without changing the instruction set.
    pub fn branch_write_pc(&mut self, value: u32) -> Step {
        let state = self.state();
        state.regs[15] = value & if state.thumb() { !1 } else { !3 };
        Step::Branch
    }

    /// Writes a branch target whose bit 0 selects the instruction set.
    pub fn bx_write_pc(&mut self, value: u32) -> Step {
        let state = self.state();
        if value & 1 != 0 {
            state.cpsr |= psr::T;
            state.regs[15] = value & !1;
        } else {
            state.cpsr &= !psr::T;
            state.regs[15] = value & !3;
        }
        Step::Branch
    }

    /// Writes the result of a data-processing instruction to the PC, which interworks in ARM
    /// state as dynarmic does with the architecture version of the core.
    pub fn alu_write_pc(&mut self, value: u32) -> Step {
        if self.state().thumb() {
            self.branch_write_pc(value)
        } else {
            self.bx_write_pc(value)
        }
    }

    /// Writes the result of a data-processing instruction.
    pub fn write_alu(&mut self, d: u32, value: u32) -> Step {
        if d == 15 {
            self.alu_write_pc(value)
        } else {
            self.set_reg(d, value);
            Step::Continue
        }
    }

    /// Writes a loaded value.
    pub fn write_load(&mut self, t: u32, value: u32) -> Step {
        if t == 15 {
            self.bx_write_pc(value)
        } else {
            self.set_reg(t, value);
            Step::Continue
        }
    }

    // Calls into the core

    /// Calls the core with the floating-point environment of the host.
    pub fn with_host<R>(&mut self, call: impl FnOnce(&HostCallbacks) -> R) -> R {
        let in_guest = self.in_guest_environment();
        self.leave_guest_environment();
        let result = call(&self.host);
        if in_guest {
            self.enter_guest_environment();
        }
        result
    }

    /// Calls the core after it has been given the ticks run so far, and takes the ticks it allows
    /// afterwards, since the call may schedule events or end the slice.
    pub fn with_host_synced<R>(&mut self, call: impl FnOnce(&HostCallbacks) -> R) -> R {
        let used = self.clock.budget - self.clock.remaining;
        let (result, remaining) = self.with_host(|host| unsafe {
            if used > 0 {
                (host.add_ticks)(host.user, used as u64);
            }
            let result = call(host);
            (result, (host.get_ticks_remaining)(host.user))
        });
        self.clock.budget = remaining as i64;
        self.clock.remaining = remaining as i64;
        result
    }

    pub fn enter_guest_environment(&mut self) {
        if self.host_environment.is_none() {
            let fpscr = self.state().fpscr;
            self.host_environment = Some(fpenv::enter_guest(fpscr));
        }
    }

    pub fn leave_guest_environment(&mut self) {
        if let Some(host) = self.host_environment.take() {
            let mut fpscr = self.state().fpscr;
            fpenv::leave_guest(host, &mut fpscr);
            self.state().fpscr = fpscr;
        }
    }

    pub fn in_guest_environment(&self) -> bool {
        self.host_environment.is_some()
    }

    pub fn call_svc(&mut self, swi: u32) {
        self.with_host_synced(|host| unsafe { (host.call_svc)(host.user, swi) });
    }

    /// Reports an exception to the core with the PC at `pc`.
    pub fn raise(&mut self, exception: Exception, pc: u32) -> Step {
        self.state().regs[15] = pc;
        self.with_host_synced(|host| unsafe {
            (host.exception_raised)(host.user, pc, exception as u32)
        });
        Step::Exit
    }

    // Clock

    pub fn start_clock(&mut self) {
        let remaining = self.with_host(|host| unsafe { (host.get_ticks_remaining)(host.user) });
        self.clock.budget = remaining as i64;
        self.clock.remaining = remaining as i64;
    }

    #[inline(always)]
    pub fn charge(&mut self, cycles: u32) {
        self.clock.remaining -= cycles as i64;
    }

    pub fn ticks_left(&self) -> bool {
        self.clock.remaining > 0
    }

    pub fn stop_clock(&mut self) {
        let used = self.clock.budget - self.clock.remaining;
        if used > 0 {
            self.with_host(|host| unsafe { (host.add_ticks)(host.user, used as u64) });
        }
        self.clock.budget = self.clock.remaining;
    }

    // Memory

    #[inline(always)]
    fn host_pointer(&self, vaddr: u32) -> *mut u8 {
        if self.page_table.is_null() {
            return ptr::null_mut();
        }
        let page = unsafe { self.page_table.page_pointer(vaddr) };
        if page.is_null() {
            return page;
        }
        unsafe { page.add((vaddr & PAGE_MASK) as usize) }
    }

    fn swap16(&mut self, value: u16) -> u16 {
        if self.state().big_endian() { value.swap_bytes() } else { value }
    }

    fn swap32(&mut self, value: u32) -> u32 {
        if self.state().big_endian() { value.swap_bytes() } else { value }
    }

    fn swap64(&mut self, value: u64) -> u64 {
        if self.state().big_endian() { value.swap_bytes() } else { value }
    }

    /// Reads the instruction at `vaddr`, which never depends on the endianness of the data.
    pub fn fetch32(&mut self, vaddr: u32) -> u32 {
        let pointer = self.host_pointer(vaddr);
        if pointer.is_null() {
            return self.with_host(|host| unsafe { (host.read32)(host.user, vaddr) });
        }
        u32::from_le(unsafe { ptr::read_unaligned(pointer.cast()) })
    }

    pub fn fetch16(&mut self, vaddr: u32) -> u16 {
        let pointer = self.host_pointer(vaddr);
        if pointer.is_null() {
            return self.with_host(|host| unsafe { (host.read16)(host.user, vaddr) });
        }
        u16::from_le(unsafe { ptr::read_unaligned(pointer.cast()) })
    }

    pub fn read8(&mut self, vaddr: u32) -> u8 {
        let pointer = self.host_pointer(vaddr);
        if pointer.is_null() {
            return self.with_host(|host| unsafe { (host.read8)(host.user, vaddr) });
        }
        unsafe { pointer.read() }
    }

    pub fn read16(&mut self, vaddr: u32) -> u16 {
        let pointer = self.host_pointer(vaddr);
        let value = if pointer.is_null() {
            self.with_host(|host| unsafe { (host.read16)(host.user, vaddr) })
        } else {
            u16::from_le(unsafe { ptr::read_unaligned(pointer.cast()) })
        };
        self.swap16(value)
    }

    pub fn read32(&mut self, vaddr: u32) -> u32 {
        let pointer = self.host_pointer(vaddr);
        let value = if pointer.is_null() {
            self.with_host(|host| unsafe { (host.read32)(host.user, vaddr) })
        } else {
            u32::from_le(unsafe { ptr::read_unaligned(pointer.cast()) })
        };
        self.swap32(value)
    }

    pub fn read64(&mut self, vaddr: u32) -> u64 {
        let pointer = self.host_pointer(vaddr);
        let value = if pointer.is_null() {
            self.with_host(|host| unsafe { (host.read64)(host.user, vaddr) })
        } else {
            u64::from_le(unsafe { ptr::read_unaligned(pointer.cast()) })
        };
        self.swap64(value)
    }

    pub fn write8(&mut self, vaddr: u32, value: u8) {
        let pointer = self.host_pointer(vaddr);
        if pointer.is_null() {
            self.with_host(|host| unsafe { (host.write8)(host.user, vaddr, value) });
            return;
        }
        unsafe { pointer.write(value) };
    }

    pub fn write16(&mut self, vaddr: u32, value: u16) {
        let value = self.swap16(value);
        let pointer = self.host_pointer(vaddr);
        if pointer.is_null() {
            self.with_host(|host| unsafe { (host.write16)(host.user, vaddr, value) });
            return;
        }
        unsafe { ptr::write_unaligned(pointer.cast(), value.to_le()) };
    }

    pub fn write32(&mut self, vaddr: u32, value: u32) {
        let value = self.swap32(value);
        let pointer = self.host_pointer(vaddr);
        if pointer.is_null() {
            self.with_host(|host| unsafe { (host.write32)(host.user, vaddr, value) });
            return;
        }
        unsafe { ptr::write_unaligned(pointer.cast(), value.to_le()) };
    }

    // Exclusive accesses

    pub fn clear_exclusive(&self) {
        self.shared().exclusive_local.store(false, Ordering::Relaxed);
    }

    pub fn exclusive_read(&mut self, vaddr: u32, size: u32) -> u64 {
        self.shared().exclusive_local.store(true, Ordering::Relaxed);
        let monitor = self.monitor();
        let core = self.core_id;
        monitor.read_and_mark(core, vaddr, || match size {
            1 => self.read8(vaddr) as u64,
            2 => self.read16(vaddr) as u64,
            4 => self.read32(vaddr) as u64,
            _ => self.read64(vaddr),
        })
    }

    /// Stores `value` if the core still holds the reservation of `vaddr` and the memory still holds
    /// the value that was read. Returns whether the store succeeded.
    pub fn exclusive_write(&mut self, vaddr: u32, size: u32, value: u64) -> bool {
        if !self.shared().exclusive_local.swap(false, Ordering::Relaxed) {
            return false;
        }
        let monitor = self.monitor();
        let core = self.core_id;
        monitor.do_exclusive(core, vaddr, |expected| {
            self.compare_and_swap(vaddr, size, value, expected)
        })
    }

    fn compare_and_swap(&mut self, vaddr: u32, size: u32, value: u64, expected: u64) -> bool {
        let (value, expected) = match size {
            1 => (value, expected),
            2 => (self.swap16(value as u16) as u64, self.swap16(expected as u16) as u64),
            4 => (self.swap32(value as u32) as u64, self.swap32(expected as u32) as u64),
            _ => (self.swap64(value), self.swap64(expected)),
        };
        let pointer = self.host_pointer(vaddr);
        if pointer.is_null() || pointer as usize % size as usize != 0 {
            return self.with_host(|host| unsafe {
                match size {
                    1 => (host.write_exclusive8)(host.user, vaddr, value as u8, expected as u8),
                    2 => (host.write_exclusive16)(host.user, vaddr, value as u16, expected as u16),
                    4 => (host.write_exclusive32)(host.user, vaddr, value as u32, expected as u32),
                    _ => (host.write_exclusive64)(host.user, vaddr, value, expected),
                }
            });
        }
        let (strong, weak) = (Ordering::SeqCst, Ordering::SeqCst);
        unsafe {
            match size {
                1 => AtomicU8::from_ptr(pointer)
                    .compare_exchange(expected as u8, value as u8, strong, weak)
                    .is_ok(),
                2 => AtomicU16::from_ptr(pointer.cast())
                    .compare_exchange((expected as u16).to_le(), (value as u16).to_le(), strong, weak)
                    .is_ok(),
                4 => AtomicU32::from_ptr(pointer.cast())
                    .compare_exchange((expected as u32).to_le(), (value as u32).to_le(), strong, weak)
                    .is_ok(),
                _ => AtomicU64::from_ptr(pointer.cast())
                    .compare_exchange(expected.to_le(), value.to_le(), strong, weak)
                    .is_ok(),
            }
        }
    }
}
