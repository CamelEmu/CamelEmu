use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "APT:U",
        max_sessions: 2,
    },
    PortInfo {
        name: "APT:S",
        max_sessions: 2,
    },
    PortInfo {
        name: "APT:A",
        max_sessions: 2,
    },
    PortInfo {
        name: "ns:s",
        max_sessions: 3,
    },
    PortInfo {
        name: "ns:c",
        max_sessions: 1,
    },
];

#[derive(Default)]
pub struct Apt {}

impl Apt {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
