use emu_kernel::{HleRequestContext, PortInfo};

pub const PORTS: &[PortInfo] = &[
    PortInfo {
        name: "gsp::Gpu",
        max_sessions: 4,
    },
    PortInfo {
        name: "gsp::Lcd",
        max_sessions: 10,
    },
];

#[derive(Default)]
pub struct Gsp {}

impl Gsp {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_sync_request(&mut self, _port: &str, _context: &mut HleRequestContext) {}
}
