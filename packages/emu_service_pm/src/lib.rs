use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "pm:app",
        max_sessions: 3,
    },
    PortInfo {
        name: "pm:dbg",
        max_sessions: 3,
    },
];

#[derive(Default)]
pub struct Pm {}

impl Pm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
