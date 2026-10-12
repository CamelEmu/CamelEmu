use std::sync::Mutex;

#[derive(Clone, Copy)]
struct Reservation {
    address: Option<u32>,
    value: u64,
}

/// The global monitor of the exclusive accesses, shared by the cores, with the behaviour of the
/// one of dynarmic: a store succeeds only if its core still holds the reservation of the address,
/// and the store clears the reservation of every core on the address.
pub struct ExclusiveMonitor {
    reservations: Mutex<Vec<Reservation>>,
}

impl ExclusiveMonitor {
    pub fn new(core_count: usize) -> Self {
        Self {
            reservations: Mutex::new(vec![
                Reservation {
                    address: None,
                    value: 0,
                };
                core_count
            ]),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Reservation>> {
        self.reservations.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Reads the value with `read` and reserves the address for the core.
    pub fn read_and_mark(&self, core: usize, address: u32, read: impl FnOnce() -> u64) -> u64 {
        let mut reservations = self.lock();
        let value = read();
        if let Some(reservation) = reservations.get_mut(core) {
            *reservation = Reservation {
                address: Some(address),
                value,
            };
        }
        value
    }

    /// Calls `store` with the value that the core read if it holds the reservation of the address.
    /// Returns whether the store happened and succeeded.
    pub fn do_exclusive(&self, core: usize, address: u32, store: impl FnOnce(u64) -> bool) -> bool {
        let mut reservations = self.lock();
        let Some(saved) = reservations.get(core).filter(|r| r.address == Some(address)).map(|r| r.value)
        else {
            return false;
        };
        for reservation in reservations.iter_mut() {
            if reservation.address == Some(address) {
                reservation.address = None;
            }
        }
        store(saved)
    }

    /// Reserves the address with a value that the caller already read.
    pub fn mark(&self, core: usize, address: u32, value: u64) {
        self.read_and_mark(core, address, || value);
    }

    /// Releases the reservation of the address for every core if the core holds it, and returns
    /// the value it read.
    pub fn check_and_clear(&self, core: usize, address: u32) -> Option<u64> {
        let mut saved = None;
        self.do_exclusive(core, address, |value| {
            saved = Some(value);
            true
        });
        saved
    }

    pub fn clear_core(&self, core: usize) {
        if let Some(reservation) = self.lock().get_mut(core) {
            reservation.address = None;
        }
    }
}
