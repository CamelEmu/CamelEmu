use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "am:app",
        max_sessions: 5,
    },
    PortInfo {
        name: "am:net",
        max_sessions: 5,
    },
    PortInfo {
        name: "am:sys",
        max_sessions: 5,
    },
    PortInfo {
        name: "am:u",
        max_sessions: 5,
    },
];

#[derive(Default)]
pub struct Am {}

impl Am {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
