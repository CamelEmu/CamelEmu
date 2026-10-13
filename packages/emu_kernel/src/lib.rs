pub mod ipc;
pub mod timing;

pub use ipc::HleRequestContext;
pub use timing::Timer;

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

    pub fn call_svc(&mut self, _immediate: u32) {}
}
