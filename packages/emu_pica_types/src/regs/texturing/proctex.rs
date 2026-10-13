use crate::bits::{assign, bits, pica_enum, sbits};

pica_enum!(ProcTexClamp {
    TO_ZERO = 0,
    TO_EDGE = 1,
    SYMMETRICAL_REPEAT = 2,
    MIRRORED_REPEAT = 3,
    PULSE = 4,
});

pica_enum!(ProcTexCombiner {
    U = 0,          // u
    U2 = 1,         // u * u
    V = 2,          // v
    V2 = 3,         // v * v
    ADD = 4,        // (u + v) / 2
    ADD2 = 5,       // (u * u + v * v) / 2
    SQRT_ADD2 = 6,  // sqrt(u * u + v * v)
    MIN = 7,        // min(u, v)
    MAX = 8,        // max(u, v)
    R_MAX = 9,      // Average of Max and SqrtAdd2
});

pica_enum!(ProcTexShift {
    NONE = 0,
    ODD = 1,
    EVEN = 2,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTex {
    pub raw: u32,
}

impl ProcTex {
    pub fn u_clamp(&self) -> ProcTexClamp {
        ProcTexClamp(bits(self.raw, 0, 3))
    }

    pub fn v_clamp(&self) -> ProcTexClamp {
        ProcTexClamp(bits(self.raw, 3, 3))
    }

    pub fn color_combiner(&self) -> ProcTexCombiner {
        ProcTexCombiner(bits(self.raw, 6, 4))
    }

    pub fn alpha_combiner(&self) -> ProcTexCombiner {
        ProcTexCombiner(bits(self.raw, 10, 4))
    }

    pub fn separate_alpha(&self) -> u32 {
        bits(self.raw, 14, 1)
    }

    pub fn noise_enable(&self) -> u32 {
        bits(self.raw, 15, 1)
    }

    pub fn u_shift(&self) -> ProcTexShift {
        ProcTexShift(bits(self.raw, 16, 2))
    }

    pub fn v_shift(&self) -> ProcTexShift {
        ProcTexShift(bits(self.raw, 18, 2))
    }

    pub fn bias_low(&self) -> u32 {
        bits(self.raw, 20, 8) // float16 TODO: unimplemented
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexNoiseConfig {
    pub raw: u32,
}

impl ProcTexNoiseConfig {
    pub fn amplitude(&self) -> i32 {
        sbits(self.raw, 0, 16) // fixed1.3.12
    }

    pub fn phase(&self) -> u32 {
        bits(self.raw, 16, 16) // float16
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexNoiseFrequency {
    pub raw: u32,
}

impl ProcTexNoiseFrequency {
    pub fn u(&self) -> u32 {
        bits(self.raw, 0, 16) // float16
    }

    pub fn v(&self) -> u32 {
        bits(self.raw, 16, 16) // float16
    }
}

pica_enum!(ProcTexFilter {
    NEAREST = 0,
    LINEAR = 1,
    NEAREST_MIPMAP_NEAREST = 2,
    LINEAR_MIPMAP_NEAREST = 3,
    NEAREST_MIPMAP_LINEAR = 4,
    LINEAR_MIPMAP_LINEAR = 5,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexLut {
    pub raw: u32,
}

impl ProcTexLut {
    pub fn filter(&self) -> ProcTexFilter {
        ProcTexFilter(bits(self.raw, 0, 3))
    }

    pub fn lod_min(&self) -> u32 {
        bits(self.raw, 3, 4)
    }

    pub fn lod_max(&self) -> u32 {
        bits(self.raw, 7, 4)
    }

    pub fn width(&self) -> u32 {
        bits(self.raw, 11, 8)
    }

    pub fn bias_high(&self) -> u32 {
        bits(self.raw, 19, 8) // TODO: unimplemented
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexLutOffset {
    pub raw: u32,
}

impl ProcTexLutOffset {
    pub fn level0(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    pub fn level1(&self) -> u32 {
        bits(self.raw, 8, 8)
    }

    pub fn level2(&self) -> u32 {
        bits(self.raw, 16, 8)
    }

    pub fn level3(&self) -> u32 {
        bits(self.raw, 24, 8)
    }
}

pica_enum!(ProcTexLutTable {
    NOISE = 0,
    COLOR_MAP = 2,
    ALPHA_MAP = 3,
    COLOR = 4,
    COLOR_DIFF = 5,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexLutConfig {
    pub raw: u32,
}

impl ProcTexLutConfig {
    pub fn index(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    pub fn set_index(&mut self, index: u32) {
        assign(&mut self.raw, 0, 8, index);
    }

    pub fn ref_table(&self) -> ProcTexLutTable {
        ProcTexLutTable(bits(self.raw, 8, 4))
    }
}
