use crate::id::Id;
use crate::stream::Stream;

pub trait Operand {
    fn write(&self, stream: &mut Stream);
}

pub trait Literal: Operand {}

impl<T: Operand + ?Sized> Operand for &T {
    fn write(&self, stream: &mut Stream) {
        (**self).write(stream);
    }
}

impl<T: Operand> Operand for [T] {
    fn write(&self, stream: &mut Stream) {
        for value in self {
            value.write(stream);
        }
    }
}

impl<T: Operand> Operand for Option<T> {
    fn write(&self, stream: &mut Stream) {
        if let Some(value) = self {
            value.write(stream);
        }
    }
}

impl Operand for u32 {
    fn write(&self, stream: &mut Stream) {
        stream.put_word(*self);
    }
}

impl Operand for i32 {
    fn write(&self, stream: &mut Stream) {
        stream.put_word(*self as u32);
    }
}

impl Operand for u64 {
    fn write(&self, stream: &mut Stream) {
        stream.put_word(*self as u32);
        stream.put_word((*self >> 32) as u32);
    }
}

impl Operand for i64 {
    fn write(&self, stream: &mut Stream) {
        (*self as u64).write(stream);
    }
}

impl Operand for f32 {
    fn write(&self, stream: &mut Stream) {
        self.to_bits().write(stream);
    }
}

impl Operand for f64 {
    fn write(&self, stream: &mut Stream) {
        self.to_bits().write(stream);
    }
}

impl Operand for bool {
    fn write(&self, stream: &mut Stream) {
        u32::from(*self).write(stream);
    }
}

impl Operand for Id {
    fn write(&self, stream: &mut Stream) {
        debug_assert!(self.value != 0);
        self.value.write(stream);
    }
}

impl Operand for str {
    fn write(&self, stream: &mut Stream) {
        stream.put_string(self);
    }
}

impl Literal for u32 {}
impl Literal for i32 {}
impl Literal for u64 {}
impl Literal for i64 {}
impl Literal for f32 {}
impl Literal for f64 {}

macro_rules! operand_enums {
    ($($type:ty),* $(,)?) => {
        $(
            impl Operand for $type {
                fn write(&self, stream: &mut Stream) {
                    (*self as u32).write(stream);
                }
            }
        )*
    };
}

macro_rules! operand_masks {
    ($($type:ty),* $(,)?) => {
        $(
            impl Operand for $type {
                fn write(&self, stream: &mut Stream) {
                    self.bits().write(stream);
                }
            }
        )*
    };
}

operand_enums!(
    spirv::Op,
    spirv::Capability,
    spirv::AddressingModel,
    spirv::MemoryModel,
    spirv::ExecutionModel,
    spirv::ExecutionMode,
    spirv::StorageClass,
    spirv::Dim,
    spirv::ImageFormat,
    spirv::AccessQualifier,
    spirv::SamplerAddressingMode,
    spirv::SamplerFilterMode,
    spirv::Decoration,
);

operand_masks!(
    spirv::FunctionControl,
    spirv::SelectionControl,
    spirv::LoopControl,
    spirv::MemoryAccess,
    spirv::ImageOperands,
);
