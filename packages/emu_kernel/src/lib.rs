pub mod ipc;

pub use ipc::HleRequestContext;

pub struct PortInfo {
    pub name: &'static str,
    pub max_sessions: u32,
}

#[derive(Default)]
pub struct Kernel {}

impl Kernel {
    pub fn new() -> Self {
        Self::default()
    }
}
