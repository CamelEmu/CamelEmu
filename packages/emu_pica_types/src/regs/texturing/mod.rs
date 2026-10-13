mod proctex;
mod tev;
mod texture;

pub use proctex::{
    ProcTex, ProcTexClamp, ProcTexCombiner, ProcTexFilter, ProcTexLut, ProcTexLutConfig,
    ProcTexLutOffset, ProcTexLutTable, ProcTexNoiseConfig, ProcTexNoiseFrequency, ProcTexShift,
};
pub use tev::{
    FogMode, FogModeConfig, Rgba8, TevAlphaModifier, TevColorModifier, TevOperation, TevSource,
    TevStageConfig,
};
pub use texture::{
    CubeFace, FullTextureConfig, TextureConfig, TextureFilter, TextureFormat, TextureType, WrapMode,
};

use crate::bits::{assign, bits, mask};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MainConfig {
    pub raw: u32,
}

impl MainConfig {
    pub fn texture0_enable(&self) -> u32 {
        bits(self.raw, 0, 1)
    }

    pub fn texture1_enable(&self) -> u32 {
        bits(self.raw, 1, 1)
    }

    pub fn texture2_enable(&self) -> u32 {
        bits(self.raw, 2, 1)
    }

    pub fn texture3_coordinates(&self) -> u32 {
        bits(self.raw, 8, 2)
    }

    pub fn texture3_enable(&self) -> u32 {
        bits(self.raw, 10, 1)
    }

    pub fn texture2_use_coord1(&self) -> u32 {
        bits(self.raw, 13, 1)
    }

    pub fn clear_texture_cache(&self) -> u32 {
        bits(self.raw, 16, 1) // TODO: unimplemented
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TexturingRegs {
    pub main_config: MainConfig,
    pub texture0: TextureConfig,
    pub cube_address_raw: [u32; 5],
    pub shadow: u32,
    _padding0: [u32; 0x2],
    pub texture0_format_raw: u32,
    pub fragment_lighting_enable_raw: u32,
    _padding1: [u32; 0x1],
    pub texture1: TextureConfig,
    pub texture1_format_raw: u32,
    _padding2: [u32; 0x2],
    pub texture2: TextureConfig,
    pub texture2_format_raw: u32,
    _padding3: [u32; 0x9],
    // 0xa8-0xad: ProcTex Config
    pub proctex: ProcTex,
    pub proctex_noise_u: ProcTexNoiseConfig,
    pub proctex_noise_v: ProcTexNoiseConfig,
    pub proctex_noise_frequency: ProcTexNoiseFrequency,
    pub proctex_lut: ProcTexLut,
    pub proctex_lut_offset: ProcTexLutOffset,
    _padding4: [u32; 0x1],
    // 0xaf-0xb7: ProcTex LUT
    pub proctex_lut_config: ProcTexLutConfig,
    pub proctex_lut_data: [u32; 8],
    _padding5: [u32; 0x8],
    pub tev_stage0: TevStageConfig,
    _padding6: [u32; 0x3],
    pub tev_stage1: TevStageConfig,
    _padding7: [u32; 0x3],
    pub tev_stage2: TevStageConfig,
    _padding8: [u32; 0x3],
    pub tev_stage3: TevStageConfig,
    _padding9: [u32; 0x3],
    pub fog_mode: FogModeConfig,
    pub fog_color: Rgba8,
    _padding10: [u32; 0x4],
    pub fog_lut_offset_raw: u32,
    _padding11: [u32; 0x1],
    pub fog_lut_data: [u32; 8],
    pub tev_stage4: TevStageConfig,
    _padding12: [u32; 0x3],
    pub tev_stage5: TevStageConfig,
    pub tev_combiner_buffer_color: Rgba8,
    _padding13: [u32; 0x2],
}

const _: () = assert!(size_of::<TexturingRegs>() == 0x80 * size_of::<u32>());

impl TexturingRegs {
    pub fn cube_address(&self, index: usize) -> u32 {
        bits(self.cube_address_raw[index], 0, 22)
    }

    pub fn get_cube_physical_address(&self, face: CubeFace) -> u32 {
        let mut address = self.texture0.address();
        if face != CubeFace::PositiveX {
            // Bits [22:27] from the main texture address is shared with all cubemap additional
            // addresses.
            let face_addr = self.cube_address(face as usize - 1);
            address &= !mask(22);
            address |= face_addr;
        }
        // A multiplier of 8 is also needed in the same way as the main address.
        address.wrapping_mul(8)
    }

    pub fn shadow_orthographic(&self) -> u32 {
        bits(self.shadow, 0, 1) // 0: enable perspective divide
    }

    pub fn shadow_bias(&self) -> u32 {
        bits(self.shadow, 1, 23) // 23-bit fraction
    }

    pub fn texture0_format(&self) -> TextureFormat {
        TextureFormat(bits(self.texture0_format_raw, 0, 4))
    }

    pub fn fragment_lighting_enable(&self) -> u32 {
        bits(self.fragment_lighting_enable_raw, 0, 1)
    }

    pub fn texture1_format(&self) -> TextureFormat {
        TextureFormat(bits(self.texture1_format_raw, 0, 4))
    }

    pub fn texture2_format(&self) -> TextureFormat {
        TextureFormat(bits(self.texture2_format_raw, 0, 4))
    }

    pub fn get_textures(&self) -> [FullTextureConfig; 3] {
        [
            FullTextureConfig {
                enabled: self.main_config.texture0_enable(),
                config: self.texture0,
                format: self.texture0_format(),
            },
            FullTextureConfig {
                enabled: self.main_config.texture1_enable(),
                config: self.texture1,
                format: self.texture1_format(),
            },
            FullTextureConfig {
                enabled: self.main_config.texture2_enable(),
                config: self.texture2,
                format: self.texture2_format(),
            },
        ]
    }

    pub fn fog_lut_offset(&self) -> u32 {
        bits(self.fog_lut_offset_raw, 0, 16)
    }

    pub fn set_fog_lut_offset(&mut self, offset: u32) {
        assign(&mut self.fog_lut_offset_raw, 0, 16, offset);
    }

    pub fn get_tev_stages(&self) -> [TevStageConfig; 6] {
        [
            self.tev_stage0,
            self.tev_stage1,
            self.tev_stage2,
            self.tev_stage3,
            self.tev_stage4,
            self.tev_stage5,
        ]
    }
}
