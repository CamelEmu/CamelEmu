use crate::bits::{bits, pica_enum};

pica_enum!(VertexAttributeFormat {
    BYTE = 0,
    UBYTE = 1,
    SHORT = 2,
    FLOAT = 3,
});

pub fn get_format_bytes(format: VertexAttributeFormat) -> u32 {
    match format {
        VertexAttributeFormat::FLOAT => 4,
        VertexAttributeFormat::SHORT => 2,
        VertexAttributeFormat::BYTE | VertexAttributeFormat::UBYTE => 1,
        _ => unreachable!(),
    }
}

// Attribute loaders map the source vertex data to input attributes
// This e.g. allows to load different attributes from different memory locations
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AttributeLoader {
    // Source attribute data offset from the base address
    pub data_offset_raw: u32,
    pub components_low: u32,
    pub components_high: u32,
}

impl AttributeLoader {
    pub fn data_offset(&self) -> u32 {
        bits(self.data_offset_raw, 0, 28)
    }

    // bytes for a single vertex in this loader
    pub fn byte_count(&self) -> u32 {
        bits(self.components_high, 16, 8)
    }

    pub fn component_count(&self) -> u32 {
        bits(self.components_high, 28, 4)
    }

    pub fn get_component(&self, n: usize) -> u32 {
        if n < 8 {
            bits(self.components_low, 4 * n as u32, 4)
        } else {
            bits(self.components_high, 4 * (n as u32 - 8), 4)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VertexAttributes {
    pub base_address_raw: u32,

    // Descriptor for internal vertex attributes
    pub formats_low: u32,
    pub formats_high: u32,

    pub attribute_loaders: [AttributeLoader; 12],
}

impl VertexAttributes {
    pub fn base_address(&self) -> u32 {
        bits(self.base_address_raw, 1, 28)
    }

    pub fn get_physical_base_address(&self) -> u32 {
        self.base_address().wrapping_mul(16)
    }

    pub fn attribute_mask(&self) -> u32 {
        bits(self.formats_high, 16, 12)
    }

    // number of total attributes minus 1
    pub fn max_attribute_index(&self) -> u32 {
        bits(self.formats_high, 28, 4)
    }

    pub fn get_format(&self, n: usize) -> VertexAttributeFormat {
        // size of one element
        if n < 8 {
            VertexAttributeFormat(bits(self.formats_low, 4 * n as u32, 2))
        } else {
            VertexAttributeFormat(bits(self.formats_high, 4 * (n as u32 - 8), 2))
        }
    }

    pub fn get_num_elements(&self, n: usize) -> u32 {
        // number of elements minus 1
        let size = if n < 8 {
            bits(self.formats_low, 4 * n as u32 + 2, 2)
        } else {
            bits(self.formats_high, 4 * (n as u32 - 8) + 2, 2)
        };
        size + 1
    }

    pub fn get_element_size_in_bytes(&self, n: usize) -> u32 {
        if self.get_format(n) == VertexAttributeFormat::FLOAT {
            4
        } else if self.get_format(n) == VertexAttributeFormat::SHORT {
            2
        } else {
            1
        }
    }

    pub fn get_stride(&self, n: usize) -> u32 {
        self.get_num_elements(n) * self.get_element_size_in_bytes(n)
    }

    pub fn is_default_attribute(&self, id: usize) -> bool {
        id >= 12 || (u64::from(self.attribute_mask()) & (1u64 << id)) != 0
    }

    pub fn get_num_total_attributes(&self) -> u32 {
        self.max_attribute_index() + 1
    }
}

pica_enum!(IndexFormat {
    BYTE = 0,
    SHORT = 1,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct IndexArray {
    pub raw: u32,
}

impl IndexArray {
    pub fn offset(&self) -> u32 {
        bits(self.raw, 0, 28) // relative to base attribute address
    }

    pub fn unused(&self) -> u32 {
        bits(self.raw, 28, 3)
    }

    pub fn format(&self) -> IndexFormat {
        IndexFormat(bits(self.raw, 31, 1))
    }
}

pica_enum!(UseGs {
    NO = 0,
    YES = 2,
});

// These registers are used to setup the default "fall-back" vertex shader attributes
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VsDefaultAttributesSetup {
    // Index of the current default attribute
    pub index: u32,

    // Writing to these registers sets the "current" default attribute.
    pub set_value: [u32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CommandBuffer {
    // There are two channels that can be used to configure the next command buffer, which can
    // be then executed by writing to the "trigger" registers. There are two reasons why a game
    // might use this feature:
    //  1) With this, an arbitrary number of additional command buffers may be executed in
    //     sequence without requiring any intervention of the CPU after the initial one is
    //     kicked off.
    //  2) Games can configure these registers to provide a command list subroutine mechanism.

    // TODO: verify the bit length of these two fields
    // According to 3dbrew, the bit length of them are 21 and 29, respectively
    pub size_raw: [u32; 2], // Size (in bytes / 8) of each channel's command buffer
    pub addr_raw: [u32; 2], // Physical address / 8 of each channel's command buffer
    pub trigger: [u32; 2],  // Triggers execution of the channel's command buffer when written to
}

impl CommandBuffer {
    pub fn get_size(&self, index: u32) -> u32 {
        assert!(index < 2, "Assertion Failed!");
        8u32.wrapping_mul(bits(self.size_raw[index as usize], 0, 20))
    }

    pub fn get_physical_address(&self, index: u32) -> u32 {
        assert!(index < 2, "Assertion Failed!");
        8u32.wrapping_mul(bits(self.addr_raw[index as usize], 0, 28))
    }
}

pica_enum!(GpuMode {
    DRAWING = 0,
    CONFIGURING = 1,
});

pica_enum!(GsMode {
    POINT = 0,
    VARIABLE_PRIMITIVE = 1,
    FIXED_PRIMITIVE = 2,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GsConfig {
    pub raw: u32,
}

impl GsConfig {
    pub fn mode(&self) -> GsMode {
        GsMode(bits(self.raw, 0, 8))
    }

    pub fn fixed_vertex_num_minus_1(&self) -> u32 {
        bits(self.raw, 8, 4)
    }

    pub fn stride_minus_1(&self) -> u32 {
        bits(self.raw, 12, 4)
    }

    pub fn start_index(&self) -> u32 {
        bits(self.raw, 16, 4)
    }
}

pica_enum!(TriangleTopology {
    LIST = 0,
    STRIP = 1,
    FAN = 2,
    SHADER = 3, // Programmable setup unit implemented in a geometry shader
});

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct PipelineRegs {
    pub vertex_attributes: VertexAttributes,

    pub index_array: IndexArray,

    // Number of vertices to render
    pub num_vertices: u32,

    pub use_gs_raw: u32,

    // The index of the first vertex to render
    pub vertex_offset: u32,

    _padding0: [u32; 0x3],

    // These two trigger rendering of triangles
    pub trigger_draw: u32,
    pub trigger_draw_indexed: u32,

    _padding1: [u32; 0x2],

    pub vs_default_attributes_setup: VsDefaultAttributesSetup,

    _padding2: [u32; 0x2],

    pub command_buffer: CommandBuffer,

    _padding3: [u32; 4],

    /// Number of input attributes to the vertex shader minus 1
    pub max_input_attrib_index_raw: u32,

    _padding4: [u32; 1],

    // The shader unit 3, which can be used for both vertex and geometry shader, gets its
    // configuration depending on this register. If this is not set, unit 3 will share some
    // configuration with other units. It is known that program code and swizzle pattern uploaded
    // via regs.vs will be also uploaded to unit 3 if this is not set. Although very likely, it is
    // still unclear whether uniforms and other configuration can be also shared.
    pub gs_unit_exclusive_configuration_raw: u32,

    pub gpu_mode: GpuMode,

    _padding5: [u32; 0x4],
    pub vs_outmap_total_minus_1_a_raw: u32,
    _padding6: [u32; 0x6],
    pub vs_outmap_total_minus_1_b_raw: u32,

    pub gs_config: GsConfig,

    _padding7: [u32; 0x1],

    pub variable_vertex_main_num_minus_1: u32,

    _padding8: [u32; 0x9],

    pub triangle_topology_raw: u32,

    pub restart_primitive: u32,

    _padding9: [u32; 0x20],
}

const _: () = assert!(
    size_of::<PipelineRegs>() == 0x80 * size_of::<u32>(),
    "PipelineRegs struct has incorrect size"
);

impl PipelineRegs {
    pub fn use_gs(&self) -> UseGs {
        UseGs(bits(self.use_gs_raw, 0, 2))
    }

    pub fn variable_primitive(&self) -> u32 {
        bits(self.use_gs_raw, 31, 1)
    }

    pub fn max_input_attrib_index(&self) -> u32 {
        bits(self.max_input_attrib_index_raw, 0, 4)
    }

    pub fn gs_unit_exclusive_configuration(&self) -> u32 {
        bits(self.gs_unit_exclusive_configuration_raw, 0, 1)
    }

    pub fn vs_outmap_total_minus_1_a(&self) -> u32 {
        bits(self.vs_outmap_total_minus_1_a_raw, 0, 4)
    }

    pub fn vs_outmap_total_minus_1_b(&self) -> u32 {
        bits(self.vs_outmap_total_minus_1_b_raw, 0, 4)
    }

    pub fn triangle_topology(&self) -> TriangleTopology {
        TriangleTopology(bits(self.triangle_topology_raw, 8, 2))
    }
}
