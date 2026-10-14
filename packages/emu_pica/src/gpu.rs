use std::mem::offset_of;
use std::rc::Rc;

use emu_pica_types::MemoryFillConfig;
use emu_pica_types::gsp_command::{Command, CommandId};
use emu_pica_types::gsp_gpu::FrameBufferInfo;
use emu_pica_types::gsp_interrupt::{InterruptHandler, InterruptId};
use emu_pica_types::hacks::{HACK_MANAGER, HackAllowMode, HackType};
use emu_renderer::Renderer;

use crate::bits::assign;
use crate::gpu_reg_index;
use crate::memory::PhysicalMemory;
use crate::pica_core::{PicaCore, Regs, SharedRegs};
use crate::regs::RegsLcd;
use crate::regs::lcd::ColorFill;
use crate::right_eye_disabler::RightEyeDisabler;
use crate::sw_blitter::SwBlitter;

/// Measured on hardware to be 2240568 timer cycles or 4481136 ARM11 cycles
pub const FRAME_TICKS: u64 = 4481136;

const VADDR_LCD: u32 = 0x1ED02000;
const VADDR_GPU: u32 = 0x1EF00000;

const VRAM_PADDR: u32 = 0x18000000;
const VRAM_SIZE: u32 = 0x00600000;
const FCRAM_PADDR: u32 = 0x20000000;
const LINEAR_HEAP_VADDR: u32 = 0x14000000;
const LINEAR_HEAP_VADDR_END: u32 = LINEAR_HEAP_VADDR + 0x08000000;
const VRAM_VADDR: u32 = 0x1F000000;
const VRAM_VADDR_END: u32 = VRAM_VADDR + VRAM_SIZE;
const NEW_LINEAR_HEAP_VADDR: u32 = 0x30000000;
const NEW_LINEAR_HEAP_VADDR_END: u32 = NEW_LINEAR_HEAP_VADDR + 0x10000000;
const PLUGIN_3GX_FB_VADDR: u32 = 0x06000000;
const PLUGIN_3GX_FB_VADDR_END: u32 = PLUGIN_3GX_FB_VADDR + 0x000A9000;

const MEMORY_FILL_TRIGGER: usize =
    gpu_reg_index!(memory_fill_config) + offset_of!(MemoryFillConfig, control) / size_of::<u32>();
const MEMORY_FILL_TRIGGER_1: usize =
    MEMORY_FILL_TRIGGER + size_of::<MemoryFillConfig>() / size_of::<u32>();
const DISPLAY_TRANSFER_TRIGGER: usize = gpu_reg_index!(display_transfer_config.trigger);
const COMMAND_BUFFER_TRIGGER: usize = gpu_reg_index!(internal.pipeline.command_buffer.trigger);
const COMMAND_BUFFER_TRIGGER_1: usize = COMMAND_BUFFER_TRIGGER + 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlushMode {
    /// Write back modified surfaces to RAM
    Flush,
    /// Remove region from the cache
    Invalidate,
    /// Write back modified surfaces to RAM, and also remove them from the cache
    FlushAndInvalidate,
}

/// The parts of the system that the GPU uses outside the video core.
pub trait GpuHost {
    /// Copies a block in the memory of the current process, as `MemorySystem::CopyBlock` does.
    fn copy_block(&mut self, dest_addr: u32, src_addr: u32, size: u32);

    /// Ends the game frame of the performance statistics.
    fn end_game_frame(&mut self);
}

/**
 * The GPU class is the high level interface to the video_core for core services.
 */
pub struct Gpu {
    right_eye_disabler: RightEyeDisabler,
    pica: Box<PicaCore>,
    renderer: Box<dyn Renderer>,
    sw_blitter: SwBlitter,
    signal_interrupt: Option<InterruptHandler>,
    plugin_fb_addr: Option<u32>,
    pub disable_right_eye_render: bool,
}

impl Gpu {
    pub fn new(
        renderer: Box<dyn Renderer>,
        regs: SharedRegs,
        memory: Rc<dyn PhysicalMemory>,
        use_shader_jit: bool,
    ) -> Self {
        Self {
            right_eye_disabler: RightEyeDisabler::default(),
            pica: Box::new(PicaCore::new(regs, memory.clone(), use_shader_jit)),
            renderer,
            sw_blitter: SwBlitter::new(memory),
            signal_interrupt: None,
            plugin_fb_addr: None,
            disable_right_eye_render: false,
        }
    }

    /// Sets the framebuffer address of the 3GX plugin, which the plugin loader service holds.
    pub fn set_plugin_fb_addr(&mut self, plugin_fb_addr: Option<u32>) {
        self.plugin_fb_addr = plugin_fb_addr;
    }

    fn virtual_to_physical_address(&self, addr: u32) -> u32 {
        if addr == 0 {
            return 0;
        }

        if (VRAM_VADDR..=VRAM_VADDR_END).contains(&addr) {
            return addr - VRAM_VADDR + VRAM_PADDR;
        }
        if (LINEAR_HEAP_VADDR..=LINEAR_HEAP_VADDR_END).contains(&addr) {
            return addr - LINEAR_HEAP_VADDR + FCRAM_PADDR;
        }
        if (NEW_LINEAR_HEAP_VADDR..=NEW_LINEAR_HEAP_VADDR_END).contains(&addr) {
            return addr - NEW_LINEAR_HEAP_VADDR + FCRAM_PADDR;
        }
        if (PLUGIN_3GX_FB_VADDR..=PLUGIN_3GX_FB_VADDR_END).contains(&addr)
            && let Some(plugin_fb_addr) = self.plugin_fb_addr
        {
            return (addr - PLUGIN_3GX_FB_VADDR).wrapping_add(plugin_fb_addr);
        }

        log::error!(target: "HW_Memory", "Unknown virtual address @ 0x{:08X}", addr);
        addr
    }

    /// Sets the function to call for signalling GSP interrupts.
    pub fn set_interrupt_handler(&mut self, handler: InterruptHandler) {
        self.signal_interrupt = Some(handler.clone());
        self.pica.set_interrupt_handler(handler);
    }

    fn signal_interrupt(&self, id: InterruptId) {
        let signal_interrupt = self
            .signal_interrupt
            .as_ref()
            .expect("the interrupt handler is set before the GPU runs");
        signal_interrupt(id);
    }

    /// Notify rasterizer that any caches of the specified region should be flushed to Switch memory
    pub fn flush_region(&mut self, addr: u32, size: u32) {
        self.renderer.rasterizer().flush_region(addr, size);
    }

    /// Notify rasterizer that any caches of the specified region should be invalidated
    pub fn invalidate_region(&mut self, addr: u32, size: u32) {
        self.renderer.rasterizer().invalidate_region(addr, size);
    }

    /// Flushes and invalidates all memory in the rasterizer cache and removes any leftover state.
    pub fn clear_all(&mut self, flush: bool) {
        self.renderer.rasterizer().clear_all(flush);
    }

    /// Flushes or invalidates the rasterizer caches of the virtual region, as
    /// `MemorySystem::RasterizerFlushVirtualRegion` does.
    pub fn rasterizer_flush_virtual_region(&mut self, start: u32, size: u32, mode: FlushMode) {
        let end = start.wrapping_add(size);
        let rasterizer = self.renderer.rasterizer();

        let mut check_region = |region_start: u32, region_end: u32, paddr_region_start: u32| {
            if start >= region_end || end <= region_start {
                // No overlap with region
                return;
            }

            let overlap_start = start.max(region_start);
            let overlap_end = end.min(region_end);
            let physical_start = paddr_region_start.wrapping_add(overlap_start - region_start);
            let overlap_size = overlap_end - overlap_start;

            match mode {
                FlushMode::Flush => rasterizer.flush_region(physical_start, overlap_size),
                FlushMode::Invalidate => rasterizer.invalidate_region(physical_start, overlap_size),
                FlushMode::FlushAndInvalidate => {
                    rasterizer.flush_and_invalidate_region(physical_start, overlap_size)
                }
            }
        };

        check_region(LINEAR_HEAP_VADDR, LINEAR_HEAP_VADDR_END, FCRAM_PADDR);
        check_region(
            NEW_LINEAR_HEAP_VADDR,
            NEW_LINEAR_HEAP_VADDR_END,
            FCRAM_PADDR,
        );
        check_region(VRAM_VADDR, VRAM_VADDR_END, VRAM_PADDR);
        if let Some(plugin_fb_addr) = self.plugin_fb_addr
            && plugin_fb_addr != 0
        {
            check_region(PLUGIN_3GX_FB_VADDR, PLUGIN_3GX_FB_VADDR_END, plugin_fb_addr);
        }
    }

    /// Executes the provided GSP command.
    pub fn execute(&mut self, host: &mut dyn GpuHost, command: &Command) {
        match command.id() {
            CommandId::REQUEST_DMA => {
                let dma_request = command.dma_request();
                self.rasterizer_flush_virtual_region(
                    dma_request.source_address,
                    dma_request.size,
                    FlushMode::Flush,
                );
                self.rasterizer_flush_virtual_region(
                    dma_request.dest_address,
                    dma_request.size,
                    FlushMode::Invalidate,
                );

                // TODO(Subv): These memory accesses should not go through the application's memory mapping.
                // They should go through the GSP module's memory mapping.
                host.copy_block(
                    dma_request.dest_address,
                    dma_request.source_address,
                    dma_request.size,
                );
                self.signal_interrupt(InterruptId::DMA);
            }
            CommandId::SUBMIT_CMD_LIST => {
                let params = command.submit_gpu_cmdlist();
                let address = self.virtual_to_physical_address(params.address);
                {
                    let mut regs = self.pica.regs.borrow_mut();
                    let cmdbuffer = &mut regs.internal.pipeline.command_buffer;

                    // Write to the command buffer GPU registers
                    assign(&mut cmdbuffer.addr_raw[0], 0, 28, address >> 3);
                    assign(&mut cmdbuffer.size_raw[0], 0, 20, params.size >> 3);
                    cmdbuffer.trigger[0] = 1;
                }

                // Trigger processing of the command list
                self.submit_cmd_list(0);
            }
            CommandId::MEMORY_FILL => {
                let params = command.memory_fill();

                // Write to the memory fill GPU registers.
                if params.start1 != 0 {
                    let address_start = self.virtual_to_physical_address(params.start1) >> 3;
                    let address_end = self.virtual_to_physical_address(params.end1) >> 3;
                    {
                        let mut regs = self.pica.regs.borrow_mut();
                        let memfill = &mut regs.memory_fill_config[0];
                        memfill.address_start = address_start;
                        memfill.address_end = address_end;
                        memfill.value_32bit = params.value1;
                        memfill.control = u32::from(params.control1);
                    }
                    self.memory_fill(0);
                }
                if params.start2 != 0 {
                    let address_start = self.virtual_to_physical_address(params.start2) >> 3;
                    let address_end = self.virtual_to_physical_address(params.end2) >> 3;
                    {
                        let mut regs = self.pica.regs.borrow_mut();
                        let memfill = &mut regs.memory_fill_config[1];
                        memfill.address_start = address_start;
                        memfill.address_end = address_end;
                        memfill.value_32bit = params.value2;
                        memfill.control = u32::from(params.control2);
                    }
                    self.memory_fill(1);
                }
            }
            CommandId::DISPLAY_TRANSFER => {
                let params = command.display_transfer();
                let input_address = self.virtual_to_physical_address(params.in_buffer_address) >> 3;
                let output_address =
                    self.virtual_to_physical_address(params.out_buffer_address) >> 3;
                {
                    let mut regs = self.pica.regs.borrow_mut();
                    let display_transfer = &mut regs.display_transfer_config;

                    // Write to the transfer engine GPU registers.
                    display_transfer.input_address = input_address;
                    display_transfer.output_address = output_address;
                    display_transfer.input_size = params.in_buffer_size;
                    display_transfer.output_size = params.out_buffer_size;
                    display_transfer.flags = params.flags;
                    display_transfer.set_triggered(true);
                }

                // Trigger the display transfer.
                self.memory_transfer();
            }
            CommandId::TEXTURE_COPY => {
                let params = command.texture_copy();
                let input_address = self.virtual_to_physical_address(params.in_buffer_address) >> 3;
                let output_address =
                    self.virtual_to_physical_address(params.out_buffer_address) >> 3;
                {
                    let mut regs = self.pica.regs.borrow_mut();
                    let texture_copy = &mut regs.display_transfer_config;

                    // Write to the transfer engine GPU registers.
                    texture_copy.input_address = input_address;
                    texture_copy.output_address = output_address;
                    texture_copy.texture_copy.size = params.size;
                    texture_copy.texture_copy.input_size = params.in_width_gap;
                    texture_copy.texture_copy.output_size = params.out_width_gap;
                    texture_copy.flags = params.flags;
                    texture_copy.set_triggered(true);
                }

                // Trigger the texture copy.
                self.memory_transfer();
            }
            CommandId::CACHE_FLUSH => {
                // Rasterizer flushing handled elsewhere in CPU read/write and other GPU handlers
                // Use command.cache_flush.regions to implement this handler
            }
            id => {
                log::error!(target: "HW_GPU", "Unknown command {:#08X}", id.0);
            }
        }
    }

    /// Updates GPU display framebuffer configuration using the specified parameters.
    pub fn set_buffer_swap(
        &mut self,
        host: &mut dyn GpuHost,
        screen_id: u32,
        info: &FrameBufferInfo,
    ) {
        let phys_address_left = self.virtual_to_physical_address(info.address_left);
        let phys_address_right = self.virtual_to_physical_address(info.address_right);

        // Update framebuffer properties.
        {
            let mut regs = self.pica.regs.borrow_mut();
            let framebuffer = &mut regs.framebuffer_config[screen_id as usize];
            if info.active_fb == 0 {
                framebuffer.address_left1 = phys_address_left;
                framebuffer.address_right1 = phys_address_right;
            } else {
                framebuffer.address_left2 = phys_address_left;
                framebuffer.address_right2 = phys_address_right;
            }

            framebuffer.stride = info.stride;
            framebuffer.format = info.format;
            framebuffer.active_fb = info.shown_fb;
        }

        if screen_id == 0 {
            host.end_game_frame();
            self.right_eye_disabler
                .report_end_frame(self.disable_right_eye_render);
        }
    }

    /// Sets the LCD color fill configuration for the top and bottom screens.
    pub fn set_color_fill(&mut self, fill: &ColorFill) {
        self.pica.regs_lcd.color_fill_top = *fill;
        self.pica.regs_lcd.color_fill_bottom = *fill;
    }

    /// Reads a word from the GPU virtual address.
    pub fn read_reg(&self, addr: u32) -> u32 {
        match addr & 0xFFFFF000 {
            VADDR_LCD => {
                let offset = addr - VADDR_LCD;
                let index = (offset as usize) / size_of::<u32>();
                assert!(
                    (addr as usize).is_multiple_of(size_of::<u32>()),
                    "Assertion Failed!"
                );
                assert!(index < RegsLcd::num_ids(), "Assertion Failed!");
                self.pica.regs_lcd.as_words()[index]
            }
            VADDR_GPU | 0x1EF01000 => {
                let offset = addr - VADDR_GPU;
                let index = (offset as usize) / size_of::<u32>();
                assert!(
                    (addr as usize).is_multiple_of(size_of::<u32>()),
                    "Assertion Failed!"
                );
                assert!(index < Regs::NUM_REGS, "Assertion Failed!");
                self.pica.regs.borrow().reg_array()[index]
            }
            _ => unreachable!("Read from unknown GPU address {:#08X}", addr),
        }
    }

    /// Writes the provided value to the GPU virtual address.
    pub fn write_reg(&mut self, addr: u32, data: u32) {
        match addr & 0xFFFFF000 {
            VADDR_LCD => {
                let offset = addr - VADDR_LCD;
                let index = (offset as usize) / size_of::<u32>();
                assert!(
                    (addr as usize).is_multiple_of(size_of::<u32>()),
                    "Assertion Failed!"
                );
                assert!(index < RegsLcd::num_ids(), "Assertion Failed!");
                self.pica.regs_lcd.as_words_mut()[index] = data;
            }
            VADDR_GPU | 0x1EF01000 => {
                let offset = addr - VADDR_GPU;
                let index = (offset as usize) / size_of::<u32>();

                assert!(
                    (addr as usize).is_multiple_of(size_of::<u32>()),
                    "Assertion Failed!"
                );
                assert!(index < Regs::NUM_REGS, "Assertion Failed!");
                self.pica.regs.borrow_mut().reg_array_mut()[index] = data;

                // Handle registers that trigger GPU actions
                match index {
                    MEMORY_FILL_TRIGGER => self.memory_fill(0),
                    MEMORY_FILL_TRIGGER_1 => self.memory_fill(1),
                    DISPLAY_TRANSFER_TRIGGER => self.memory_transfer(),
                    COMMAND_BUFFER_TRIGGER => self.submit_cmd_list(0),
                    COMMAND_BUFFER_TRIGGER_1 => self.submit_cmd_list(1),
                    _ => {}
                }
            }
            _ => unreachable!("Write to unknown GPU address {:#08X}", addr),
        }
    }

    /// Synchronizes fixed function renderer state with PICA registers.
    pub fn sync(&mut self) {
        self.renderer.sync();
    }

    /// Returns a mutable reference to the renderer.
    pub fn renderer(&mut self) -> &mut dyn Renderer {
        self.renderer.as_mut()
    }

    /// Returns a mutable reference to the PICA GPU.
    pub fn pica_core(&mut self) -> &mut PicaCore {
        &mut self.pica
    }

    pub fn right_eye_disabler(&mut self) -> &mut RightEyeDisabler {
        &mut self.right_eye_disabler
    }

    pub fn report_loading_program_id(&mut self, program_id: u64, shaders_accurate_mul: bool) {
        let hack = HACK_MANAGER.get_hack(HackType::AccurateMultiplication, program_id);
        let mut use_accurate_mul = shaders_accurate_mul;
        if let Some(hack) = hack {
            match hack.mode {
                HackAllowMode::Disallow => use_accurate_mul = false,
                HackAllowMode::Force => use_accurate_mul = true,
                HackAllowMode::Allow => {}
            }
        }
        self.renderer
            .rasterizer()
            .set_accurate_mul(use_accurate_mul);
    }

    fn submit_cmd_list(&mut self, index: u32) {
        // Check if a command list was triggered.
        let (addr, size) = {
            let regs = self.pica.regs.borrow();
            let config = &regs.internal.pipeline.command_buffer;
            if config.trigger[index as usize] == 0 {
                return;
            }

            // Forward command list processing to the PICA core.
            (config.get_physical_address(index), config.get_size(index))
        };
        let allow = self.right_eye_disabler.should_allow_cmd_queue_trigger(
            &mut self.pica,
            self.disable_right_eye_render,
            addr,
            size,
        );
        self.pica
            .process_cmd_list(self.renderer.rasterizer(), addr, size, !allow);
        self.pica
            .regs
            .borrow_mut()
            .internal
            .pipeline
            .command_buffer
            .trigger[index as usize] = 0;
    }

    fn memory_fill(&mut self, index: u32) {
        // Check if a memory fill was triggered.
        let config = self.pica.regs.borrow().memory_fill_config[index as usize];
        if !config.trigger() {
            return;
        }

        // Perform memory fill.
        let rasterizer = self.renderer.rasterizer();
        if !rasterizer.accelerate_fill(&config) {
            self.sw_blitter.memory_fill(rasterizer, &config);
        }

        // It seems that it won't signal interrupt if "address_start" is zero.
        // TODO: hwtest this
        if config.start_address() != 0 {
            if index == 0 {
                self.signal_interrupt(InterruptId::PSC0);
            } else {
                self.signal_interrupt(InterruptId::PSC1);
            }
        }

        // Reset "trigger" flag and set the "finish" flag
        // This was confirmed to happen on hardware even if "address_start" is zero.
        let mut regs = self.pica.regs.borrow_mut();
        let config = &mut regs.memory_fill_config[index as usize];
        config.set_trigger(false);
        config.set_finished(true);
    }

    fn memory_transfer(&mut self) {
        // Check if a transfer was triggered.
        let config = self.pica.regs.borrow().display_transfer_config;
        if !config.triggered() {
            return;
        }

        // Perform memory transfer
        let rasterizer = self.renderer.rasterizer();
        if config.is_texture_copy() {
            if !rasterizer.accelerate_texture_copy(&config) {
                self.sw_blitter.texture_copy(rasterizer, &config);
            }
        } else if self.right_eye_disabler.should_allow_display_transfer(
            config.physical_input_address(),
            config.input_height() as usize,
        ) && !rasterizer.accelerate_display_transfer(&config)
        {
            self.sw_blitter.display_transfer(rasterizer, &config);
        }

        // Complete transfer.
        self.pica
            .regs
            .borrow_mut()
            .display_transfer_config
            .set_triggered(false);
        self.signal_interrupt(InterruptId::PPF);
    }

    /// Presents the frame and signals the interrupts of the VBlank, and returns the ticks until the
    /// next VBlank, which the caller schedules as the recurrent event of the core timing.
    pub fn vblank_callback(&mut self, cycles_late: i64) -> i64 {
        // Present renderered frame.
        self.renderer.swap_buffers();

        // Signal to GSP that GPU interrupt has occurred
        self.signal_interrupt(InterruptId::PDC0);
        self.signal_interrupt(InterruptId::PDC1);

        // Reschedule recurrent event
        FRAME_TICKS as i64 - cycles_late
    }
}
