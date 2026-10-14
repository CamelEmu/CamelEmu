use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "hid:USER",
        max_sessions: 6,
    },
    PortInfo {
        name: "hid:SPVR",
        max_sessions: 6,
    },
];

#[derive(Default)]
pub struct Hid {}

impl Hid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
