use std::sync::atomic::AtomicBool;

use emu_pica_types::{
    DiskResourceLoadCallback, DisplayTransferConfig, MemoryFillConfig, OutputVertex,
};

#[derive(Default)]
pub struct RasterizerVulkan {
    accurate_mul: bool,
}

impl RasterizerVulkan {
    pub fn add_triangle(&mut self, _v0: &OutputVertex, _v1: &OutputVertex, _v2: &OutputVertex) {}

    pub fn draw_triangles(&mut self) {}

    pub fn notify_pica_register_changed(&mut self, _id: u32) {}

    pub fn flush_all(&mut self) {}

    pub fn flush_region(&mut self, _addr: u32, _size: u32) {}

    pub fn invalidate_region(&mut self, _addr: u32, _size: u32) {}

    pub fn flush_and_invalidate_region(&mut self, _addr: u32, _size: u32) {}

    pub fn clear_all(&mut self, _flush: bool) {}

    pub fn accelerate_display_transfer(&mut self, _config: &DisplayTransferConfig) -> bool {
        false
    }

    pub fn accelerate_texture_copy(&mut self, _config: &DisplayTransferConfig) -> bool {
        false
    }

    pub fn accelerate_fill(&mut self, _config: &MemoryFillConfig) -> bool {
        false
    }

    pub fn accelerate_draw_batch(&mut self, _is_indexed: bool) -> bool {
        false
    }

    pub fn load_disk_resources(
        &mut self,
        _stop_loading: &AtomicBool,
        _callback: DiskResourceLoadCallback,
    ) {
    }

    pub fn sync_entire_state(&mut self) {}

    pub fn set_accurate_mul(&mut self, accurate_mul: bool) {
        self.accurate_mul = accurate_mul;
    }
}

#[derive(Default)]
pub struct VulkanRenderer {
    rasterizer: RasterizerVulkan,
}

impl VulkanRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn rasterizer(&mut self) -> &mut RasterizerVulkan {
        &mut self.rasterizer
    }

    pub fn swap_buffers(&mut self) {}

    pub fn try_present(&mut self, _timeout_ms: i32, _is_secondary: bool) {}

    pub fn prepare_video_dumping(&mut self) {}

    pub fn cleanup_video_dumping(&mut self) {}

    pub fn sync(&mut self) {}

    pub fn notify_surface_changed(&mut self, _is_secondary: bool) {}
}
