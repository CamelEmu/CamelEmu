//! The translation of blocks into native code of the host, which is AArch64.

mod arena;
mod assembler;
mod emit;

use crate::cpu::Cpu;
use crate::exec::Block;
use crate::instruction::Step;

/// The size of the code of a core. When it is full, the cache drops every block.
const ARENA_SIZE: usize = 32 << 20;

/// The native code of a block.
pub struct Code {
    entry: *const u8,
}

impl Code {
    pub fn run(&self, cpu: &mut Cpu) -> Step {
        #[cfg(target_arch = "aarch64")]
        {
            type Entry = unsafe extern "C" fn(*mut Cpu, *mut crate::state::CpuState, *mut i64) -> u32;
            let entry: Entry = unsafe { std::mem::transmute(self.entry) };
            let state = cpu.state_ptr();
            let remaining = cpu.remaining_ptr();
            match unsafe { entry(cpu, state, remaining) } {
                0 => Step::Continue,
                1 => Step::Branch,
                _ => Step::Exit,
            }
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            let _ = (cpu, self.entry);
            unreachable!("native code only exists on AArch64")
        }
    }
}

/// The translator of a core, with the memory of its code.
#[derive(Default)]
pub struct Jit {
    arena: Option<arena::Arena>,
    /// Whether the arena ran out of space, so the cache has to drop everything.
    full: bool,
}

impl Jit {
    pub fn is_full(&self) -> bool {
        self.full
    }

    /// Drops all the code, when the cache drops its blocks.
    pub fn reset(&mut self) {
        if let Some(arena) = &mut self.arena {
            arena.reset();
        }
        self.full = false;
    }

    /// Translates the block, whose memory accesses go through the entries of the page table.
    pub fn compile(&mut self, block: &Block, page_table_entries: *const u8) -> Option<Code> {
        if !cfg!(target_arch = "aarch64") || page_table_entries.is_null() || self.full {
            return None;
        }
        if self.arena.is_none() {
            self.arena = arena::Arena::new(ARENA_SIZE);
        }
        let arena = self.arena.as_mut()?;
        let code = emit::emit(block, page_table_entries as u64)?;
        match arena.write(&code) {
            Some(entry) => Some(Code { entry }),
            None => {
                self.full = true;
                None
            }
        }
    }
}
