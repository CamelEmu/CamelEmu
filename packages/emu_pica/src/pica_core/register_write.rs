use emu_pica_types::gsp_interrupt::InterruptId;
use emu_renderer::Rasterizer;

use super::PicaCore;
use crate::pica_reg_index;
use crate::regs::RegsInternal;
use crate::regs::pipeline::UseGs;
use crate::regs::texturing::ProcTexLutTable;

const TRIGGER_IRQ: usize = pica_reg_index!(trigger_irq);
const TRIANGLE_TOPOLOGY: usize = pica_reg_index!(pipeline.triangle_topology_raw);
const RESTART_PRIMITIVE: usize = pica_reg_index!(pipeline.restart_primitive);
const VS_DEFAULT_ATTRIBUTES_INDEX: usize =
    pica_reg_index!(pipeline.vs_default_attributes_setup.index);
const VS_DEFAULT_ATTRIBUTES_SET_VALUE: usize =
    pica_reg_index!(pipeline.vs_default_attributes_setup.set_value);
const VS_DEFAULT_ATTRIBUTES_SET_VALUE_LAST: usize = VS_DEFAULT_ATTRIBUTES_SET_VALUE + 2;
const GPU_MODE: usize = pica_reg_index!(pipeline.gpu_mode);
pub(super) const COMMAND_BUFFER_TRIGGER: usize = pica_reg_index!(pipeline.command_buffer.trigger);
pub(super) const COMMAND_BUFFER_TRIGGER_LAST: usize = COMMAND_BUFFER_TRIGGER + 1;
const TRIGGER_DRAW: usize = pica_reg_index!(pipeline.trigger_draw);
const TRIGGER_DRAW_INDEXED: usize = pica_reg_index!(pipeline.trigger_draw_indexed);
const GS_BOOL_UNIFORMS: usize = pica_reg_index!(gs.bool_uniforms_raw);
const GS_INT_UNIFORMS: usize = pica_reg_index!(gs.int_uniforms);
const GS_INT_UNIFORMS_LAST: usize = GS_INT_UNIFORMS + 3;
const GS_UNIFORM_SET_VALUE: usize = pica_reg_index!(gs.uniform_setup.set_value);
const GS_UNIFORM_SET_VALUE_LAST: usize = GS_UNIFORM_SET_VALUE + 7;
const GS_PROGRAM_SET_WORD: usize = pica_reg_index!(gs.program.set_word);
const GS_PROGRAM_SET_WORD_LAST: usize = GS_PROGRAM_SET_WORD + 7;
const GS_SWIZZLE_SET_WORD: usize = pica_reg_index!(gs.swizzle_patterns.set_word);
const GS_SWIZZLE_SET_WORD_LAST: usize = GS_SWIZZLE_SET_WORD + 7;
const VS_OUTPUT_MASK: usize = pica_reg_index!(vs.output_mask_raw);
const VS_BOOL_UNIFORMS: usize = pica_reg_index!(vs.bool_uniforms_raw);
const VS_INT_UNIFORMS: usize = pica_reg_index!(vs.int_uniforms);
const VS_INT_UNIFORMS_LAST: usize = VS_INT_UNIFORMS + 3;
const VS_UNIFORM_SET_VALUE: usize = pica_reg_index!(vs.uniform_setup.set_value);
const VS_UNIFORM_SET_VALUE_LAST: usize = VS_UNIFORM_SET_VALUE + 7;
const VS_PROGRAM_SET_WORD: usize = pica_reg_index!(vs.program.set_word);
const VS_PROGRAM_SET_WORD_LAST: usize = VS_PROGRAM_SET_WORD + 7;
const VS_SWIZZLE_SET_WORD: usize = pica_reg_index!(vs.swizzle_patterns.set_word);
const VS_SWIZZLE_SET_WORD_LAST: usize = VS_SWIZZLE_SET_WORD + 7;
const LIGHTING_LUT_DATA: usize = pica_reg_index!(lighting.lut_data);
const LIGHTING_LUT_DATA_LAST: usize = LIGHTING_LUT_DATA + 7;
const FOG_LUT_DATA: usize = pica_reg_index!(texturing.fog_lut_data);
const FOG_LUT_DATA_LAST: usize = FOG_LUT_DATA + 7;
const PROCTEX_LUT_DATA: usize = pica_reg_index!(texturing.proctex_lut_data);
const PROCTEX_LUT_DATA_LAST: usize = PROCTEX_LUT_DATA + 7;
pub(super) const VIEWPORT_SIZE_Y: usize = pica_reg_index!(rasterizer.viewport_size_y_raw);
pub(super) const COLOR_BUFFER_ADDRESS: usize =
    pica_reg_index!(framebuffer.framebuffer.color_buffer_address_raw);

impl PicaCore {
    pub(super) fn write_internal_reg(
        &mut self,
        rasterizer: &mut dyn Rasterizer,
        id: u32,
        value: u32,
        mask: u32,
    ) {
        if id as usize >= RegsInternal::NUM_REGS {
            log::error!(
                target: "HW_GPU",
                "Commandlist tried to write to invalid register 0x{:03X} (value: {:08X}, mask: {:X})",
                id,
                value,
                mask
            );
            return;
        }

        // Expand a 4-bit mask to 4-byte mask, e.g. 0b0101 -> 0x00FF00FF
        const EXPAND_BITS_TO_BYTES: [u32; 16] = [
            0x00000000, 0x000000ff, 0x0000ff00, 0x0000ffff, 0x00ff0000, 0x00ff00ff, 0x00ffff00,
            0x00ffffff, 0xff000000, 0xff0000ff, 0xff00ff00, 0xff00ffff, 0xffff0000, 0xffff00ff,
            0xffffff00, 0xffffffff,
        ];

        // TODO: Figure out how register masking acts on e.g. vs.uniform_setup.set_value
        let reg_array = self.regs.internal.reg_array_mut();
        let old_value = reg_array[id as usize];
        let write_mask = EXPAND_BITS_TO_BYTES[mask as usize];
        reg_array[id as usize] = (old_value & !write_mask) | (value & write_mask);

        let id_index = id as usize;
        match id_index {
            // Trigger IRQ
            TRIGGER_IRQ => {
                self.signal_interrupt(InterruptId::P3D);
            }

            TRIANGLE_TOPOLOGY => {
                self.primitive_assembler
                    .reconfigure(self.regs.internal.pipeline.triangle_topology());
            }

            RESTART_PRIMITIVE => {
                self.primitive_assembler.reset();
            }

            VS_DEFAULT_ATTRIBUTES_INDEX => {
                self.immediate.reset();
            }

            // Load default vertex input attributes
            VS_DEFAULT_ATTRIBUTES_SET_VALUE..=VS_DEFAULT_ATTRIBUTES_SET_VALUE_LAST => {
                self.submit_immediate(rasterizer, value);
            }

            GPU_MODE => {
                // This register likely just enables vertex processing and doesn't need any special handling
            }

            COMMAND_BUFFER_TRIGGER..=COMMAND_BUFFER_TRIGGER_LAST => {
                let index = (id_index - COMMAND_BUFFER_TRIGGER) as u32;
                let command_buffer = &self.regs.internal.pipeline.command_buffer;
                let addr = command_buffer.get_physical_address(index);
                let size = command_buffer.get_size(index);
                let head = self.memory.get_physical_pointer(addr);
                self.cmd_list.reset(addr, head, size);
            }

            // It seems like these trigger vertex rendering
            TRIGGER_DRAW | TRIGGER_DRAW_INDEXED => {
                let is_indexed = id_index == TRIGGER_DRAW_INDEXED;
                self.draw_arrays(rasterizer, is_indexed);
            }

            GS_BOOL_UNIFORMS..=GS_SWIZZLE_SET_WORD_LAST => {
                self.write_gs_reg(id_index, value);
            }

            VS_BOOL_UNIFORMS..=VS_SWIZZLE_SET_WORD_LAST => {
                self.write_vs_reg(id_index, value);
            }

            LIGHTING_LUT_DATA..=LIGHTING_LUT_DATA_LAST => {
                let lut_config = &mut self.regs.internal.lighting.lut_config;
                assert!(
                    lut_config.index() < 256,
                    "lut_config.index exceeded maximum value of 255!"
                );

                self.lighting.luts[lut_config.lut_type() as usize][lut_config.index() as usize]
                    .raw = value;
                lut_config.set_index(lut_config.index() + 1);
            }

            FOG_LUT_DATA..=FOG_LUT_DATA_LAST => {
                let texturing = &mut self.regs.internal.texturing;
                self.fog.lut[(texturing.fog_lut_offset() % 128) as usize].raw = value;
                texturing.set_fog_lut_offset(texturing.fog_lut_offset() + 1);
            }

            PROCTEX_LUT_DATA..=PROCTEX_LUT_DATA_LAST => {
                self.write_proctex_lut(value);
            }

            _ => {}
        }

        // Notify the rasterizer an internal register was updated.
        rasterizer.notify_pica_register_changed(id);
    }

    /// Handles the writes to the registers of the geometry shader.
    fn write_gs_reg(&mut self, id_index: usize, value: u32) {
        match id_index {
            GS_BOOL_UNIFORMS => {
                self.gs_setup
                    .write_uniform_bool_reg(self.regs.internal.gs.bool_uniforms());
            }

            GS_INT_UNIFORMS..=GS_INT_UNIFORMS_LAST => {
                let index = (id_index - GS_INT_UNIFORMS) as u32;
                self.gs_setup
                    .write_uniform_int_reg(index, self.regs.internal.gs.get_int_uniform(index));
            }

            GS_UNIFORM_SET_VALUE..=GS_UNIFORM_SET_VALUE_LAST => {
                self.gs_setup
                    .write_uniform_float_reg(&mut self.regs.internal.gs, value);
            }

            GS_PROGRAM_SET_WORD..=GS_PROGRAM_SET_WORD_LAST => {
                let offset = &mut self.regs.internal.gs.program.offset;
                if *offset >= 4096 {
                    log::error!(target: "HW_GPU", "Invalid GS program offset {}", *offset);
                } else {
                    self.gs_setup.program_code[*offset as usize] = value;
                    self.gs_setup.mark_program_code_dirty();
                    *offset += 1;
                }
            }

            GS_SWIZZLE_SET_WORD..=GS_SWIZZLE_SET_WORD_LAST => {
                let offset = &mut self.regs.internal.gs.swizzle_patterns.offset;
                if *offset as usize >= self.gs_setup.swizzle_data.len() {
                    log::error!(target: "HW_GPU", "Invalid GS swizzle pattern offset {}", *offset);
                } else {
                    self.gs_setup.swizzle_data[*offset as usize] = value;
                    self.gs_setup.mark_swizzle_data_dirty();
                    *offset += 1;
                }
            }

            _ => {}
        }
    }

    /// Handles the writes to the registers of the vertex shader.
    fn write_vs_reg(&mut self, id_index: usize, value: u32) {
        match id_index {
            VS_OUTPUT_MASK => {
                let internal = &mut self.regs.internal;
                if internal.pipeline.gs_unit_exclusive_configuration() == 0
                    && internal.pipeline.use_gs() == UseGs::NO
                {
                    internal.gs.set_output_mask(value);
                }
            }

            VS_BOOL_UNIFORMS => {
                let internal = &self.regs.internal;
                self.vs_setup
                    .write_uniform_bool_reg(internal.vs.bool_uniforms());
                if internal.pipeline.gs_unit_exclusive_configuration() == 0
                    && internal.pipeline.use_gs() == UseGs::NO
                {
                    self.gs_setup
                        .write_uniform_bool_reg(internal.vs.bool_uniforms());
                }
            }

            VS_INT_UNIFORMS..=VS_INT_UNIFORMS_LAST => {
                let index = (id_index - VS_INT_UNIFORMS) as u32;
                let internal = &self.regs.internal;
                self.vs_setup
                    .write_uniform_int_reg(index, internal.vs.get_int_uniform(index));
                if internal.pipeline.gs_unit_exclusive_configuration() == 0
                    && internal.pipeline.use_gs() == UseGs::NO
                {
                    self.gs_setup
                        .write_uniform_int_reg(index, internal.vs.get_int_uniform(index));
                }
            }

            VS_UNIFORM_SET_VALUE..=VS_UNIFORM_SET_VALUE_LAST => {
                let internal = &mut self.regs.internal;
                let index = self
                    .vs_setup
                    .write_uniform_float_reg(&mut internal.vs, value);
                if internal.pipeline.gs_unit_exclusive_configuration() == 0
                    && internal.pipeline.use_gs() == UseGs::NO
                    && let Some(index) = index
                {
                    self.gs_setup.uniforms.f[index as usize] =
                        self.vs_setup.uniforms.f[index as usize];
                }
            }

            VS_PROGRAM_SET_WORD..=VS_PROGRAM_SET_WORD_LAST => {
                let internal = &mut self.regs.internal;
                let gs_unit_exclusive_configuration =
                    internal.pipeline.gs_unit_exclusive_configuration();
                let offset = &mut internal.vs.program.offset;
                if *offset >= 512 {
                    log::error!(target: "HW_GPU", "Invalid VS program offset {}", *offset);
                } else {
                    self.vs_setup.program_code[*offset as usize] = value;
                    self.vs_setup.mark_program_code_dirty();
                    if gs_unit_exclusive_configuration == 0 {
                        self.gs_setup.program_code[*offset as usize] = value;
                        self.gs_setup.mark_program_code_dirty();
                    }
                    *offset += 1;
                }
            }

            VS_SWIZZLE_SET_WORD..=VS_SWIZZLE_SET_WORD_LAST => {
                let internal = &mut self.regs.internal;
                let gs_unit_exclusive_configuration =
                    internal.pipeline.gs_unit_exclusive_configuration();
                let offset = &mut internal.vs.swizzle_patterns.offset;
                if *offset as usize >= self.vs_setup.swizzle_data.len() {
                    log::error!(target: "HW_GPU", "Invalid VS swizzle pattern offset {}", *offset);
                } else {
                    self.vs_setup.swizzle_data[*offset as usize] = value;
                    self.vs_setup.mark_swizzle_data_dirty();
                    if gs_unit_exclusive_configuration == 0 {
                        self.gs_setup.swizzle_data[*offset as usize] = value;
                        self.gs_setup.mark_swizzle_data_dirty();
                    }
                    *offset += 1;
                }
            }

            _ => {}
        }
    }

    /// Handles the writes to the data of the procedural texture lookup tables.
    fn write_proctex_lut(&mut self, value: u32) {
        let config = &mut self.regs.internal.texturing.proctex_lut_config;
        let index = config.index() as usize;
        let proctex = &mut self.proctex;

        match config.ref_table() {
            ProcTexLutTable::NOISE => {
                proctex.noise_table[index % proctex.noise_table.len()].raw = value;
            }
            ProcTexLutTable::COLOR_MAP => {
                proctex.color_map_table[index % proctex.color_map_table.len()].raw = value;
            }
            ProcTexLutTable::ALPHA_MAP => {
                proctex.alpha_map_table[index % proctex.alpha_map_table.len()].raw = value;
            }
            ProcTexLutTable::COLOR => {
                proctex.color_table[index % proctex.color_table.len()].raw = value;
            }
            ProcTexLutTable::COLOR_DIFF => {
                proctex.color_diff_table[index % proctex.color_diff_table.len()].raw = value;
            }
            _ => {}
        }
        config.set_index(config.index() + 1);
    }

    pub(super) fn submit_immediate(&mut self, rasterizer: &mut dyn Rasterizer, value: u32) {
        // Push to word to the queue. This returns true when a full attribute is formed.
        if !self.immediate.queue.push(value, false) {
            return;
        }

        const IMMEDIATE_MODE_INDEX: u32 = 0xF;

        let setup = &mut self.regs.internal.pipeline.vs_default_attributes_setup;
        if setup.index > IMMEDIATE_MODE_INDEX {
            log::error!(target: "HW_GPU", "Invalid VS default attribute index {}", setup.index);
            return;
        }

        // Retrieve the attribute and place it in the default attribute buffer.
        let attribute = self.immediate.queue.get(false);
        if setup.index < IMMEDIATE_MODE_INDEX {
            self.input_default_attributes[setup.index as usize] = attribute;
            setup.index += 1;
            return;
        }

        // When index is 0xF the attribute is used for immediate mode drawing.
        self.immediate.input_vertex[self.immediate.current_attribute as usize] = attribute;
        if self.immediate.current_attribute < self.regs.internal.pipeline.max_input_attrib_index() {
            self.immediate.current_attribute += 1;
            return;
        }

        // We formed a vertex, flush.
        self.draw_immediate(rasterizer);
    }
}
