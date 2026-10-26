macro_rules! typed_instructions {
    ($($name:ident($($arg:ident),*) => $op:ident;)*) => {
        impl crate::module::Module {
            $(
                #[allow(clippy::too_many_arguments)]
                pub fn $name(
                    &mut self,
                    result_type: crate::id::Id,
                    $($arg: crate::id::Id),*
                ) -> crate::id::Id {
                    let reserve = 3 + [$(stringify!($arg)),*].len();
                    self.code.emit_typed(
                        &mut self.bound,
                        reserve,
                        spirv::Op::$op,
                        result_type,
                        &[$(&$arg),*],
                    )
                }
            )*
        }
    };
}

macro_rules! untyped_instructions {
    ($($name:ident($($arg:ident),*) => $op:ident;)*) => {
        impl crate::module::Module {
            $(
                pub fn $name(&mut self, $($arg: crate::id::Id),*) -> crate::id::Id {
                    let reserve = 1 + [$(stringify!($arg)),*].len();
                    self.code.emit(&mut self.bound, reserve, spirv::Op::$op, &[$(&$arg),*])
                }
            )*
        }
    };
}

mod annotation;
mod arithmetic;
mod atomic;
mod barrier;
mod bit;
mod constant;
mod conversion;
mod debug;
mod derivatives;
mod extension;
mod flow;
mod function;
mod group;
mod image;
mod logical;
mod memory;
mod misc;
mod types;

pub use extension::GlslInstruction;
pub use extension::glsl;
