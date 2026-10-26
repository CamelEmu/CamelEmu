use crate::fragment::FragmentModule;

pub trait FogEmulation {
    fn write(&self, fragment: &mut FragmentModule) -> bool;
}

pub struct NoFog;
pub struct Fog;
pub struct Gas;

impl FogEmulation for NoFog {
    fn write(&self, _fragment: &mut FragmentModule) -> bool {
        false
    }
}

impl FogEmulation for Fog {
    fn write(&self, fragment: &mut FragmentModule) -> bool {
        fragment.write_fog();
        false
    }
}

impl FogEmulation for Gas {
    fn write(&self, fragment: &mut FragmentModule) -> bool {
        fragment.write_gas();
        true
    }
}

pub fn fog_emulation(raw: u32) -> &'static dyn FogEmulation {
    match raw {
        5 => &Fog,
        7 => &Gas,
        _ => &NoFog,
    }
}
