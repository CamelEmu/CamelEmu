/// GSP command ID
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommandId(pub u32);

impl CommandId {
    pub const REQUEST_DMA: Self = Self(0x00);
    pub const SUBMIT_CMD_LIST: Self = Self(0x01);
    pub const MEMORY_FILL: Self = Self(0x02);
    pub const DISPLAY_TRANSFER: Self = Self(0x03);
    pub const TEXTURE_COPY: Self = Self(0x04);
    pub const CACHE_FLUSH: Self = Self(0x05);
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DmaCommand {
    pub source_address: u32,
    pub dest_address: u32,
    pub size: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SubmitCmdListCommand {
    pub address: u32,
    pub size: u32,
    pub flags: u32,
    pub unused: [u32; 3],
    pub do_flush: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryFillCommand {
    pub start1: u32,
    pub value1: u32,
    pub end1: u32,

    pub start2: u32,
    pub value2: u32,
    pub end2: u32,

    pub control1: u16,
    pub control2: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct DisplayTransferCommand {
    pub in_buffer_address: u32,
    pub out_buffer_address: u32,
    pub in_buffer_size: u32,
    pub out_buffer_size: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct TextureCopyCommand {
    pub in_buffer_address: u32,
    pub out_buffer_address: u32,
    pub size: u32,
    pub in_width_gap: u32,
    pub out_width_gap: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheFlushRegion {
    pub address: u32,
    pub size: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CacheFlushCommand {
    pub regions: [CacheFlushRegion; 3],
}

/// GSP command
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Command {
    pub header: u32,
    pub raw_data: [u8; 0x1C],
}

const _: () = assert!(
    size_of::<Command>() == 0x20,
    "Command struct has incorrect size"
);

impl Command {
    pub fn id(&self) -> CommandId {
        CommandId(self.header & 0xFF)
    }

    pub fn unknown1(&self) -> u32 {
        (self.header >> 8) & 0xFF
    }

    pub fn stop(&self) -> u32 {
        (self.header >> 16) & 0xFF
    }

    pub fn unknown2(&self) -> u32 {
        (self.header >> 24) & 0xFF
    }

    fn read<T: Copy>(&self) -> T {
        const { assert!(size_of::<T>() <= 0x1C) };
        unsafe { self.raw_data.as_ptr().cast::<T>().read_unaligned() }
    }

    pub fn dma_request(&self) -> DmaCommand {
        self.read()
    }

    pub fn submit_gpu_cmdlist(&self) -> SubmitCmdListCommand {
        self.read()
    }

    pub fn memory_fill(&self) -> MemoryFillCommand {
        self.read()
    }

    pub fn display_transfer(&self) -> DisplayTransferCommand {
        self.read()
    }

    pub fn texture_copy(&self) -> TextureCopyCommand {
        self.read()
    }

    pub fn cache_flush(&self) -> CacheFlushCommand {
        self.read()
    }
}

/// GSP shared memory GX command buffer header
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CommandBuffer {
    pub hex: u32,

    pub unk: [u32; 7],

    pub commands: [Command; 0xF],
}

const _: () = assert!(
    size_of::<CommandBuffer>() == 0x200,
    "CommandBuffer struct has incorrect size"
);

impl CommandBuffer {
    pub const STATUS_STOPPED: u32 = 0x1;
    pub const STATUS_CMD_FAILED: u32 = 0x80;

    // Current command index. This index is updated by GSP module after loading the command
    // data, right before the command is processed. When this index is updated by GSP module,
    // the total commands field is decreased by one as well.
    pub fn index(&self) -> u32 {
        self.hex & 0xFF
    }

    // Total commands to process, must not be value 0 when GSP module handles commands. This
    // must be <=15 when writing a command to shared memory. This is incremented by the
    // application when writing a command to shared memory, after increasing this value
    // TriggerCmdReqQueue is only used if this field is value 1.
    pub fn number_commands(&self) -> u32 {
        (self.hex >> 8) & 0xFF
    }

    // When any of the following flags are set to 1, the GSP module stops processing the
    // commands in the command buffer.
    pub fn status(&self) -> u32 {
        (self.hex >> 16) & 0xFF
    }

    pub fn should_stop(&self) -> u32 {
        (self.hex >> 24) & 0xFF
    }
}
