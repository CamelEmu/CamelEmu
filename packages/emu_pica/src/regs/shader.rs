use emu_pica_types::Vec4;

use crate::bits::{bits, pica_enum};

pica_enum!(ShaderMode {
    GS = 0x08,
    VS = 0xA0,
});

pica_enum!(UniformFormat {
    FLOAT24 = 0,
    FLOAT32 = 1,
});

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct UniformSetup {
    pub config: u32,

    // Writing to these registers sets the current uniform.
    pub set_value: [u32; 8],
}

impl UniformSetup {
    pub fn is_float32(&self) -> bool {
        self.format() == UniformFormat::FLOAT32
    }

    // Index of the next uniform to write to
    // TODO: ctrulib uses 8 bits for this, however that seems to yield lots of invalid
    // indices
    // TODO: Maybe the uppermost index is for the geometry shader? Investigate!
    pub fn index(&self) -> u32 {
        bits(self.config, 0, 7)
    }

    pub fn set_index(&mut self, index: u32) {
        self.config = (self.config & !0x7F) | (index & 0x7F);
    }

    pub fn format(&self) -> UniformFormat {
        UniformFormat(bits(self.config, 31, 1))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProgramUpload {
    // Offset of the next instruction to write code to.
    // Incremented with each instruction write.
    pub offset: u32,

    // Writing to these registers sets the "current" word in the shader program.
    pub set_word: [u32; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ShaderRegs {
    pub bool_uniforms_raw: u32,

    pub int_uniforms: [u32; 4],

    _padding0: [u32; 0x4],

    pub input_buffer_config: u32,

    // Offset to shader program entry point (in words)
    pub main_offset_raw: u32,

    /// Maps input attributes to registers. 4-bits per attribute, specifying a register index
    pub input_attribute_to_register_map_low: u32,
    pub input_attribute_to_register_map_high: u32,

    pub output_mask_raw: u32,

    // 0x28E, CODETRANSFER_END
    _padding1: [u32; 0x2],

    pub uniform_setup: UniformSetup,

    _padding2: [u32; 0x2],

    pub program: ProgramUpload,

    _padding3: [u32; 0x1],

    // This register group is used to load an internal table of swizzling patterns,
    // which are indexed by each shader instruction to specify vector component swizzling.
    pub swizzle_patterns: ProgramUpload,

    _padding4: [u32; 0x2],
}

const _: () = assert!(
    size_of::<ShaderRegs>() == 0x30 * size_of::<u32>(),
    "ShaderRegs struct has incorrect size"
);

impl ShaderRegs {
    pub fn bool_uniforms(&self) -> u32 {
        bits(self.bool_uniforms_raw, 0, 16)
    }

    pub fn get_int_uniform(&self, index: u32) -> Vec4<u8> {
        let values = self.int_uniforms[index as usize];
        Vec4 {
            x: bits(values, 0, 8) as u8,
            y: bits(values, 8, 8) as u8,
            z: bits(values, 16, 8) as u8,
            w: bits(values, 24, 8) as u8,
        }
    }

    // Number of input attributes to shader unit - 1
    pub fn max_input_attribute_index(&self) -> u32 {
        bits(self.input_buffer_config, 0, 4)
    }

    pub fn input_to_uniform(&self) -> u32 {
        bits(self.input_buffer_config, 8, 8)
    }

    pub fn shader_mode(&self) -> ShaderMode {
        ShaderMode(bits(self.input_buffer_config, 24, 8))
    }

    pub fn main_offset(&self) -> u32 {
        bits(self.main_offset_raw, 0, 16)
    }

    pub fn get_register_for_attribute(&self, attribute_index: usize) -> u32 {
        let map = (u64::from(self.input_attribute_to_register_map_high) << 32)
            | u64::from(self.input_attribute_to_register_map_low);
        ((map >> (attribute_index * 4)) & 0b1111) as u32
    }

    pub fn output_mask(&self) -> u32 {
        bits(self.output_mask_raw, 0, 16)
    }
}
