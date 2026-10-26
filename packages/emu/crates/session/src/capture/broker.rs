use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Mutex, MutexGuard};

use super::image::ScreenImage;

pub enum CoreCapture {
    Screenshot(Option<ScreenImage>),
    Data(Vec<u8>),
}

#[derive(Default)]
pub struct CaptureBroker {
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Sender<CoreCapture>>>,
}

impl CaptureBroker {
    pub fn register(&self) -> (u64, Receiver<CoreCapture>) {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = channel();
        self.pending().insert(id, sender);
        (id, receiver)
    }

    pub fn cancel(&self, id: u64) {
        self.pending().remove(&id);
    }

    pub fn deliver(&self, id: u64, capture: CoreCapture) {
        if let Some(sender) = self.pending().remove(&id) {
            let _ = sender.send(capture);
        }
    }

    fn pending(&self) -> MutexGuard<'_, HashMap<u64, Sender<CoreCapture>>> {
        self.pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
