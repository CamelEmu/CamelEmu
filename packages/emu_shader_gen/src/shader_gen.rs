use emu_pica_types::regs::RegsInternal;
use emu_pica_types::regs::rasterizer::Semantic;

// NOTE: Changing the order impacts shader transferable and precompiled cache loading.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProgramType {
    VS = 0,
    FS = 1,
    GS = 2,
}

pub const ATTRIBUTE_POSITION: u32 = 0;
pub const ATTRIBUTE_COLOR: u32 = 1;
pub const ATTRIBUTE_TEXCOORD0: u32 = 2;
pub const ATTRIBUTE_TEXCOORD1: u32 = 3;
pub const ATTRIBUTE_TEXCOORD2: u32 = 4;
pub const ATTRIBUTE_TEXCOORD0_W: u32 = 5;
pub const ATTRIBUTE_NORMQUAT: u32 = 6;
pub const ATTRIBUTE_VIEW: u32 = 7;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct AttribLoadFlags(pub u32);

impl AttribLoadFlags {
    pub const FLOAT: Self = Self(1 << 0);
    pub const SINT: Self = Self(1 << 1);
    pub const UINT: Self = Self(1 << 2);
    pub const ZERO_W: Self = Self(1 << 3);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SemanticMap {
    pub attribute_index: u32,
    pub component_index: u32,
}

/**
 * This struct contains common information to identify a GLSL geometry shader generated from
 * PICA geometry shader.
 */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PicaGSConfigState {
    pub use_clip_planes: bool,

    pub vs_output_attributes: u32,
    pub gs_output_attributes: u32,

    // semantic_maps[semantic name] -> GS output attribute index + component index
    pub semantic_maps: [SemanticMap; 24],
}

impl PicaGSConfigState {
    pub fn init(&mut self, regs: &RegsInternal, use_clip_planes: bool) {
        self.use_clip_planes = use_clip_planes;

        self.vs_output_attributes = regs.vs.output_mask().count_ones();
        self.gs_output_attributes = self.vs_output_attributes;

        self.semantic_maps.fill(SemanticMap {
            attribute_index: 16,
            component_index: 0,
        });
        for attrib in 0..regs.rasterizer.vs_output_total() {
            let attributes = &regs.rasterizer.vs_output_attributes[attrib as usize];
            let semantics = [
                attributes.map_x(),
                attributes.map_y(),
                attributes.map_z(),
                attributes.map_w(),
            ];
            for (comp, &semantic) in semantics.iter().enumerate() {
                if (semantic.0 as usize) < 24 {
                    self.semantic_maps[semantic.0 as usize] = SemanticMap {
                        attribute_index: attrib,
                        component_index: comp as u32,
                    };
                } else if semantic != Semantic::INVALID {
                    log::error!(target: "Render", "Invalid/unknown semantic id: {}", semantic.0);
                }
            }
        }
    }
}

/**
 * This struct contains common information to identify a GLSL vertex shader generated from
 * PICA vertex shader.
 */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PicaVSConfigState {
    pub use_clip_planes: bool,
    pub use_geometry_shader: bool,

    pub program_hash: u64,
    pub swizzle_hash: u64,
    pub main_offset: u32,
    pub sanitize_mul: bool,

    pub num_outputs: u32,
    // Load operations to apply to the input vertex data
    pub load_flags: [AttribLoadFlags; 16],

    // output_map[output register index] -> output attribute index
    pub output_map: [u32; 16],

    pub gs_state: PicaGSConfigState,
}

impl PicaVSConfigState {
    /// The hashes of the program code and the swizzle data come from the `ShaderSetup` of the
    /// vertex shader.
    pub fn init(
        &mut self,
        regs: &RegsInternal,
        program_hash: u64,
        swizzle_hash: u64,
        use_clip_planes: bool,
        use_geometry_shader: bool,
        accurate_mul: bool,
    ) {
        self.use_clip_planes = use_clip_planes;
        self.use_geometry_shader = use_geometry_shader;
        self.sanitize_mul = accurate_mul;

        self.program_hash = program_hash;
        self.swizzle_hash = swizzle_hash;
        self.main_offset = regs.vs.main_offset();

        self.num_outputs = 0;
        self.load_flags.fill(AttribLoadFlags::FLOAT);
        self.output_map.fill(16);

        let output_mask = regs.vs.output_mask();
        for reg in 0..16 {
            if output_mask & (1 << reg) == 0 {
                continue;
            }
            self.output_map[reg] = self.num_outputs;
            self.num_outputs += 1;
        }

        if !use_geometry_shader {
            self.gs_state.init(regs, use_clip_planes);
        }
    }
}

/**
 * This struct contains information to identify a GL vertex shader generated from PICA vertex
 * shader.
 */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PicaVSConfig {
    pub state: PicaVSConfigState,
}

impl PicaVSConfig {
    pub fn new(
        regs: &RegsInternal,
        program_hash: u64,
        swizzle_hash: u64,
        use_clip_planes: bool,
        use_geometry_shader: bool,
        accurate_mul: bool,
    ) -> Self {
        let mut config = Self::default();
        config.state.init(
            regs,
            program_hash,
            swizzle_hash,
            use_clip_planes,
            use_geometry_shader,
            accurate_mul,
        );
        config
    }
}

/**
 * This struct contains information to identify a GL geometry shader generated from PICA no-geometry
 * shader pipeline
 */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PicaFixedGSConfig {
    pub state: PicaGSConfigState,
}

impl PicaFixedGSConfig {
    pub fn new(regs: &RegsInternal, use_clip_planes: bool) -> Self {
        let mut config = Self::default();
        config.state.init(regs, use_clip_planes);
        config
    }
}
