use crate::api::capture::CaptureTarget;

pub trait Screen: Sync {
    fn is_bottom(&self) -> bool;
    fn owns_draw(&self, screen: u32) -> bool;
}

pub struct TopScreen {
    pub index: u32,
}

pub struct BottomScreen {
    pub index: u32,
}

impl Screen for TopScreen {
    fn is_bottom(&self) -> bool {
        false
    }

    fn owns_draw(&self, screen: u32) -> bool {
        screen == self.index
    }
}

impl Screen for BottomScreen {
    fn is_bottom(&self) -> bool {
        true
    }

    fn owns_draw(&self, screen: u32) -> bool {
        screen == self.index
    }
}

static TOP: TopScreen = TopScreen { index: 0 };
static BOTTOM: BottomScreen = BottomScreen { index: 1 };

impl CaptureTarget {
    pub fn screens(self) -> Vec<&'static dyn Screen> {
        match self {
            CaptureTarget::Both => vec![&TOP, &BOTTOM],
            CaptureTarget::Top => vec![&TOP],
            CaptureTarget::Bottom => vec![&BOTTOM],
        }
    }
}
