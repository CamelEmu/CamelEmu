use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "ac:i",
        max_sessions: 10,
    },
    PortInfo {
        name: "ac:u",
        max_sessions: 10,
    },
    PortInfo {
        name: "soc:U",
        max_sessions: 18,
    },
];

#[derive(Default)]
pub struct Network {}

impl Network {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
