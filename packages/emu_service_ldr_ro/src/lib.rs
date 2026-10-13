use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[PortInfo {
    name: "ldr:ro",
    max_sessions: 2,
}];

#[derive(Default)]
pub struct LdrRo {}

impl LdrRo {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
