use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[PortInfo {
    name: "fs:USER",
    max_sessions: 30,
}];

#[derive(Default)]
pub struct Fs {}

impl Fs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
