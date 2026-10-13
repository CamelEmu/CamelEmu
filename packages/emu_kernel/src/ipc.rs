pub const COMMAND_BUFFER_LENGTH: usize = 0x100 / size_of::<u32>();

pub struct HleRequestContext {
    command_buffer: [u32; COMMAND_BUFFER_LENGTH],
}

impl HleRequestContext {
    pub fn new() -> Self {
        Self {
            command_buffer: [0; COMMAND_BUFFER_LENGTH],
        }
    }

    pub fn command_buffer(&self) -> &[u32; COMMAND_BUFFER_LENGTH] {
        &self.command_buffer
    }

    pub fn command_buffer_mut(&mut self) -> &mut [u32; COMMAND_BUFFER_LENGTH] {
        &mut self.command_buffer
    }

    pub fn command_id(&self) -> u32 {
        self.command_buffer[0] >> 16
    }
}

impl Default for HleRequestContext {
    fn default() -> Self {
        Self::new()
    }
}
