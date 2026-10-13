mod ffi;

pub use emu_cpu;
pub use emu_mem;

pub use emu_audio::Audio;
pub use emu_kernel::Kernel;
pub use emu_network::Network;
pub use emu_pica::Gpu;
pub use emu_renderer::{GraphicsApi, Renderer};
pub use emu_service::ServiceManager;

pub const VERSION: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Config {
    pub renderer: bool,
    pub audio: bool,
    pub network: bool,
    pub kernel: bool,
    pub service: bool,
}

pub struct EmuAzahar {
    config: Config,
}

impl EmuAzahar {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn config(&self) -> Config {
        self.config
    }
}
