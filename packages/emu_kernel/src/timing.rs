pub const BASE_CLOCK_RATE_ARM11: u64 = 268_111_856;
pub const MAX_SLICE_LENGTH: i64 = (BASE_CLOCK_RATE_ARM11 / 234) as i64;

pub struct Timer {
    downcount: i64,
    cpu_clock_scale: f64,
}

impl Timer {
    pub fn new(cpu_clock_percentage: u32) -> Self {
        Self {
            downcount: MAX_SLICE_LENGTH,
            cpu_clock_scale: 100.0 / f64::from(cpu_clock_percentage),
        }
    }

    pub fn add_ticks(&mut self, ticks: u64) {
        self.downcount = self
            .downcount
            .wrapping_sub((ticks as f64 * self.cpu_clock_scale) as u64 as i64);
    }

    pub fn downcount(&self) -> i64 {
        self.downcount
    }
}
