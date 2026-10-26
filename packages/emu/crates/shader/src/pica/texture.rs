use emu_spirv::Id;

use crate::error::{Result, ShaderError};
use crate::fragment::FragmentModule;

pub trait Texture0Type {
    fn is_disabled(&self) -> bool {
        false
    }

    fn uses_cube_image(&self) -> bool {
        false
    }

    fn sample(&self, fragment: &mut FragmentModule, texcoord: Id, raw: u32) -> Result<Id>;
}

pub struct Texture2D;
pub struct TextureCube;
pub struct Shadow2D;
pub struct Projection2D;
pub struct Disabled;
pub struct UnsupportedTexture;

impl Texture0Type for Texture2D {
    fn sample(&self, fragment: &mut FragmentModule, texcoord: Id, _raw: u32) -> Result<Id> {
        let tex0_id = fragment.tex0_id;
        Ok(fragment.sample_lod(tex0_id, texcoord, 0))
    }
}

impl Texture0Type for TextureCube {
    fn uses_cube_image(&self) -> bool {
        true
    }

    fn sample(&self, fragment: &mut FragmentModule, texcoord: Id, _raw: u32) -> Result<Id> {
        let tex0_id = fragment.tex0_id;
        Ok(fragment.sample_3d(tex0_id, false, texcoord))
    }
}

impl Texture0Type for Shadow2D {
    fn sample(&self, fragment: &mut FragmentModule, _texcoord: Id, _raw: u32) -> Result<Id> {
        Ok(fragment.sample_shadow())
    }
}

impl Texture0Type for Projection2D {
    fn sample(&self, fragment: &mut FragmentModule, texcoord: Id, _raw: u32) -> Result<Id> {
        let tex0_id = fragment.tex0_id;
        Ok(fragment.sample_3d(tex0_id, true, texcoord))
    }
}

impl Texture0Type for Disabled {
    fn is_disabled(&self) -> bool {
        true
    }

    fn sample(&self, _fragment: &mut FragmentModule, _texcoord: Id, raw: u32) -> Result<Id> {
        Err(unsupported(raw))
    }
}

impl Texture0Type for UnsupportedTexture {
    fn sample(&self, _fragment: &mut FragmentModule, _texcoord: Id, raw: u32) -> Result<Id> {
        Err(unsupported(raw))
    }
}

fn unsupported(raw: u32) -> ShaderError {
    ShaderError::UnsupportedValue {
        kind: "texture type",
        value: raw,
    }
}

pub fn texture0_type(raw: u32) -> &'static dyn Texture0Type {
    match raw {
        0 => &Texture2D,
        1 => &TextureCube,
        2 => &Shadow2D,
        3 => &Projection2D,
        5 => &Disabled,
        _ => &UnsupportedTexture,
    }
}
