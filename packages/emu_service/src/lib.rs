use emu_kernel::{HleRequestContext, PortInfo};

pub use emu_service_am::Am;
pub use emu_service_apt::Apt;
pub use emu_service_cfg::Cfg;
pub use emu_service_dsp::Dsp;
pub use emu_service_err::ErrF;
pub use emu_service_fs::Fs;
pub use emu_service_gsp::Gsp;
pub use emu_service_hid::Hid;
pub use emu_service_ldr_ro::LdrRo;
pub use emu_service_ndm::Ndm;
pub use emu_service_pm::Pm;
pub use emu_service_ps::Ps;
pub use emu_service_ptm::Ptm;
pub use emu_service_sm::Sm;

pub trait Service {
    fn ports(&self) -> &'static [PortInfo];

    fn handle_sync_request(&mut self, port: &str, context: &mut HleRequestContext);
}

macro_rules! impl_service {
    ($($ty:ty => $ports:path),* $(,)?) => {
        $(
            impl Service for $ty {
                fn ports(&self) -> &'static [PortInfo] {
                    $ports
                }

                fn handle_sync_request(&mut self, port: &str, context: &mut HleRequestContext) {
                    <$ty>::handle_sync_request(self, port, context)
                }
            }
        )*
    };
}

impl_service!(
    Sm => emu_service_sm::PORTS,
    Fs => emu_service_fs::PORTS,
    Pm => emu_service_pm::PORTS,
    LdrRo => emu_service_ldr_ro::PORTS,
    ErrF => emu_service_err::PORTS,
    Am => emu_service_am::PORTS,
    Cfg => emu_service_cfg::PORTS,
    Dsp => emu_service_dsp::PORTS,
    Gsp => emu_service_gsp::PORTS,
    Hid => emu_service_hid::PORTS,
    Ndm => emu_service_ndm::PORTS,
    Apt => emu_service_apt::PORTS,
    Ptm => emu_service_ptm::PORTS,
    Ps => emu_service_ps::PORTS,
);

#[derive(Default)]
pub struct ServiceManager {
    services: Vec<Box<dyn Service>>,
}

impl ServiceManager {
    pub fn new() -> Self {
        Self::default()
    }

    // The order follows Service::Init of the C++ core, so that the ports are registered in the
    // same order.
    pub fn install_interfaces(&mut self) {
        self.services = vec![
            Box::new(Sm::new()),
            Box::new(Fs::new()),
            Box::new(Pm::new()),
            Box::new(LdrRo::new()),
            Box::new(ErrF::new()),
            Box::new(Am::new()),
            Box::new(Cfg::new()),
            Box::new(Dsp::new()),
            Box::new(Gsp::new()),
            Box::new(Hid::new()),
            Box::new(Ndm::new()),
            Box::new(Apt::new()),
            Box::new(Ptm::new()),
            Box::new(Ps::new()),
        ];
    }

    pub fn find(&mut self, port: &str) -> Option<&mut (dyn Service + 'static)> {
        self.services
            .iter_mut()
            .find(|service| service.ports().iter().any(|info| info.name == port))
            .map(|service| service.as_mut())
    }
}
