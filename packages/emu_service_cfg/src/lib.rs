use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "cfg:i",
        max_sessions: 23,
    },
    PortInfo {
        name: "cfg:s",
        max_sessions: 23,
    },
    PortInfo {
        name: "cfg:u",
        max_sessions: 23,
    },
    PortInfo {
        name: "cfg:nor",
        max_sessions: 23,
    },
];

#[derive(Default)]
pub struct Cfg {}

impl Cfg {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
