use std::mem::offset_of;
use std::ptr::NonNull;

use emu_pica_types::{AttributeBuffer, F24, Vec4};

use crate::aligned::Aligned;
use crate::regs::ShaderRegs;

/// Handler type for receiving vertex outputs from vertex shader or geometry shader
pub type VertexHandler = Box<dyn FnMut(&AttributeBuffer)>;

/// Handler type for signaling to invert the vertex order of the next triangle
pub type WindingSetter = Box<dyn FnMut()>;

/// This structure contains the state information that needs to be unique for a shader unit. The 3DS
/// has four shader units that process shaders in parallel.
#[repr(C)]
#[derive(Clone, Debug)]
pub struct ShaderUnit {
    pub address_registers: [i32; 3],
    pub conditional_code: [bool; 2],
    pub input: Aligned<[Vec4<F24>; 16]>,
    pub temporary: Aligned<[Vec4<F24>; 16]>,
    pub output: Aligned<[Vec4<F24>; 16]>,
    pub emitter_ptr: Option<NonNull<GeometryEmitter>>,
}

impl Default for ShaderUnit {
    fn default() -> Self {
        Self::new(None)
    }
}

impl ShaderUnit {
    pub fn new(emitter: Option<NonNull<GeometryEmitter>>) -> Self {
        Self {
            address_registers: [0; 3],
            conditional_code: [false; 2],
            input: Aligned::default(),
            temporary: Aligned::default(),
            output: Aligned::default(),
            emitter_ptr: emitter,
        }
    }

    pub fn load_input(&mut self, config: &ShaderRegs, buffer: &AttributeBuffer) {
        let max_attribute = config.max_input_attribute_index();
        for (attr, value) in buffer.iter().enumerate().take(max_attribute as usize + 1) {
            let reg = config.get_register_for_attribute(attr) as usize;
            self.input[reg] = *value;
        }
    }

    pub fn write_output(&self, config: &ShaderRegs, buffer: &mut AttributeBuffer) {
        for (output_index, reg) in set_bits(config.output_mask()).enumerate() {
            buffer[output_index] = self.output[reg];
        }
    }

    pub const fn input_offset(register_index: i32) -> usize {
        offset_of!(ShaderUnit, input) + register_index as usize * size_of::<Vec4<F24>>()
    }

    pub const fn output_offset(register_index: i32) -> usize {
        offset_of!(ShaderUnit, output) + register_index as usize * size_of::<Vec4<F24>>()
    }

    pub const fn temporary_offset(register_index: i32) -> usize {
        offset_of!(ShaderUnit, temporary) + register_index as usize * size_of::<Vec4<F24>>()
    }
}

/// The positions of the set bits, from the lowest, as `Common::BitSet` iterates them.
pub(crate) fn set_bits(mut value: u32) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if value == 0 {
            return None;
        }
        let bit = value.trailing_zeros();
        value &= value - 1;
        Some(bit as usize)
    })
}

pub struct Handlers {
    pub vertex_handler: VertexHandler,
    pub winding_setter: WindingSetter,
}

/// This structure contains state information for primitive emitting in geometry shader.
#[derive(Default)]
pub struct GeometryEmitter {
    pub buffer: [AttributeBuffer; 3],
    pub vertex_id: u8,
    pub prim_emit: bool,
    pub winding: bool,
    pub output_mask: u32,
    pub handlers: Option<Box<Handlers>>,
}

impl GeometryEmitter {
    pub fn emit(&mut self, output_regs: &[Vec4<F24>; 16]) {
        assert!(self.vertex_id < 3, "Assertion Failed!");

        for (output_index, reg) in set_bits(self.output_mask).enumerate() {
            self.buffer[self.vertex_id as usize][output_index] = output_regs[reg];
        }

        if self.prim_emit {
            let handlers = self
                .handlers
                .as_mut()
                .expect("the vertex handlers are set before the geometry shader runs");
            if self.winding {
                (handlers.winding_setter)();
            }
            for i in 0..self.buffer.len() {
                (handlers.vertex_handler)(&self.buffer[i]);
            }
        }
    }
}

/// This is an extended shader unit state that represents the special unit that can run both vertex
/// shader and geometry shader. It contains an additional primitive emitter and utilities for
/// geometry shader.
pub struct GeometryShaderUnit {
    pub unit: ShaderUnit,
    pub emitter: GeometryEmitter,
}

impl GeometryShaderUnit {
    /// The unit points to its own emitter, so it lives in a box that keeps its address.
    pub fn new() -> Box<Self> {
        let mut unit = Box::new(Self {
            unit: ShaderUnit::new(None),
            emitter: GeometryEmitter::default(),
        });
        unit.unit.emitter_ptr = Some(NonNull::from(&mut unit.emitter));
        unit
    }

    pub fn set_vertex_handlers(
        &mut self,
        vertex_handler: VertexHandler,
        winding_setter: WindingSetter,
    ) {
        self.emitter.handlers = Some(Box::new(Handlers {
            vertex_handler,
            winding_setter,
        }));
    }

    pub fn config_output(&mut self, config: &ShaderRegs) {
        self.emitter.output_mask = config.output_mask();
    }
}
