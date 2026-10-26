pub trait Stage: Sync {
    fn order(&self) -> u32;
}

pub trait Notification: Sync {
    fn notify(&self, stage: &dyn Stage, done: u64, total: u64);
}

pub struct Shaders;
pub struct Meshes;
pub struct Merging;
pub struct Atlas;
pub struct Exporting;

pub const STAGE_COUNT: u32 = 5;

impl Stage for Shaders {
    fn order(&self) -> u32 {
        0
    }
}

impl Stage for Meshes {
    fn order(&self) -> u32 {
        1
    }
}

impl Stage for Merging {
    fn order(&self) -> u32 {
        2
    }
}

impl Stage for Atlas {
    fn order(&self) -> u32 {
        3
    }
}

impl Stage for Exporting {
    fn order(&self) -> u32 {
        4
    }
}

struct Silent;

impl Notification for Silent {
    fn notify(&self, _: &dyn Stage, _: u64, _: u64) {}
}

pub struct Progress<'a> {
    notification: &'a dyn Notification,
    stage: &'static dyn Stage,
    done: u64,
    total: u64,
}

impl<'a> Progress<'a> {
    pub fn new(notification: &'a dyn Notification) -> Self {
        Self {
            notification,
            stage: &Shaders,
            done: 0,
            total: 0,
        }
    }

    pub fn silent() -> Progress<'static> {
        Progress::new(&Silent)
    }

    pub fn begin(&mut self, stage: &'static dyn Stage, total: u64) {
        self.stage = stage;
        self.done = 0;
        self.total = total;
        self.send();
    }

    pub fn advance(&mut self, count: u64) {
        self.done = (self.done + count).min(self.total);
        self.send();
    }

    pub fn complete(&mut self) {
        self.done = self.total;
        self.send();
    }

    fn send(&self) {
        self.notification.notify(self.stage, self.done, self.total);
    }
}
