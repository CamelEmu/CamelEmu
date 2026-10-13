use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[PortInfo {
    name: "srv:",
    max_sessions: 64,
}];

#[derive(Default)]
pub struct Sm {}

impl Sm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
