use emu_spirv::Id;

use crate::error::{Result, ShaderError};
use crate::fragment::FragmentModule;

pub trait AlphaTestFunc {
    fn discards_all(&self) -> bool {
        false
    }

    fn tests_alpha(&self) -> bool {
        true
    }

    fn discard_condition(&self, fragment: &mut FragmentModule, alpha: Id, reference: Id) -> Id;
}

pub struct Never;
pub struct Always;
pub struct Equal;
pub struct NotEqual;
pub struct LessThan;
pub struct LessThanOrEqual;
pub struct GreaterThan;
pub struct GreaterThanOrEqual;

impl AlphaTestFunc for Never {
    fn discards_all(&self) -> bool {
        true
    }

    fn tests_alpha(&self) -> bool {
        false
    }

    fn discard_condition(&self, _fragment: &mut FragmentModule, _alpha: Id, _reference: Id) -> Id {
        Id::default()
    }
}

impl AlphaTestFunc for Always {
    fn tests_alpha(&self) -> bool {
        false
    }

    fn discard_condition(&self, _fragment: &mut FragmentModule, _alpha: Id, _reference: Id) -> Id {
        Id::default()
    }
}

impl AlphaTestFunc for Equal {
    fn discard_condition(&self, fragment: &mut FragmentModule, alpha: Id, reference: Id) -> Id {
        let bool_id = fragment.bool_id;
        fragment.op_i_not_equal(bool_id, alpha, reference)
    }
}

impl AlphaTestFunc for NotEqual {
    fn discard_condition(&self, fragment: &mut FragmentModule, alpha: Id, reference: Id) -> Id {
        let bool_id = fragment.bool_id;
        fragment.op_i_equal(bool_id, alpha, reference)
    }
}

impl AlphaTestFunc for LessThan {
    fn discard_condition(&self, fragment: &mut FragmentModule, alpha: Id, reference: Id) -> Id {
        let bool_id = fragment.bool_id;
        fragment.op_s_greater_than_equal(bool_id, alpha, reference)
    }
}

impl AlphaTestFunc for LessThanOrEqual {
    fn discard_condition(&self, fragment: &mut FragmentModule, alpha: Id, reference: Id) -> Id {
        let bool_id = fragment.bool_id;
        fragment.op_s_greater_than(bool_id, alpha, reference)
    }
}

impl AlphaTestFunc for GreaterThan {
    fn discard_condition(&self, fragment: &mut FragmentModule, alpha: Id, reference: Id) -> Id {
        let bool_id = fragment.bool_id;
        fragment.op_s_less_than_equal(bool_id, alpha, reference)
    }
}

impl AlphaTestFunc for GreaterThanOrEqual {
    fn discard_condition(&self, fragment: &mut FragmentModule, alpha: Id, reference: Id) -> Id {
        let bool_id = fragment.bool_id;
        fragment.op_s_less_than(bool_id, alpha, reference)
    }
}

pub fn alpha_test_func(raw: u32) -> &'static dyn AlphaTestFunc {
    match raw {
        0 => &Never,
        2 => &Equal,
        3 => &NotEqual,
        4 => &LessThan,
        5 => &LessThanOrEqual,
        6 => &GreaterThan,
        7 => &GreaterThanOrEqual,
        _ => &Always,
    }
}

pub trait ScissorMode {
    fn is_enabled(&self) -> bool {
        true
    }

    fn apply(&self, fragment: &mut FragmentModule, result: Id) -> Id;
}

pub struct ScissorDisabled;
pub struct ScissorExclude;
pub struct ScissorInclude;

impl ScissorMode for ScissorDisabled {
    fn is_enabled(&self) -> bool {
        false
    }

    fn apply(&self, _fragment: &mut FragmentModule, result: Id) -> Id {
        result
    }
}

impl ScissorMode for ScissorExclude {
    fn apply(&self, _fragment: &mut FragmentModule, result: Id) -> Id {
        result
    }
}

impl ScissorMode for ScissorInclude {
    fn apply(&self, fragment: &mut FragmentModule, result: Id) -> Id {
        let bool_id = fragment.bool_id;
        fragment.op_logical_not(bool_id, result)
    }
}

pub fn scissor_mode(raw: u32) -> &'static dyn ScissorMode {
    match raw {
        0 => &ScissorDisabled,
        3 => &ScissorInclude,
        _ => &ScissorExclude,
    }
}

pub trait DepthBuffering {
    fn apply(&self, fragment: &mut FragmentModule, input_pointer_id: Id);
}

pub struct WBuffering;
pub struct ZBuffering;

impl DepthBuffering for WBuffering {
    fn apply(&self, fragment: &mut FragmentModule, input_pointer_id: Id) {
        let f32_id = fragment.f32_id;
        let gl_frag_coord_id = fragment.gl_frag_coord_id;
        let index = fragment.const_u32(3);
        let access_chain = fragment.op_access_chain(input_pointer_id, gl_frag_coord_id, &[index]);
        let gl_frag_coord_w = fragment.op_load(f32_id, access_chain, None);
        let depth = fragment.depth;
        fragment.depth = fragment.op_f_div(f32_id, depth, gl_frag_coord_w);
    }
}

impl DepthBuffering for ZBuffering {
    fn apply(&self, _fragment: &mut FragmentModule, _input_pointer_id: Id) {}
}

pub fn depth_buffering(raw: u32) -> &'static dyn DepthBuffering {
    match raw {
        0 => &WBuffering,
        _ => &ZBuffering,
    }
}

pub trait LogicOp {
    fn apply(&self, fragment: &mut FragmentModule, color: Id, raw: u32) -> Result<Id>;
}

pub struct Clear;
pub struct Set;
pub struct Copy;
pub struct CopyInverted;
pub struct NoOp;
pub struct UnhandledLogicOp;

impl LogicOp for Clear {
    fn apply(&self, fragment: &mut FragmentModule, _color: Id, _raw: u32) -> Result<Id> {
        Ok(fragment.const_f32s(&[0.0, 0.0, 0.0, 0.0]))
    }
}

impl LogicOp for Set {
    fn apply(&self, fragment: &mut FragmentModule, _color: Id, _raw: u32) -> Result<Id> {
        Ok(fragment.const_f32s(&[1.0, 1.0, 1.0, 1.0]))
    }
}

impl LogicOp for Copy {
    fn apply(&self, _fragment: &mut FragmentModule, color: Id, _raw: u32) -> Result<Id> {
        Ok(color)
    }
}

impl LogicOp for CopyInverted {
    fn apply(&self, _fragment: &mut FragmentModule, color: Id, _raw: u32) -> Result<Id> {
        Ok(color)
    }
}

impl LogicOp for NoOp {
    fn apply(&self, _fragment: &mut FragmentModule, color: Id, _raw: u32) -> Result<Id> {
        Ok(color)
    }
}

impl LogicOp for UnhandledLogicOp {
    fn apply(&self, _fragment: &mut FragmentModule, _color: Id, raw: u32) -> Result<Id> {
        Err(ShaderError::UnsupportedValue {
            kind: "logic operation",
            value: raw,
        })
    }
}

pub fn logic_op(raw: u32) -> &'static dyn LogicOp {
    match raw {
        0 => &Clear,
        4 => &Set,
        3 => &Copy,
        5 => &CopyInverted,
        6 => &NoOp,
        _ => &UnhandledLogicOp,
    }
}
