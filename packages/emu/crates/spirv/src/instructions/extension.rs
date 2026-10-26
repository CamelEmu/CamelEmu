use crate::id::Id;
use crate::module::Module;

pub trait GlslInstruction {
    const NUMBER: u32;
}

macro_rules! glsl_instructions {
    ($($name:ident = $number:expr;)*) => {
        pub mod glsl {
            use super::GlslInstruction;
            $(
                pub struct $name;

                impl GlslInstruction for $name {
                    const NUMBER: u32 = $number;
                }
            )*
        }
    };
}

glsl_instructions! {
    Round = 1;
    RoundEven = 2;
    Trunc = 3;
    FAbs = 4;
    SAbs = 5;
    FSign = 6;
    SSign = 7;
    Floor = 8;
    Ceil = 9;
    Fract = 10;
    Sin = 13;
    Cos = 14;
    Asin = 16;
    Acos = 17;
    Pow = 26;
    Exp = 27;
    Log = 28;
    Exp2 = 29;
    Log2 = 30;
    Sqrt = 31;
    InverseSqrt = 32;
    FMin = 37;
    UMin = 38;
    SMin = 39;
    FMax = 40;
    UMax = 41;
    SMax = 42;
    FClamp = 43;
    UClamp = 44;
    SClamp = 45;
    FMix = 46;
    Fma = 50;
    PackHalf2x16 = 58;
    UnpackHalf2x16 = 62;
    Length = 66;
    Cross = 68;
    Normalize = 69;
    FindILsb = 73;
    FindSMsb = 74;
    FindUMsb = 75;
    InterpolateAtCentroid = 76;
    InterpolateAtSample = 77;
    InterpolateAtOffset = 78;
}

impl Module {
    pub fn op_ext_inst(
        &mut self,
        result_type: Id,
        set: Id,
        instruction: u32,
        operands: &[Id],
    ) -> Id {
        self.code.emit_typed(
            &mut self.bound,
            5 + operands.len(),
            spirv::Op::ExtInst,
            result_type,
            &[&set, &instruction, &operands],
        )
    }

    pub fn glsl<I: GlslInstruction>(&mut self, result_type: Id, operands: &[Id]) -> Id {
        let set = self.get_glsl_std450();
        self.op_ext_inst(result_type, set, I::NUMBER, operands)
    }
}
