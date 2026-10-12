use std::collections::HashMap;
use std::sync::atomic::Ordering;

use emu_mem::page_table::PAGE_MASK;

use crate::cpu::Cpu;
use crate::decoder;
use crate::instruction::{Instruction, Step};
use crate::jit;

/// The most instructions that a block holds.
const MAX_BLOCK_INSTRUCTIONS: usize = 64;

/// Where a block starts: the page table it was read through, its address and its instruction set.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Location {
    pub page_table: usize,
    pub pc: u32,
    pub thumb: bool,
}

/// An instruction of a block, decoded once when the block is translated.
#[derive(Clone, Copy)]
pub struct Decoded {
    pub instruction: &'static Instruction,
    pub raw: u32,
    pub address: u32,
    pub size: u32,
    /// The condition of an ARM instruction, or 0b1110 for the instructions that check their own.
    pub condition: u32,
    pub cycles: u32,
}

/// A run of instructions that ends with a branch, translated together.
pub struct Block {
    pub location: Location,
    pub end: u32,
    pub instructions: Vec<Decoded>,
    pub code: Option<jit::Code>,
}

const FAST_LOOKUP_SIZE: usize = 0x1000;

/// The translated blocks of a core, with a direct-mapped table in front of the map, as dynarmic
/// has for the dispatcher.
pub struct BlockCache {
    blocks: HashMap<Location, Box<Block>>,
    fast: Vec<Option<(Location, *const Block)>>,
    jit: jit::Jit,
}

impl Default for BlockCache {
    fn default() -> Self {
        Self {
            blocks: HashMap::new(),
            fast: vec![None; FAST_LOOKUP_SIZE],
            jit: jit::Jit::default(),
        }
    }
}

fn fast_index(location: &Location) -> usize {
    ((location.pc >> 1) ^ (location.pc >> 13) ^ location.page_table as u32 >> 4) as usize
        & (FAST_LOOKUP_SIZE - 1)
}

impl BlockCache {
    pub fn clear(&mut self) {
        self.blocks.clear();
        self.fast.fill(None);
        self.jit.reset();
    }

    /// Drops the blocks that hold any instruction in `[start, start + length)`.
    pub fn invalidate(&mut self, start: u32, length: u32) {
        let end = start as u64 + length as u64;
        self.blocks.retain(|_, block| {
            (block.end as u64) <= start as u64 || block.location.pc as u64 >= end
        });
        self.fast.fill(None);
    }

    fn get(&mut self, location: &Location) -> Option<*const Block> {
        let index = fast_index(location);
        if let Some((cached, block)) = self.fast[index] {
            if cached == *location {
                return Some(block);
            }
        }
        let block: *const Block = &**self.blocks.get(location)?;
        self.fast[index] = Some((*location, block));
        Some(block)
    }

    fn insert(&mut self, block: Box<Block>) -> *const Block {
        let location = block.location;
        let pointer: *const Block = &*block;
        self.blocks.insert(location, block);
        self.fast[fast_index(&location)] = Some((location, pointer));
        pointer
    }
}

/// Why `run` returned, which is `EmuCpuExit` of `emu_cpu.h`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// The slice has no ticks left.
    Ticks = 0,
    /// The core asked the CPU to stop.
    Halted = 1,
    /// An instruction raised an exception that the core has to see.
    Exception = 2,
}

fn decode_at(cpu: &mut Cpu, address: u32, thumb: bool) -> Decoded {
    if !thumb {
        let raw = cpu.fetch32(address);
        let instruction = decoder::decode_arm(raw);
        return Decoded {
            instruction,
            raw,
            address,
            size: 4,
            condition: raw >> 28,
            cycles: instruction.cycles.of(raw),
        };
    }
    let first = cpu.fetch16(address);
    let fits_in_page = address & PAGE_MASK != PAGE_MASK - 1;
    if decoder::is_thumb32_prefix(first) && fits_in_page {
        let second = cpu.fetch16(address.wrapping_add(2));
        if let Some(instruction) = decoder::decode_thumb32(first, second) {
            let raw = ((first as u32) << 16) | second as u32;
            return Decoded {
                instruction,
                raw,
                address,
                size: 4,
                condition: 0b1110,
                cycles: instruction.cycles.of(raw),
            };
        }
    }
    let instruction = decoder::decode_thumb16(first);
    Decoded {
        instruction,
        raw: first as u32,
        address,
        size: 2,
        condition: 0b1110,
        cycles: instruction.cycles.of(first as u32),
    }
}

/// Decodes the instructions from `location` up to the first one that leaves the block, the end
/// of the page or the limit of the size.
fn translate(cpu: &mut Cpu, location: Location) -> Block {
    let mut instructions = Vec::new();
    let mut address = location.pc;
    loop {
        let decoded = decode_at(cpu, address, location.thumb);
        instructions.push(decoded);
        address = address.wrapping_add(decoded.size);
        let ends_block = decoded.instruction.kind.ends_block() && decoded.condition >= 0b1110;
        if ends_block || instructions.len() >= MAX_BLOCK_INSTRUCTIONS || address & PAGE_MASK == 0 {
            break;
        }
    }
    Block {
        location,
        end: address,
        instructions,
        code: None,
    }
}

/// Runs one decoded instruction and moves the PC past it unless it branched.
#[inline(always)]
pub fn execute(cpu: &mut Cpu, decoded: &Decoded) -> Step {
    cpu.charge(decoded.cycles);
    cpu.state().regs[15] = decoded.address;
    if !cpu.state().condition_passed(decoded.condition) {
        cpu.state().regs[15] = decoded.address.wrapping_add(decoded.size);
        return Step::Continue;
    }
    let step = (decoded.instruction.execute)(cpu, decoded.raw);
    if step == Step::Continue {
        cpu.state().regs[15] = decoded.address.wrapping_add(decoded.size);
    }
    step
}

fn run_block(cpu: &mut Cpu, block: &Block) -> Step {
    if let Some(code) = &block.code {
        return code.run(cpu);
    }
    for decoded in &block.instructions {
        let step = execute(cpu, decoded);
        if step != Step::Continue {
            return step;
        }
    }
    Step::Continue
}

fn current_location(cpu: &mut Cpu) -> Location {
    let state = cpu.state();
    let (pc, thumb) = (state.regs[15], state.thumb());
    Location {
        page_table: cpu.page_table_key,
        pc,
        thumb,
    }
}

/// Applies what the core asked while the CPU ran: a new page table and the invalidation of code.
pub fn apply_requests(cpu: &mut Cpu) {
    if !cpu.shared().has_requests.swap(false, Ordering::Acquire) {
        return;
    }
    let requests = {
        let mut requests = cpu
            .shared()
            .requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        std::mem::take(&mut *requests)
    };
    if requests.clear_cache {
        cpu.cache.clear();
    }
    for (start, length) in requests.invalidate {
        cpu.cache.invalidate(start, length);
    }
    if let Some((view, key)) = requests.page_table {
        cpu.page_table = view;
        cpu.page_table_key = key;
    }
}

/// Finds the block at `location`, translating it if it is new. It runs between blocks, so it may
/// drop every block when the native code has no space left.
fn block_at(cpu: &mut Cpu, location: Location) -> *const Block {
    if let Some(block) = cpu.cache.get(&location) {
        return block;
    }
    if cpu.cache.jit.is_full() {
        cpu.cache.clear();
    }
    let mut block = Box::new(translate(cpu, location));
    let entries = cpu.page_table.entries as *const u8;
    block.code = cpu.cache.jit.compile(&block, entries);
    cpu.cache.insert(block)
}

/// Runs blocks until the slice has no ticks left, the core asks to stop or an instruction raises
/// an exception. At least one block runs, as with dynarmic.
pub fn run(cpu: &mut Cpu) -> Exit {
    cpu.shared().halt.store(false, Ordering::Release);
    cpu.shared().executing.store(true, Ordering::Release);
    apply_requests(cpu);
    cpu.start_clock();
    cpu.enter_guest_environment();

    let exit = loop {
        let location = current_location(cpu);
        // The cache only changes between blocks, so the block stays alive while it runs.
        let block = unsafe { &*block_at(cpu, location) };
        let step = run_block(cpu, block);
        apply_requests(cpu);
        if step == Step::Exit {
            break Exit::Exception;
        }
        if cpu.shared().halt.swap(false, Ordering::AcqRel) {
            break Exit::Halted;
        }
        if !cpu.ticks_left() {
            break Exit::Ticks;
        }
    };

    cpu.leave_guest_environment();
    cpu.stop_clock();
    cpu.shared().executing.store(false, Ordering::Release);
    exit
}

/// Runs the instruction at the PC.
pub fn step(cpu: &mut Cpu) -> Exit {
    cpu.shared().executing.store(true, Ordering::Release);
    apply_requests(cpu);
    cpu.start_clock();
    cpu.enter_guest_environment();

    let location = current_location(cpu);
    let decoded = decode_at(cpu, location.pc, location.thumb);
    let step = execute(cpu, &decoded);
    apply_requests(cpu);

    cpu.leave_guest_environment();
    cpu.stop_clock();
    cpu.shared().executing.store(false, Ordering::Release);
    cpu.shared().halt.store(false, Ordering::Release);
    if step == Step::Exit { Exit::Exception } else { Exit::Ticks }
}
