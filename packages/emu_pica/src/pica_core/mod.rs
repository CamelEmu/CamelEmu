mod draw;
mod register_write;
mod tables;

use std::cell::RefCell;
use std::rc::Rc;

use emu_pica_types::gsp_interrupt::{InterruptHandler, InterruptId};
use emu_pica_types::{AttributeBuffer, PixelFormat};
use emu_renderer::Rasterizer;

pub use emu_pica_types::pica_regs::{Regs, SharedRegs};
pub use tables::{
    Fog, FogLutEntry, Lighting, LightingLutEntry, ProcTex, ProcTexColorDifferenceEntry,
    ProcTexColorEntry, ProcTexValueEntry,
};

use crate::bits::bits;
use crate::geometry_pipeline::GeometryPipeline;
use crate::memory::PhysicalMemory;
use crate::packed_attribute::PackedAttribute;
use crate::primitive_assembly::PrimitiveAssembler;
use crate::regs::RegsLcd;
use crate::regs::pipeline::TriangleTopology;
use crate::regs::shader::ShaderMode;
use crate::shader::{ShaderEngine, create_engine};
use crate::shader_setup::ShaderSetup;
use crate::shader_unit::GeometryShaderUnit;
use register_write::{
    COLOR_BUFFER_ADDRESS, COMMAND_BUFFER_TRIGGER, COMMAND_BUFFER_TRIGGER_LAST, VIEWPORT_SIZE_Y,
};

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

pub struct PicaCore {
    pub regs_lcd: RegsLcd,
    pub regs: SharedRegs,
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
    pub fn new(regs: SharedRegs, memory: Rc<dyn PhysicalMemory>, use_shader_jit: bool) -> Self {
        let mut core = Self {
            regs_lcd: RegsLcd::default(),
            regs,
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
        let mut regs = self.regs.borrow_mut();
        let [framebuffer_top, framebuffer_sub] = &mut regs.framebuffer_config;

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
        let gs = &mut regs.internal.gs;
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

    pub fn guess_cmd_render_properties(&mut self, list: u32, size: u32) -> RenderPropertiesGuess {
        // Initialize command list tracking.
        let head = self.memory.get_physical_pointer(list);
        self.cmd_list.reset(list, head, size);

        const MAX_ITERATIONS: usize = 0x100;

        let mut find_info = {
            let regs = self.regs.borrow();
            RenderPropertiesGuess {
                vp_height: regs.internal.rasterizer.viewport_size_y(),
                paddr: regs
                    .internal
                    .framebuffer
                    .framebuffer
                    .color_buffer_address()
                    .wrapping_mul(8),
                ..Default::default()
            }
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
                        let regs = core.regs.borrow();
                        let command_buffer = &regs.internal.pipeline.command_buffer;
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
