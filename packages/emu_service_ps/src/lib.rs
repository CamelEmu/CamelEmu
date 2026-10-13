use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[PortInfo {
    name: "ps:ps",
    max_sessions: 10,
}];

#[derive(Default)]
pub struct Ps {}

impl Ps {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
