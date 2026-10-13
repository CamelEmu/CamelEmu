use crate::bits::{bits, pica_enum, sbits};
use crate::math_util::Rectangle;
use crate::{F24, Vec4};

pica_enum!(CullMode {
    // Select which polygons are considered to be "frontfacing".
    KEEP_ALL = 0,
    KEEP_CLOCK_WISE = 1,
    KEEP_COUNTER_CLOCK_WISE = 2,
    // TODO: What does the third value imply?
});

pica_enum!(
    /// Maps components of output vertex attributes to semantics
    Semantic {
    POSITION_X = 0,
    POSITION_Y = 1,
    POSITION_Z = 2,
    POSITION_W = 3,
    QUATERNION_X = 4,
    QUATERNION_Y = 5,
    QUATERNION_Z = 6,
    QUATERNION_W = 7,
    COLOR_R = 8,
    COLOR_G = 9,
    COLOR_B = 10,
    COLOR_A = 11,
    TEXCOORD0_U = 12,
    TEXCOORD0_V = 13,
    TEXCOORD1_U = 14,
    TEXCOORD1_V = 15,
    TEXCOORD0_W = 16,
    VIEW_X = 18,
    VIEW_Y = 19,
    VIEW_Z = 20,
    TEXCOORD2_U = 22,
    TEXCOORD2_V = 23,
    INVALID = 31,
});

pica_enum!(ScissorMode {
    DISABLED = 0,
    EXCLUDE = 1, // Exclude pixels inside the scissor box

    INCLUDE = 3, // Exclude pixels outside the scissor box
});

pica_enum!(DepthBuffering {
    W_BUFFERING = 0,
    Z_BUFFERING = 1,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VsOutputAttributes {
    pub raw: u32,
}

impl VsOutputAttributes {
    pub fn map_x(&self) -> Semantic {
        Semantic(bits(self.raw, 0, 5))
    }

    pub fn map_y(&self) -> Semantic {
        Semantic(bits(self.raw, 8, 5))
    }

    pub fn map_z(&self) -> Semantic {
        Semantic(bits(self.raw, 16, 5))
    }

    pub fn map_w(&self) -> Semantic {
        Semantic(bits(self.raw, 24, 5))
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ScissorTest {
    pub mode_raw: u32,
    pub x1y1: u32,
    pub x2y2: u32,
}

impl ScissorTest {
    pub fn mode(&self) -> ScissorMode {
        ScissorMode(bits(self.mode_raw, 0, 2))
    }

    pub fn x1(&self) -> u32 {
        bits(self.x1y1, 0, 10)
    }

    pub fn y1(&self) -> u32 {
        bits(self.x1y1, 16, 10)
    }

    pub fn x2(&self) -> u32 {
        bits(self.x2y2, 0, 10)
    }

    pub fn y2(&self) -> u32 {
        bits(self.x2y2, 16, 10)
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ViewportCorner {
    pub raw: u32,
}

impl ViewportCorner {
    pub fn x(&self) -> i32 {
        sbits(self.raw, 0, 10)
    }

    pub fn y(&self) -> i32 {
        sbits(self.raw, 16, 10)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct RasterizerRegs {
    pub cull_mode_raw: u32,
    pub viewport_size_x_raw: u32,
    pub viewport_size_x_inv_raw: u32,
    pub viewport_size_y_raw: u32,
    pub viewport_size_y_inv_raw: u32,
    _padding0: [u32; 0x2],
    pub clip_enable_raw: u32,
    pub clip_coef_raw: [u32; 4], // float24
    _padding1: [u32; 0x1],
    pub viewport_depth_range_raw: u32,      // float24
    pub viewport_depth_near_plane_raw: u32, // float24
    pub vs_output_total_raw: u32,
    pub vs_output_attributes: [VsOutputAttributes; 7],
    _padding2: [u32; 0xe],
    pub scissor_test: ScissorTest,
    pub viewport_corner: ViewportCorner,
    _padding3: [u32; 0x1],
    // TODO: early depth
    _padding4: [u32; 0x1],
    _padding5: [u32; 0x2],
    pub depthmap_enable_raw: u32,
    _padding6: [u32; 0x12],
}

const _: () = assert!(size_of::<RasterizerRegs>() == 0x40 * size_of::<u32>());

impl RasterizerRegs {
    pub fn cull_mode(&self) -> CullMode {
        CullMode(bits(self.cull_mode_raw, 0, 2))
    }

    pub fn viewport_size_x(&self) -> u32 {
        bits(self.viewport_size_x_raw, 0, 24)
    }

    pub fn viewport_size_x_inv(&self) -> u32 {
        bits(self.viewport_size_x_inv_raw, 1, 31)
    }

    pub fn viewport_size_y(&self) -> u32 {
        bits(self.viewport_size_y_raw, 0, 24)
    }

    pub fn viewport_size_y_inv(&self) -> u32 {
        bits(self.viewport_size_y_inv_raw, 1, 31)
    }

    pub fn clip_enable(&self) -> bool {
        bits(self.clip_enable_raw, 0, 1) != 0
    }

    pub fn clip_coef(&self, index: usize) -> u32 {
        bits(self.clip_coef_raw[index], 0, 24)
    }

    pub fn get_clip_coef(&self) -> Vec4<F24> {
        Vec4 {
            x: F24::from_raw(self.clip_coef(0)),
            y: F24::from_raw(self.clip_coef(1)),
            z: F24::from_raw(self.clip_coef(2)),
            w: F24::from_raw(self.clip_coef(3)),
        }
    }

    pub fn get_viewport_rect(&self) -> Rectangle<i32> {
        Rectangle {
            // These registers hold half-width and half-height, so must be multiplied by 2
            left: self.viewport_corner.x(),
            top: self.viewport_corner.y()
                + (F24::from_raw(self.viewport_size_y()).to_float32() * 2.0) as i32,
            right: self.viewport_corner.x()
                + (F24::from_raw(self.viewport_size_x()).to_float32() * 2.0) as i32,
            bottom: self.viewport_corner.y(),
        }
    }

    pub fn viewport_depth_range(&self) -> u32 {
        bits(self.viewport_depth_range_raw, 0, 24)
    }

    pub fn viewport_depth_near_plane(&self) -> u32 {
        bits(self.viewport_depth_near_plane_raw, 0, 24)
    }

    pub fn vs_output_total(&self) -> u32 {
        bits(self.vs_output_total_raw, 0, 3)
    }

    pub fn validate_semantics(&self) {
        for attrib in 0..self.vs_output_total() as usize {
            let output_register_map = self.vs_output_attributes[attrib].raw;
            for comp in 0..4 {
                let semantic = (output_register_map >> (8 * comp)) & 0x1F;
                assert!(
                    semantic < 24 || semantic == Semantic::INVALID.0,
                    "Invalid/unknown semantic id: {semantic}"
                );
            }
        }
    }

    pub fn depthmap_enable(&self) -> DepthBuffering {
        DepthBuffering(bits(self.depthmap_enable_raw, 0, 1))
    }
}
