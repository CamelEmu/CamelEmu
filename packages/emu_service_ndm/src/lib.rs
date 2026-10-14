use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[PortInfo {
    name: "ndm:u",
    max_sessions: 6,
}];

#[derive(Default)]
pub struct Ndm {}

impl Ndm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
