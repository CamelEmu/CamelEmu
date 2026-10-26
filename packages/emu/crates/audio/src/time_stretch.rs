use crate::error::Result;
use crate::sound_touch::SoundTouch;

pub struct TimeStretchSettings {
    pub native_sample_rate: u32,
    pub channels: usize,
    pub pitch: f64,
    pub tempo: f64,
    pub max_latency: f64,
    pub backlog_overflow: f64,
    pub target_backlog: f64,
    pub tweak_time_scale: f64,
    pub lpf_time_scale: f64,
    pub min_stretch_ratio: f64,
    pub rising_correction_power: f64,
    pub falling_correction_power: f64,
    pub correction_gain: f64,
}

impl Default for TimeStretchSettings {
    fn default() -> Self {
        Self {
            native_sample_rate: 32728,
            channels: 2,
            pitch: 1.0,
            tempo: 1.0,
            max_latency: 0.25,
            backlog_overflow: 4.0,
            target_backlog: 0.5,
            tweak_time_scale: 0.050,
            lpf_time_scale: 0.712,
            min_stretch_ratio: 0.05,
            rising_correction_power: 1.0,
            falling_correction_power: 3.0,
            correction_gain: 2.0,
        }
    }
}

pub struct TimeStretcher {
    settings: TimeStretchSettings,
    sound_touch: SoundTouch,
    stretch_ratio: f64,
}

impl TimeStretcher {
    pub fn new() -> Result<Self> {
        let settings = TimeStretchSettings::default();
        let mut sound_touch = SoundTouch::new()?;
        sound_touch.set_channels(settings.channels)?;
        sound_touch.set_sample_rate(settings.native_sample_rate)?;
        sound_touch.set_pitch(settings.pitch)?;
        sound_touch.set_tempo(settings.tempo)?;
        Ok(Self {
            settings,
            sound_touch,
            stretch_ratio: 1.0,
        })
    }

    pub fn channels(&self) -> usize {
        self.settings.channels
    }

    pub fn set_output_sample_rate(&mut self, sample_rate: u32) -> Result<()> {
        self.sound_touch.set_sample_rate(sample_rate)
    }

    pub fn process(
        &mut self,
        input: &[i16],
        mut num_in: usize,
        out: &mut [i16],
        num_out: usize,
    ) -> Result<usize> {
        let native_sample_rate = self.settings.native_sample_rate as f64;
        let time_delta = num_out as f64 / native_sample_rate;
        let mut current_ratio = num_in as f64 / num_out as f64;

        let max_backlog = native_sample_rate * self.settings.max_latency;
        let backlog_fullness = self.sound_touch.num_samples() as f64 / max_backlog;
        if backlog_fullness > self.settings.backlog_overflow {
            num_in = 0;
        }

        let tweak_correction = (backlog_fullness - self.settings.target_backlog)
            * (time_delta / self.settings.tweak_time_scale);
        let correction_power = if tweak_correction < 0.0 {
            self.settings.falling_correction_power
        } else {
            self.settings.rising_correction_power
        };
        current_ratio *=
            (1.0 + self.settings.correction_gain * tweak_correction).powf(correction_power);

        let lpf_gain = 1.0 - (-time_delta / self.settings.lpf_time_scale).exp();
        self.stretch_ratio += lpf_gain * (current_ratio - self.stretch_ratio);

        self.stretch_ratio = self.stretch_ratio.max(self.settings.min_stretch_ratio);
        self.sound_touch.set_tempo(self.stretch_ratio)?;

        self.sound_touch.put_samples(input, num_in)?;
        Ok(self.sound_touch.receive_samples(out, num_out))
    }

    pub fn clear(&mut self) {
        self.sound_touch.clear();
    }

    pub fn flush(&mut self) -> Result<()> {
        self.sound_touch.flush()
    }
}
