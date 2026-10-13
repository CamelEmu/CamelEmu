use crate::pica_core::PicaCore;

pub struct RightEyeDisabler {
    enabled: bool,
    enable_for_frame: bool,

    top_screen_drawn: bool,
    top_screen_transfered: bool,
    top_screen_blocked: bool,
    cmd_trigger_blocked: bool,
    top_screen_buf: u32,

    cmd_queue_trigger_happened: bool,
    display_tranfer_happened: bool,
    report_end_frame_pending: bool,
}

impl Default for RightEyeDisabler {
    fn default() -> Self {
        Self {
            enabled: true,
            enable_for_frame: true,
            top_screen_drawn: false,
            top_screen_transfered: false,
            top_screen_blocked: false,
            cmd_trigger_blocked: false,
            top_screen_buf: 0,
            cmd_queue_trigger_happened: false,
            display_tranfer_happened: false,
            report_end_frame_pending: false,
        }
    }
}

impl RightEyeDisabler {
    pub fn should_allow_cmd_queue_trigger(
        &mut self,
        pica: &mut PicaCore,
        disable_right_eye_render: bool,
        addr: u32,
        size: u32,
    ) -> bool {
        if !self.enabled || !self.enable_for_frame {
            return true;
        }

        const TOP_SCREEN_SIZE: u32 = 0x00469000;

        if self.report_end_frame_pending {
            self.report_end_frame(disable_right_eye_render);
            self.report_end_frame_pending = false;
        }
        self.cmd_queue_trigger_happened = true;

        let guess = pica.guess_cmd_render_properties(addr, size);
        if guess.vp_height == TOP_SCREEN_SIZE && !self.top_screen_blocked {
            if self.top_screen_buf == 0 {
                self.top_screen_buf = guess.paddr;
            }
            self.top_screen_drawn = true;
            if self.top_screen_transfered {
                self.cmd_trigger_blocked = true;
                return false;
            }
        }

        self.cmd_trigger_blocked = false;
        true
    }

    pub fn should_allow_display_transfer(&mut self, src_address: u32, size: usize) -> bool {
        if !self.enabled || !self.enable_for_frame {
            return true;
        }

        if size >= 400 && !self.top_screen_blocked {
            if self.top_screen_drawn && src_address == self.top_screen_buf {
                self.top_screen_transfered = true;
            }

            if src_address == self.top_screen_buf && self.cmd_trigger_blocked {
                self.top_screen_blocked = true;
                return false;
            }
        }

        if self.cmd_queue_trigger_happened {
            self.display_tranfer_happened = true;
        }
        true
    }

    pub fn report_end_frame(&mut self, disable_right_eye_render: bool) {
        if !self.enabled {
            return;
        }

        self.enable_for_frame = disable_right_eye_render;

        if self.display_tranfer_happened {
            self.top_screen_drawn = false;
            self.top_screen_transfered = false;
            self.top_screen_blocked = false;
            self.cmd_queue_trigger_happened = false;
            self.cmd_trigger_blocked = false;
            self.display_tranfer_happened = false;
            self.top_screen_buf = 0;
        } else {
            self.report_end_frame_pending = true;
        }
    }

    pub fn set_enabled(&mut self, enable: bool) {
        self.enabled = enable;
    }
}
