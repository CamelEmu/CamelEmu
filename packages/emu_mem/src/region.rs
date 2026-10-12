use crate::mapping::Mapping;

/// The physical memory regions, in the order of `Memory::Region` of the core.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Region {
    Fcram = 0,
    Vram = 1,
    /// Owned by the DSP of the core, so it is never allocated here.
    Dsp = 2,
    N3ds = 3,
}

/// The FCRAM is always allocated with the size of the New 3DS, as the core does.
pub const FCRAM_SIZE: usize = 0x1000_0000;
pub const VRAM_SIZE: usize = 0x0060_0000;
pub const DSP_RAM_SIZE: usize = 0x0008_0000;
pub const N3DS_EXTRA_RAM_SIZE: usize = 0x0040_0000;

impl Region {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::Fcram),
            1 => Some(Self::Vram),
            2 => Some(Self::Dsp),
            3 => Some(Self::N3ds),
            _ => None,
        }
    }

    pub fn size(self) -> usize {
        match self {
            Self::Fcram => FCRAM_SIZE,
            Self::Vram => VRAM_SIZE,
            Self::Dsp => DSP_RAM_SIZE,
            Self::N3ds => N3DS_EXTRA_RAM_SIZE,
        }
    }
}

/// The physical memory that the emulator allocates for itself.
pub struct GuestMemory {
    fcram: Mapping,
    vram: Mapping,
    n3ds_extra_ram: Mapping,
}

impl GuestMemory {
    pub fn new() -> Option<Self> {
        Some(Self {
            fcram: Mapping::new(FCRAM_SIZE)?,
            vram: Mapping::new(VRAM_SIZE)?,
            n3ds_extra_ram: Mapping::new(N3DS_EXTRA_RAM_SIZE)?,
        })
    }

    /// Returns the start of the region, or `None` for a region this crate does not own.
    pub fn region_ptr(&self, region: Region) -> Option<*mut u8> {
        let mapping = match region {
            Region::Fcram => &self.fcram,
            Region::Vram => &self.vram,
            Region::N3ds => &self.n3ds_extra_ram,
            Region::Dsp => return None,
        };
        debug_assert_eq!(mapping.len(), region.size());
        Some(mapping.as_ptr())
    }
}
