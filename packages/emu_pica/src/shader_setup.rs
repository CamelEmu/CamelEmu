use std::mem::offset_of;

use emu_pica_types::hash::compute_hash64;
use emu_pica_types::{F24, Vec4};

use crate::aligned::Aligned;
use crate::packed_attribute::PackedAttribute;
use crate::regs::ShaderRegs;

pub const MAX_PROGRAM_CODE_LENGTH: usize = 4096;
pub const MAX_SWIZZLE_DATA_LENGTH: usize = 4096;

pub type ProgramCode = [u32; MAX_PROGRAM_CODE_LENGTH];
pub type SwizzleData = [u32; MAX_SWIZZLE_DATA_LENGTH];

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Uniforms {
    pub f: Aligned<[Vec4<F24>; 96]>,
    pub b: [bool; 16],
    pub i: [Vec4<u8>; 4],
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            f: Aligned([Vec4::default(); 96]),
            b: [false; 16],
            i: [Vec4::default(); 4],
        }
    }
}

impl Uniforms {
    pub fn get_float_uniform_offset(index: u32) -> usize {
        offset_of!(Uniforms, f) + index as usize * size_of::<Vec4<F24>>()
    }

    pub fn get_bool_uniform_offset(index: u32) -> usize {
        offset_of!(Uniforms, b) + index as usize * size_of::<bool>()
    }

    pub fn get_int_uniform_offset(index: u32) -> usize {
        offset_of!(Uniforms, i) + index as usize * size_of::<Vec4<u8>>()
    }
}

/// This structure contains the state information common for all shader units such as uniforms.
/// The geometry shaders has a unique configuration so when enabled it has its own setup.
#[derive(Clone)]
pub struct ShaderSetup {
    pub uniforms: Uniforms,
    pub uniform_queue: PackedAttribute,
    pub program_code: Box<ProgramCode>,
    pub swizzle_data: Box<SwizzleData>,
    pub entry_point: u32,
    pub cached_shader: usize,

    program_code_hash_dirty: bool,
    swizzle_data_hash_dirty: bool,
    program_code_hash: u64,
    swizzle_data_hash: u64,
}

impl Default for ShaderSetup {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaderSetup {
    pub fn new() -> Self {
        Self {
            uniforms: Uniforms::default(),
            uniform_queue: PackedAttribute::default(),
            program_code: Box::new([0; MAX_PROGRAM_CODE_LENGTH]),
            swizzle_data: Box::new([0; MAX_SWIZZLE_DATA_LENGTH]),
            entry_point: 0,
            cached_shader: 0,
            program_code_hash_dirty: true,
            swizzle_data_hash_dirty: true,
            program_code_hash: 0xDEADC0DE,
            swizzle_data_hash: 0xDEADC0DE,
        }
    }

    pub fn write_uniform_bool_reg(&mut self, value: u32) {
        for (i, b) in self.uniforms.b.iter_mut().enumerate() {
            *b = (value >> i) & 1 != 0;
        }
    }

    pub fn write_uniform_int_reg(&mut self, index: u32, values: Vec4<u8>) {
        assert!(
            (index as usize) < self.uniforms.i.len(),
            "Assertion Failed!"
        );
        self.uniforms.i[index as usize] = values;
    }

    pub fn write_uniform_float_reg(&mut self, config: &mut ShaderRegs, value: u32) -> Option<u32> {
        let uniform_setup = &mut config.uniform_setup;
        let is_float32 = uniform_setup.is_float32();
        if !self.uniform_queue.push(value, is_float32) {
            return None;
        }

        let uniform = self.uniform_queue.get(is_float32);
        if uniform_setup.index() as usize >= self.uniforms.f.len() {
            log::error!(
                target: "HW_GPU",
                "Invalid float uniform index {}",
                uniform_setup.index()
            );
            return None;
        }

        let index = uniform_setup.index();
        self.uniforms.f[index as usize] = uniform;
        uniform_setup.set_index(index + 1);
        Some(index)
    }

    pub fn get_program_code_hash(&mut self) -> u64 {
        if self.program_code_hash_dirty {
            self.program_code_hash = compute_hash64(words_as_bytes(&self.program_code[..]));
            self.program_code_hash_dirty = false;
        }
        self.program_code_hash
    }

    pub fn get_swizzle_data_hash(&mut self) -> u64 {
        if self.swizzle_data_hash_dirty {
            self.swizzle_data_hash = compute_hash64(words_as_bytes(&self.swizzle_data[..]));
            self.swizzle_data_hash_dirty = false;
        }
        self.swizzle_data_hash
    }

    pub fn mark_program_code_dirty(&mut self) {
        self.program_code_hash_dirty = true;
    }

    pub fn mark_swizzle_data_dirty(&mut self) {
        self.swizzle_data_hash_dirty = true;
    }
}

fn words_as_bytes(words: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(words.as_ptr().cast(), std::mem::size_of_val(words)) }
}
