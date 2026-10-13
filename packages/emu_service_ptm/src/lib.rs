use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "ptm:gets",
        max_sessions: 26,
    },
    PortInfo {
        name: "ptm:play",
        max_sessions: 26,
    },
    PortInfo {
        name: "ptm:sets",
        max_sessions: 1,
    },
    PortInfo {
        name: "ptm:s",
        max_sessions: 26,
    },
    PortInfo {
        name: "ptm:sysm",
        max_sessions: 26,
    },
    PortInfo {
        name: "ptm:u",
        max_sessions: 26,
    },
];

#[derive(Default)]
pub struct Ptm {}

impl Ptm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
