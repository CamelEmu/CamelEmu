#![deny(unsafe_code)]

mod id;
mod instructions;
mod module;
mod operand;
mod stream;

pub use id::Id;
pub use instructions::{GlslInstruction, glsl};
pub use module::{Module, ModuleSettings};
pub use operand::{Literal, Operand};
pub use stream::{Declarations, Stream, words_in_string};
