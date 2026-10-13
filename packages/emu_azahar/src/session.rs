use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use emu_audio::Audio;
use emu_kernel::{Kernel, Timer};
use emu_network::Network;
use emu_pica::Gpu;
use emu_renderer::GraphicsApi;
use emu_service::ServiceManager;

use crate::cpu::{CpuCore, Monitor};
use crate::host::{Host, System};
use crate::memory::Memory;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultStatus {
    Success,
    ErrorNotInitialized,
    ErrorGetLoader,
    ErrorSystemMode,
    ErrorLoader,
    ErrorLoaderErrorEncrypted,
    ErrorLoaderErrorInvalidFormat,
    ErrorLoaderErrorGbaTitle,
    ErrorSystemFiles,
    ErrorSavestate,
    ErrorArticDisconnected,
    ErrorArticCountryMismatch,
    ShutdownRequested,
    ErrorUnknown,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SessionConfig {
    pub graphics_api: u32,
    pub cpu_clock_percentage: u32,
    pub num_cores: u32,
}

struct SessionPtr(NonNull<Session>);

unsafe impl Send for SessionPtr {}

pub struct Session {
    cores: Vec<CpuCore>,
    _monitor: Monitor,
    _hosts: Vec<Host>,
    system: Box<System>,
    services: ServiceManager,
    network: Network,
    audio: Audio,
    powered_on: bool,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Session {
    pub fn new(config: SessionConfig) -> Option<Box<Self>> {
        let gpu = match config.graphics_api {
            1 => emu_renderer::create(GraphicsApi::OpenGl),
            2 => emu_renderer::create(GraphicsApi::Vulkan),
            _ => emu_renderer::create(GraphicsApi::Software),
        }
        .map(Gpu::new);

        let mut system = Box::new(System {
            memory: Memory::new(gpu)?,
            kernel: Kernel::new(),
        });
        let system_ptr = NonNull::from(system.as_mut());

        let monitor = Monitor::new(config.num_cores)?;
        let mut hosts: Vec<Host> = (0..config.num_cores)
            .map(|_| Host::new(system_ptr, Timer::new(config.cpu_clock_percentage)))
            .collect();
        let mut cores = Vec::new();
        for (core_id, host) in (0..config.num_cores).zip(hosts.iter_mut()) {
            let core = CpuCore::new(&monitor, core_id, &host.callbacks())?;
            host.state = core.state();
            core.set_page_table(
                system.memory.page_table_view(),
                system.memory.page_table_key().cast::<c_void>(),
            );
            cores.push(core);
        }
        if let Some(core) = cores.first() {
            let state = core.state();
            if !state.is_null() {
                system
                    .memory
                    .set_pc_source(unsafe { &raw const (*state).regs[15] });
            }
        }

        let mut services = ServiceManager::new();
        services.install_interfaces();

        Some(Box::new(Self {
            cores,
            _monitor: monitor,
            _hosts: hosts,
            system,
            services,
            network: Network::new(),
            audio: Audio::new(),
            powered_on: false,
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
        }))
    }

    pub fn load(&mut self, path: &str) -> ResultStatus {
        log::error!(target: "Core", "Failed to obtain loader for {path}!");
        ResultStatus::ErrorGetLoader
    }

    pub fn start(&mut self) -> ResultStatus {
        if !self.powered_on {
            return ResultStatus::ErrorNotInitialized;
        }
        if self.thread.is_some() {
            return ResultStatus::Success;
        }
        self.running.store(true, Ordering::Release);
        let session = SessionPtr(NonNull::from(&mut *self));
        let running = Arc::clone(&self.running);
        self.thread = Some(std::thread::spawn(move || {
            let session = session;
            while running.load(Ordering::Acquire) {
                for core in unsafe { &session.0.as_ref().cores } {
                    core.run();
                }
            }
        }));
        ResultStatus::Success
    }

    pub fn pause(&mut self) {
        self.running.store(false, Ordering::Release);
        for core in &self.cores {
            core.halt();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    pub fn stop(&mut self) {
        self.pause();
        self.powered_on = false;
    }

    pub fn gpu(&mut self) -> Option<&mut Gpu> {
        self.system.memory.gpu()
    }

    pub fn services(&mut self) -> &mut ServiceManager {
        &mut self.services
    }

    pub fn network(&mut self) -> &mut Network {
        &mut self.network
    }

    pub fn audio(&mut self) -> &mut Audio {
        &mut self.audio
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop();
    }
}
