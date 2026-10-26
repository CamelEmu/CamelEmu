use crate::api::capture::CaptureTarget;

pub trait Screen: Sync {
    fn is_bottom(&self) -> bool;
    fn owns_draw(&self, screen_width: u32) -> bool;
}

pub struct TopScreen {
    pub widths: &'static [u32],
}

pub struct BottomScreen {
    pub widths: &'static [u32],
}

impl Screen for TopScreen {
    fn is_bottom(&self) -> bool {
        false
    }

    fn owns_draw(&self, screen_width: u32) -> bool {
        self.widths.contains(&screen_width)
    }
}

impl Screen for BottomScreen {
    fn is_bottom(&self) -> bool {
        true
    }

    fn owns_draw(&self, screen_width: u32) -> bool {
        self.widths.contains(&screen_width)
    }
}

static TOP: TopScreen = TopScreen { widths: &[400, 800] };
static BOTTOM: BottomScreen = BottomScreen { widths: &[320] };

impl CaptureTarget {
    pub fn screens(self) -> Vec<&'static dyn Screen> {
        match self {
            CaptureTarget::Both => vec![&TOP, &BOTTOM],
            CaptureTarget::Top => vec![&TOP],
            CaptureTarget::Bottom => vec![&BOTTOM],
        }
    }
}
