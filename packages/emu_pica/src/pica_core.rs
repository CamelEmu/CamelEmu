use std::cell::RefCell;
use std::rc::Rc;

use emu_pica_types::gsp_interrupt::{InterruptHandler, InterruptId};
use emu_pica_types::{
    AttributeBuffer, DisplayTransferConfig, FramebufferConfig, MemoryFillConfig, PixelFormat, Vec4,
};
use emu_renderer::Rasterizer;

use crate::bits::{bits, sbits};
use crate::geometry_pipeline::GeometryPipeline;
use crate::memory::PhysicalMemory;
use crate::output_vertex::output_vertex;
use crate::packed_attribute::PackedAttribute;
use crate::pica_reg_index;
use crate::primitive_assembly::PrimitiveAssembler;
use crate::regs::pipeline::{IndexFormat, TriangleTopology, UseGs};
use crate::regs::shader::ShaderMode;
use crate::regs::texturing::ProcTexLutTable;
use crate::regs::{RegsInternal, RegsLcd};
use crate::shader::{ShaderEngine, create_engine};
use crate::shader_setup::ShaderSetup;
use crate::shader_unit::{GeometryShaderUnit, ShaderUnit};
use crate::vertex_loader::VertexLoader;

#[derive(Clone, Copy)]
struct CommandHeader {
    hex: u32,
}

const _: () = assert!(
    size_of::<CommandHeader>() == size_of::<u32>(),
    "CommandHeader has incorrect size!"
);

impl CommandHeader {
    fn cmd_id(self) -> u32 {
        bits(self.hex, 0, 16)
    }

    fn parameter_mask(self) -> u32 {
        bits(self.hex, 16, 4)
    }

    fn extra_data_length(self) -> u32 {
        bits(self.hex, 20, 8)
    }

    fn group_commands(self) -> bool {
        bits(self.hex, 31, 1) != 0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Regs {
    pub hardware_id: u32,
    _padding0: [u32; 0x3],
    pub memory_fill_config: [MemoryFillConfig; 2],
    pub vram_bank_control: u32,
    pub gpu_busy: u32,
    _padding1: [u32; 0x22],
    pub backlight_control: u32,
    _padding2: [u32; 0xCF],
    pub framebuffer_config: [FramebufferConfig; 2],
    _padding3: [u32; 0x180],
    pub display_transfer_config: DisplayTransferConfig,
    _padding4: [u32; 0xF5],
    pub internal: RegsInternal,
    _padding5: [u32; Regs::NUM_REGS - 0x700],
}

const _: () = assert!(size_of::<Regs>() == Regs::NUM_REGS * size_of::<u32>());

impl Default for Regs {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Regs {
    pub const NUM_REGS: usize = 0x732;

    pub fn reg_array(&self) -> &[u32; Self::NUM_REGS] {
        unsafe { &*(self as *const Self).cast() }
    }

    pub fn reg_array_mut(&mut self) -> &mut [u32; Self::NUM_REGS] {
        unsafe { &mut *(self as *mut Self).cast() }
    }
}

#[macro_export]
macro_rules! gpu_reg_index {
    ($($field:tt).+) => {
        ::std::mem::offset_of!($crate::pica_core::Regs, $($field).+) / ::std::mem::size_of::<u32>()
    };
}

struct CommandList {
    addr: u32,
    head: *const u32,
    current_index: u32,
    length: u32,
}

impl Default for CommandList {
    fn default() -> Self {
        Self {
            addr: 0,
            head: std::ptr::null(),
            current_index: 0,
            length: 0,
        }
    }
}

impl CommandList {
    fn reset(&mut self, addr: u32, head: *const u8, size: u32) {
        self.addr = addr;
        self.head = head.cast();
        self.length = size / size_of::<u32>() as u32;
        self.current_index = 0;
    }

    fn read_next(&mut self) -> u32 {
        let value = unsafe { self.head.add(self.current_index as usize).read_unaligned() };
        self.current_index += 1;
        value
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ImmediateModeState {
    pub input_vertex: AttributeBuffer,
    pub current_attribute: u32,
    pub reset_geometry_pipeline: bool,
    pub queue: PackedAttribute,
}

impl Default for ImmediateModeState {
    fn default() -> Self {
        Self {
            input_vertex: AttributeBuffer::default(),
            current_attribute: 0,
            reset_geometry_pipeline: true,
            queue: PackedAttribute::default(),
        }
    }
}

impl ImmediateModeState {
    pub fn reset(&mut self) {
        self.current_attribute = 0;
        self.reset_geometry_pipeline = true;
        self.queue.reset();
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexValueEntry {
    pub raw: u32,
}

impl ProcTexValueEntry {
    // LUT value, encoded as 12-bit fixed point, with 12 fraction bits
    pub fn value(&self) -> u32 {
        bits(self.raw, 0, 12) // 0.0.12 fixed point
    }

    // Difference between two entry values. Used for efficient interpolation.
    // 0.0.12 fixed point with two's complement. The range is [-0.5, 0.5).
    // Note: the type of this is different from the one of lighting LUT
    pub fn difference(&self) -> i32 {
        sbits(self.raw, 12, 12)
    }

    pub fn to_float(&self) -> f32 {
        self.value() as f32 / 4095.0
    }

    pub fn diff_to_float(&self) -> f32 {
        self.difference() as f32 / 4095.0
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexColorEntry {
    pub raw: u32,
}

impl ProcTexColorEntry {
    pub fn r(&self) -> u32 {
        bits(self.raw, 0, 8)
    }

    pub fn g(&self) -> u32 {
        bits(self.raw, 8, 8)
    }

    pub fn b(&self) -> u32 {
        bits(self.raw, 16, 8)
    }

    pub fn a(&self) -> u32 {
        bits(self.raw, 24, 8)
    }

    pub fn to_vector(&self) -> Vec4<u8> {
        Vec4 {
            x: self.r() as u8,
            y: self.g() as u8,
            z: self.b() as u8,
            w: self.a() as u8,
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ProcTexColorDifferenceEntry {
    pub raw: u32,
}

impl ProcTexColorDifferenceEntry {
    pub fn r(&self) -> i32 {
        sbits(self.raw, 0, 8) // half of the difference between two ColorEntry
    }

    pub fn g(&self) -> i32 {
        sbits(self.raw, 8, 8)
    }

    pub fn b(&self) -> i32 {
        sbits(self.raw, 16, 8)
    }

    pub fn a(&self) -> i32 {
        sbits(self.raw, 24, 8)
    }

    pub fn to_vector(&self) -> Vec4<i32> {
        Vec4 {
            x: self.r() * 2,
            y: self.g() * 2,
            z: self.b() * 2,
            w: self.a() * 2,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ProcTex {
    pub noise_table: [ProcTexValueEntry; 128],
    pub color_map_table: [ProcTexValueEntry; 128],
    pub alpha_map_table: [ProcTexValueEntry; 128],
    pub color_table: [ProcTexColorEntry; 256],
    pub color_diff_table: [ProcTexColorDifferenceEntry; 256],
}

impl Default for ProcTex {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct LightingLutEntry {
    // Used for raw access
    pub raw: u32,
}

impl LightingLutEntry {
    // LUT value, encoded as 12-bit fixed point, with 12 fraction bits
    pub fn value(&self) -> u32 {
        bits(self.raw, 0, 12) // 0.0.12 fixed point
    }

    // Used for efficient interpolation.
    pub fn difference(&self) -> u32 {
        bits(self.raw, 12, 11) // 0.0.11 fixed point
    }

    pub fn neg_difference(&self) -> u32 {
        bits(self.raw, 23, 1)
    }

    pub fn to_float(&self) -> f32 {
        self.value() as f32 / 4095.0
    }

    pub fn diff_to_float(&self) -> f32 {
        let diff = self.difference() as f32 / 2047.0;
        if self.neg_difference() != 0 {
            -diff
        } else {
            diff
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Lighting {
    pub luts: [[LightingLutEntry; 256]; 24],
}

impl Default for Lighting {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FogLutEntry {
    // Used for raw access
    pub raw: u32,
}

impl FogLutEntry {
    pub fn difference(&self) -> i32 {
        sbits(self.raw, 0, 13) // 1.1.11 fixed point
    }

    pub fn value(&self) -> u32 {
        bits(self.raw, 13, 11) // 0.0.11 fixed point
    }

    pub fn to_float(&self) -> f32 {
        self.value() as f32 / 2047.0
    }

    pub fn diff_to_float(&self) -> f32 {
        self.difference() as f32 / 2047.0
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Fog {
    pub lut: [FogLutEntry; 128],
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            lut: [FogLutEntry::default(); 128],
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderPropertiesGuess {
    pub vp_height: u32,
    pub paddr: u32,
    pub vp_heigh_found: bool,
    pub paddr_found: bool,
}

/// A vertex output from a shader, held until the shader returns and the core can pass it to the
/// primitive assembler, in the order the handlers of the core receive them.
enum PendingVertex {
    Vertex(Box<AttributeBuffer>),
    Winding,
}

type PendingVertices = Rc<RefCell<Vec<PendingVertex>>>;

const TRIGGER_IRQ: usize = pica_reg_index!(trigger_irq);
const TRIANGLE_TOPOLOGY: usize = pica_reg_index!(pipeline.triangle_topology_raw);
const RESTART_PRIMITIVE: usize = pica_reg_index!(pipeline.restart_primitive);
const VS_DEFAULT_ATTRIBUTES_INDEX: usize =
    pica_reg_index!(pipeline.vs_default_attributes_setup.index);
const VS_DEFAULT_ATTRIBUTES_SET_VALUE: usize =
    pica_reg_index!(pipeline.vs_default_attributes_setup.set_value);
const VS_DEFAULT_ATTRIBUTES_SET_VALUE_LAST: usize = VS_DEFAULT_ATTRIBUTES_SET_VALUE + 2;
const GPU_MODE: usize = pica_reg_index!(pipeline.gpu_mode);
const COMMAND_BUFFER_TRIGGER: usize = pica_reg_index!(pipeline.command_buffer.trigger);
const COMMAND_BUFFER_TRIGGER_LAST: usize = COMMAND_BUFFER_TRIGGER + 1;
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
const VIEWPORT_SIZE_Y: usize = pica_reg_index!(rasterizer.viewport_size_y_raw);
const COLOR_BUFFER_ADDRESS: usize =
    pica_reg_index!(framebuffer.framebuffer.color_buffer_address_raw);

pub struct PicaCore {
    pub regs_lcd: RegsLcd,
    pub regs: Box<Regs>,
    // TODO: Move these to a separate shader scheduler class
    pub gs_unit: Box<GeometryShaderUnit>,
    pub vs_setup: ShaderSetup,
    pub gs_setup: ShaderSetup,
    pub proctex: Box<ProcTex>,
    pub lighting: Box<Lighting>,
    pub fog: Fog,
    pub input_default_attributes: AttributeBuffer,
    pub immediate: ImmediateModeState,
    pub use_hw_shader: bool,

    memory: Rc<dyn PhysicalMemory>,
    signal_interrupt: Option<InterruptHandler>,
    geometry_pipeline: GeometryPipeline,
    primitive_assembler: PrimitiveAssembler,
    cmd_list: CommandList,
    shader_engine: Box<dyn ShaderEngine>,
    pending_vertices: PendingVertices,
}

impl PicaCore {
    pub fn new(memory: Rc<dyn PhysicalMemory>, use_shader_jit: bool) -> Self {
        let mut core = Self {
            regs_lcd: RegsLcd::default(),
            regs: Box::default(),
            gs_unit: GeometryShaderUnit::new(),
            vs_setup: ShaderSetup::new(),
            gs_setup: ShaderSetup::new(),
            proctex: Box::default(),
            lighting: Box::default(),
            fog: Fog::default(),
            input_default_attributes: AttributeBuffer::default(),
            immediate: ImmediateModeState::default(),
            use_hw_shader: true,
            memory,
            signal_interrupt: None,
            geometry_pipeline: GeometryPipeline::new(),
            primitive_assembler: PrimitiveAssembler::default(),
            cmd_list: CommandList::default(),
            shader_engine: create_engine(use_shader_jit),
            pending_vertices: PendingVertices::default(),
        };
        core.initialize_regs();

        let submit_vertex = |pending_vertices: PendingVertices| {
            move |buffer: &AttributeBuffer| {
                pending_vertices
                    .borrow_mut()
                    .push(PendingVertex::Vertex(Box::new(*buffer)));
            }
        };
        let set_winding = {
            let pending_vertices = core.pending_vertices.clone();
            move || pending_vertices.borrow_mut().push(PendingVertex::Winding)
        };

        core.gs_unit.set_vertex_handlers(
            Box::new(submit_vertex(core.pending_vertices.clone())),
            Box::new(set_winding),
        );
        core.geometry_pipeline
            .set_vertex_handler(Box::new(submit_vertex(core.pending_vertices.clone())));

        core.primitive_assembler.reconfigure(TriangleTopology::LIST);
        core
    }

    fn initialize_regs(&mut self) {
        let [framebuffer_top, framebuffer_sub] = &mut self.regs.framebuffer_config;

        // Set framebuffer defaults from nn::gx::Initialize
        framebuffer_top.address_left1 = 0x181E6000;
        framebuffer_top.address_left2 = 0x1822C800;
        framebuffer_top.address_right1 = 0x18273000;
        framebuffer_top.address_right2 = 0x182B9800;
        framebuffer_sub.address_left1 = 0x1848F000;
        framebuffer_sub.address_left2 = 0x184C7800;

        framebuffer_top.set_width(240);
        framebuffer_top.set_height(400);
        framebuffer_top.stride = 3 * 240;
        framebuffer_top.set_color_format(PixelFormat::RGB8);
        framebuffer_top.active_fb = 0;

        framebuffer_sub.set_width(240);
        framebuffer_sub.set_height(320);
        framebuffer_sub.stride = 3 * 240;
        framebuffer_sub.set_color_format(PixelFormat::RGB8);
        framebuffer_sub.active_fb = 0;

        // Tales of Abyss expects this register to have the following default values.
        let gs = &mut self.regs.internal.gs;
        gs.set_max_input_attribute_index(1);
        gs.set_shader_mode(ShaderMode::VS);
    }

    pub fn set_interrupt_handler(&mut self, signal_interrupt: InterruptHandler) {
        self.signal_interrupt = Some(signal_interrupt);
    }

    fn signal_interrupt(&self, id: InterruptId) {
        let signal_interrupt = self
            .signal_interrupt
            .as_ref()
            .expect("the interrupt handler is set before the GPU runs");
        signal_interrupt(id);
    }

    pub fn process_cmd_list(
        &mut self,
        rasterizer: &mut dyn Rasterizer,
        list: u32,
        size: u32,
        ignore_list: bool,
    ) {
        if ignore_list {
            self.signal_interrupt(InterruptId::P3D);
            return;
        }
        // Initialize command list tracking.
        let head = self.memory.get_physical_pointer(list);
        self.cmd_list.reset(list, head, size);

        while self.cmd_list.current_index < self.cmd_list.length {
            // Align read pointer to 8 bytes
            if !self.cmd_list.current_index.is_multiple_of(2) {
                self.cmd_list.current_index += 1;
            }

            // Read the header and the value to write.
            let value = self.cmd_list.read_next();
            let header = CommandHeader {
                hex: self.cmd_list.read_next(),
            };

            // Write to the requested PICA register.
            self.write_internal_reg(rasterizer, header.cmd_id(), value, header.parameter_mask());

            // Write any extra paramters as well.
            for i in 0..header.extra_data_length() {
                let cmd = header.cmd_id() + if header.group_commands() { i + 1 } else { 0 };
                let extra_value = self.cmd_list.read_next();
                self.write_internal_reg(rasterizer, cmd, extra_value, header.parameter_mask());
            }
        }
    }

    fn write_internal_reg(
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
                        proctex.color_diff_table[index % proctex.color_diff_table.len()].raw =
                            value;
                    }
                    _ => {}
                }
                config.set_index(config.index() + 1);
            }

            _ => {}
        }

        // Notify the rasterizer an internal register was updated.
        rasterizer.notify_pica_register_changed(id);
    }

    fn submit_immediate(&mut self, rasterizer: &mut dyn Rasterizer, value: u32) {
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

    fn draw_immediate(&mut self, rasterizer: &mut dyn Rasterizer) {
        // Compile the vertex shader.
        self.shader_engine
            .setup_batch(&mut self.vs_setup, self.regs.internal.vs.main_offset());

        let mut shader_unit = ShaderUnit::default();
        let mut output = AttributeBuffer::default();

        // Invoke the vertex shader for the vertex.
        shader_unit.load_input(&self.regs.internal.vs, &self.immediate.input_vertex);
        self.shader_engine.run(&self.vs_setup, &mut shader_unit);
        shader_unit.write_output(&self.regs.internal.vs, &mut output);

        // Reconfigure geometry pipeline if needed.
        if self.immediate.reset_geometry_pipeline {
            self.geometry_pipeline
                .reconfigure(&self.regs.internal, &mut self.gs_unit);
            self.immediate.reset_geometry_pipeline = false;
        }

        // Send to geometry pipeline.
        assert!(
            !self.geometry_pipeline.need_index_input(),
            "Assertion Failed!"
        );
        self.geometry_pipeline.setup(
            self.shader_engine.as_mut(),
            &self.regs.internal,
            &mut self.gs_setup,
        );
        self.submit_to_geometry_pipeline(rasterizer, &output);

        // Flush the immediate triangle.
        rasterizer.draw_triangles();
        self.immediate.current_attribute = 0;
    }

    fn draw_arrays(&mut self, rasterizer: &mut dyn Rasterizer, is_indexed: bool) {
        let accelerate_draw = {
            // Geometry shaders cannot be accelerated due to register preservation.
            if self.regs.internal.pipeline.use_gs() == UseGs::YES {
                false
            } else {
                // TODO (wwylele): for Strip/Fan topology, if the primitive assember is not restarted
                // after this draw call, the buffered vertex from this draw should "leak" to the next
                // draw, in which case we should buffer the vertex into the software primitive assember,
                // or disable accelerate draw completely. However, there is not game found yet that does
                // this, so this is left unimplemented for now. Revisit this when an issue is found in
                // games.

                let mut accelerate_draw = self.use_hw_shader && self.primitive_assembler.is_empty();
                let topology = self.primitive_assembler.get_topology();
                if topology == TriangleTopology::SHADER || topology == TriangleTopology::LIST {
                    accelerate_draw = accelerate_draw
                        && self.regs.internal.pipeline.num_vertices.is_multiple_of(3);
                }
                accelerate_draw
            }
        };

        // Attempt to use hardware vertex shaders if possible.
        if accelerate_draw && rasterizer.accelerate_draw_batch(is_indexed) {
            return;
        }

        // We cannot accelerate the draw, so load and execute the vertex shader for each vertex.
        self.load_vertices(rasterizer, is_indexed);

        // Draw emitted triangles.
        rasterizer.draw_triangles();
    }

    fn load_vertices(&mut self, rasterizer: &mut dyn Rasterizer, is_indexed: bool) {
        // Read and validate vertex information from the loaders
        let pipeline = self.regs.internal.pipeline;
        let base_address = pipeline.vertex_attributes.get_physical_base_address();
        let memory = self.memory.clone();
        let loader = VertexLoader::new(memory.as_ref(), &pipeline);
        self.regs.internal.rasterizer.validate_semantics();

        // Locate index buffer.
        let index_info = &pipeline.index_array;
        let index_address_8: *const u8 =
            memory.get_physical_pointer(base_address.wrapping_add(index_info.offset()));
        let index_address_16: *const u16 = index_address_8.cast();
        let index_u16 = index_info.format() != IndexFormat::BYTE;

        // Simple circular-replacement vertex cache
        const VERTEX_CACHE_SIZE: usize = 64;
        let mut vertex_cache_valid = [false; VERTEX_CACHE_SIZE];
        let mut vertex_cache_ids = [0u16; VERTEX_CACHE_SIZE];
        let mut vertex_cache = vec![AttributeBuffer::default(); VERTEX_CACHE_SIZE];
        let mut vertex_cache_pos = 0usize;

        // Compile the vertex shader for this batch.
        let mut shader_unit = ShaderUnit::default();
        let mut vs_output = AttributeBuffer::default();
        self.shader_engine
            .setup_batch(&mut self.vs_setup, self.regs.internal.vs.main_offset());

        // Setup geometry pipeline in case we are using a geometry shader.
        self.geometry_pipeline
            .reconfigure(&self.regs.internal, &mut self.gs_unit);
        self.geometry_pipeline.setup(
            self.shader_engine.as_mut(),
            &self.regs.internal,
            &mut self.gs_setup,
        );
        assert!(
            !self.geometry_pipeline.need_index_input() || is_indexed,
            "Assertion Failed!"
        );

        for index in 0..pipeline.num_vertices {
            // Indexed rendering doesn't use the start offset
            let vertex = if is_indexed {
                if index_u16 {
                    u32::from(unsafe { index_address_16.add(index as usize).read_unaligned() })
                } else {
                    u32::from(unsafe { index_address_8.add(index as usize).read() })
                }
            } else {
                index.wrapping_add(pipeline.vertex_offset)
            };

            let mut vertex_cache_hit = false;
            if is_indexed {
                if self.geometry_pipeline.need_index_input() {
                    self.geometry_pipeline.submit_index(
                        &self.regs.internal,
                        &mut self.gs_unit,
                        &mut self.gs_setup,
                        vertex,
                    );
                    continue;
                }

                for i in 0..VERTEX_CACHE_SIZE {
                    if vertex_cache_valid[i] && vertex == u32::from(vertex_cache_ids[i]) {
                        vs_output = vertex_cache[i];
                        vertex_cache_hit = true;
                        break;
                    }
                }
            }

            if !vertex_cache_hit {
                // Initialize data for the current vertex
                let mut input = AttributeBuffer::default();
                loader.load_vertex(
                    base_address,
                    index,
                    vertex,
                    &mut input,
                    &self.input_default_attributes,
                );

                // Invoke the vertex shader for this vertex.
                shader_unit.load_input(&self.regs.internal.vs, &input);
                self.shader_engine.run(&self.vs_setup, &mut shader_unit);
                shader_unit.write_output(&self.regs.internal.vs, &mut vs_output);

                // Cache the vertex when doing indexed rendering.
                if is_indexed {
                    vertex_cache[vertex_cache_pos] = vs_output;
                    vertex_cache_valid[vertex_cache_pos] = true;
                    vertex_cache_ids[vertex_cache_pos] = vertex as u16;
                    vertex_cache_pos = (vertex_cache_pos + 1) % VERTEX_CACHE_SIZE;
                }
            }

            // Send to geometry pipeline
            self.submit_to_geometry_pipeline(rasterizer, &vs_output);
        }
    }

    /// Sends the vertex to the geometry pipeline, then passes the vertices that the handlers
    /// received to the primitive assembler.
    fn submit_to_geometry_pipeline(
        &mut self,
        rasterizer: &mut dyn Rasterizer,
        input: &AttributeBuffer,
    ) {
        self.geometry_pipeline.submit_vertex(
            self.shader_engine.as_ref(),
            &self.regs.internal,
            &mut self.gs_unit,
            &mut self.gs_setup,
            input,
        );

        let pending_vertices = std::mem::take(&mut *self.pending_vertices.borrow_mut());
        for pending_vertex in pending_vertices {
            match pending_vertex {
                PendingVertex::Vertex(buffer) => {
                    let mut add_triangle = |v0: &_, v1: &_, v2: &_| {
                        rasterizer.add_triangle(v0, v1, v2);
                    };
                    let vertex = output_vertex(&self.regs.internal.rasterizer, &buffer);
                    self.primitive_assembler
                        .submit_vertex(&vertex, &mut add_triangle);
                }
                PendingVertex::Winding => self.primitive_assembler.set_winding(),
            }
        }
    }

    pub fn guess_cmd_render_properties(&mut self, list: u32, size: u32) -> RenderPropertiesGuess {
        // Initialize command list tracking.
        let head = self.memory.get_physical_pointer(list);
        self.cmd_list.reset(list, head, size);

        const MAX_ITERATIONS: usize = 0x100;

        let mut find_info = RenderPropertiesGuess {
            vp_height: self.regs.internal.rasterizer.viewport_size_y(),
            paddr: self
                .regs
                .internal
                .framebuffer
                .framebuffer
                .color_buffer_address()
                .wrapping_mul(8),
            ..Default::default()
        };

        let process_write =
            |core: &mut Self, find_info: &mut RenderPropertiesGuess, cmd_id: u32, value: u32| {
                match cmd_id as usize {
                    VIEWPORT_SIZE_Y => {
                        find_info.vp_height = value;
                        find_info.vp_heigh_found = true;
                    }
                    COLOR_BUFFER_ADDRESS => {
                        find_info.paddr = value.wrapping_mul(8);
                        find_info.paddr_found = true;
                    }
                    COMMAND_BUFFER_TRIGGER..=COMMAND_BUFFER_TRIGGER_LAST => {
                        let index = (cmd_id as usize - COMMAND_BUFFER_TRIGGER) as u32;
                        let command_buffer = &core.regs.internal.pipeline.command_buffer;
                        let addr = command_buffer.get_physical_address(index);
                        let size = command_buffer.get_size(index);
                        let head = core.memory.get_physical_pointer(addr);
                        core.cmd_list.reset(addr, head, size);
                    }
                    _ => {}
                }
                find_info.vp_heigh_found && find_info.paddr_found
            };

        let mut iterations = 0;
        while self.cmd_list.current_index < self.cmd_list.length && iterations < MAX_ITERATIONS {
            // Align read pointer to 8 bytes
            if !self.cmd_list.current_index.is_multiple_of(2) {
                self.cmd_list.current_index += 1;
            }

            // Read the header and the value to write.
            let value = self.cmd_list.read_next();
            let header = CommandHeader {
                hex: self.cmd_list.read_next(),
            };

            // Write to the requested PICA register.
            if process_write(self, &mut find_info, header.cmd_id(), value) {
                break;
            }

            // Write any extra paramters as well.
            for i in 0..header.extra_data_length() {
                let cmd = header.cmd_id() + if header.group_commands() { i + 1 } else { 0 };
                let extra_value = self.cmd_list.read_next();
                if process_write(self, &mut find_info, cmd, extra_value) {
                    break;
                }
            }

            iterations += 1;
        }

        find_info
    }
}
