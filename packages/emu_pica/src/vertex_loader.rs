use emu_pica_types::{AttributeBuffer, F24};

use crate::memory::PhysicalMemory;
use crate::regs::PipelineRegs;
use crate::regs::pipeline::VertexAttributeFormat;
use crate::vector::component_mut;

fn align_up(value: u32, size: u32) -> u32 {
    value.div_ceil(size) * size
}

pub struct VertexLoader<'a> {
    memory: &'a dyn PhysicalMemory,
    vertex_attribute_sources: [u32; 16],
    vertex_attribute_strides: [u32; 16],
    vertex_attribute_formats: [VertexAttributeFormat; 16],
    vertex_attribute_elements: [u32; 16],
    vertex_attribute_is_default: [bool; 16],
    num_total_attributes: i32,
}

trait AttributeComponent: Copy {
    fn to_f32(self) -> f32;
}

impl AttributeComponent for i8 {
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
}

impl AttributeComponent for u8 {
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
}

impl AttributeComponent for i16 {
    fn to_f32(self) -> f32 {
        f32::from(self)
    }
}

impl AttributeComponent for f32 {
    fn to_f32(self) -> f32 {
        self
    }
}

impl<'a> VertexLoader<'a> {
    pub fn new(memory: &'a dyn PhysicalMemory, regs: &PipelineRegs) -> Self {
        let attribute_config = &regs.vertex_attributes;
        let mut loader = Self {
            memory,
            vertex_attribute_sources: [0xdeadbeef; 16],
            vertex_attribute_strides: [0; 16],
            vertex_attribute_formats: [VertexAttributeFormat::default(); 16],
            vertex_attribute_elements: [0; 16],
            vertex_attribute_is_default: [false; 16],
            num_total_attributes: attribute_config.get_num_total_attributes() as i32,
        };

        for i in 0..16 {
            loader.vertex_attribute_is_default[i] = attribute_config.is_default_attribute(i);
        }

        // Setup attribute data from loaders
        for loader_index in 0..12 {
            let loader_config = &attribute_config.attribute_loaders[loader_index];

            let mut offset = 0u32;

            // TODO: What happens if a loader overwrites a previous one's data?
            for component in 0..loader_config.component_count() {
                if component >= 12 {
                    log::error!(
                        target: "HW_GPU",
                        "Overflow in the vertex attribute loader {} trying to load component {}",
                        loader_index,
                        component
                    );
                    continue;
                }

                let attribute_index = loader_config.get_component(component as usize);
                if attribute_index < 12 {
                    let attribute = attribute_index as usize;
                    offset = align_up(
                        offset,
                        attribute_config.get_element_size_in_bytes(attribute),
                    );
                    loader.vertex_attribute_sources[attribute] =
                        loader_config.data_offset().wrapping_add(offset);
                    loader.vertex_attribute_strides[attribute] = loader_config.byte_count();
                    loader.vertex_attribute_formats[attribute] =
                        attribute_config.get_format(attribute);
                    loader.vertex_attribute_elements[attribute] =
                        attribute_config.get_num_elements(attribute);
                    offset += attribute_config.get_stride(attribute);
                } else if attribute_index < 16 {
                    // Attribute ids 12, 13, 14 and 15 signify 4, 8, 12 and 16-byte paddings,
                    // respectively
                    offset = align_up(offset, 4);
                    offset += (attribute_index - 11) * 4;
                } else {
                    // This is truly unreachable due to the number of bits for each
                    // component
                    unreachable!();
                }
            }
        }
        loader
    }

    fn load_attribute<T: AttributeComponent>(
        &self,
        source_addr: u32,
        attrib: usize,
        out: &mut AttributeBuffer,
    ) {
        let data = self.memory.get_physical_pointer(source_addr).cast::<T>();
        for comp in 0..self.vertex_attribute_elements[attrib] as usize {
            let value = unsafe { data.add(comp).read_unaligned() };
            *component_mut(&mut out[attrib], comp) = F24::from_float32(value.to_f32());
        }
    }

    pub fn load_vertex(
        &self,
        base_address: u32,
        _index: u32,
        vertex: u32,
        input: &mut AttributeBuffer,
        input_default_attributes: &AttributeBuffer,
    ) {
        for i in 0..self.num_total_attributes.max(0) as usize {
            // Load the default attribute if we're configured to do so
            if self.vertex_attribute_is_default[i] {
                input[i] = input_default_attributes[i];
                continue;
            }

            // TODO(yuriks): In this case, no data gets loaded and the vertex
            // remains with the last value it had. This isn't currently maintained
            // as global state, however, and so won't work in Citra yet.
            if self.vertex_attribute_elements[i] == 0 {
                log::error!(target: "HW_GPU", "Vertex retension unimplemented");
                continue;
            }

            // Load per-vertex data from the loader arrays
            let source_addr = base_address
                .wrapping_add(self.vertex_attribute_sources[i])
                .wrapping_add(self.vertex_attribute_strides[i].wrapping_mul(vertex));

            match self.vertex_attribute_formats[i] {
                VertexAttributeFormat::BYTE => self.load_attribute::<i8>(source_addr, i, input),
                VertexAttributeFormat::UBYTE => self.load_attribute::<u8>(source_addr, i, input),
                VertexAttributeFormat::SHORT => self.load_attribute::<i16>(source_addr, i, input),
                VertexAttributeFormat::FLOAT => self.load_attribute::<f32>(source_addr, i, input),
                _ => {}
            }

            // Default attribute values set if array elements have < 4 components. This
            // is *not* carried over from the default attribute settings even if they're
            // enabled for this attribute.
            for comp in self.vertex_attribute_elements[i] as usize..4 {
                *component_mut(&mut input[i], comp) =
                    if comp == 3 { F24::one() } else { F24::zero() };
            }
        }
    }

    pub fn get_num_total_attributes(&self) -> i32 {
        self.num_total_attributes
    }
}
