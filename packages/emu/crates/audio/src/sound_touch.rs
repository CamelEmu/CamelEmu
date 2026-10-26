use crate::error::{AudioError, Result};
use crate::fifo::{ChannelLimits, FifoSampleBuffer};
use crate::rate_transposer::RateTransposer;
use crate::td_stretch::TdStretch;

struct SoundTouchSettings {
    float_epsilon: f64,
    flush_block_samples: usize,
    flush_max_blocks: usize,
    max_rate_for_tempo_first: f64,
}

impl Default for SoundTouchSettings {
    fn default() -> Self {
        Self {
            float_epsilon: 1e-10,
            flush_block_samples: 128,
            flush_max_blocks: 200,
            max_rate_for_tempo_first: 1.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    TdStretch,
    RateTransposer,
}

pub struct SoundTouch {
    settings: SoundTouchSettings,
    limits: ChannelLimits,
    rate_transposer: RateTransposer,
    td_stretch: TdStretch,
    output: Output,
    virtual_rate: f64,
    virtual_tempo: f64,
    virtual_pitch: f64,
    srate_set: bool,
    samples_expected_out: f64,
    samples_output: i64,
    channels: usize,
    rate: f64,
    tempo: f64,
}

impl SoundTouch {
    pub fn new() -> Result<Self> {
        let mut sound_touch = Self {
            settings: SoundTouchSettings::default(),
            limits: ChannelLimits::default(),
            rate_transposer: RateTransposer::new()?,
            td_stretch: TdStretch::new()?,
            output: Output::TdStretch,
            virtual_rate: 1.0,
            virtual_tempo: 1.0,
            virtual_pitch: 1.0,
            srate_set: false,
            samples_expected_out: 0.0,
            samples_output: 0,
            channels: 0,
            rate: 0.0,
            tempo: 0.0,
        };

        sound_touch.calc_effective_rate_and_tempo()?;
        Ok(sound_touch)
    }

    fn test_float_equal(&self, a: f64, b: f64) -> bool {
        (a - b).abs() < self.settings.float_epsilon
    }

    fn output_buffer(&mut self) -> &mut FifoSampleBuffer {
        match self.output {
            Output::TdStretch => &mut self.td_stretch.output_buffer,
            Output::RateTransposer => &mut self.rate_transposer.output_buffer,
        }
    }

    pub fn set_channels(&mut self, num_channels: usize) -> Result<()> {
        self.limits.verify(num_channels)?;

        self.channels = num_channels;
        self.rate_transposer.set_channels(num_channels)?;
        self.td_stretch.set_channels(num_channels)
    }

    pub fn set_rate(&mut self, new_rate: f64) -> Result<()> {
        self.virtual_rate = new_rate;
        self.calc_effective_rate_and_tempo()
    }

    pub fn set_tempo(&mut self, new_tempo: f64) -> Result<()> {
        self.virtual_tempo = new_tempo;
        self.calc_effective_rate_and_tempo()
    }

    pub fn set_pitch(&mut self, new_pitch: f64) -> Result<()> {
        self.virtual_pitch = new_pitch;
        self.calc_effective_rate_and_tempo()
    }

    fn calc_effective_rate_and_tempo(&mut self) -> Result<()> {
        let old_tempo = self.tempo;
        let old_rate = self.rate;

        self.tempo = self.virtual_tempo / self.virtual_pitch;
        self.rate = self.virtual_pitch * self.virtual_rate;

        if !self.test_float_equal(self.rate, old_rate) {
            self.rate_transposer.set_rate(self.rate)?;
        }
        if !self.test_float_equal(self.tempo, old_tempo) {
            self.td_stretch.set_tempo(self.tempo);
        }

        if self.rate <= self.settings.max_rate_for_tempo_first {
            if self.output != Output::TdStretch {
                self.td_stretch
                    .output_buffer
                    .move_samples(&mut self.rate_transposer.output_buffer);

                self.output = Output::TdStretch;
            }
        } else if self.output != Output::RateTransposer {
            self.rate_transposer
                .output_buffer
                .move_samples(&mut self.td_stretch.output_buffer);
            self.rate_transposer
                .move_samples(&mut self.td_stretch.input_buffer);

            self.output = Output::RateTransposer;
        }
        Ok(())
    }

    pub fn set_sample_rate(&mut self, srate: u32) -> Result<()> {
        self.td_stretch.set_sample_rate(srate as i32)?;
        self.srate_set = true;
        Ok(())
    }

    pub fn put_samples(&mut self, samples: &[i16], num_samples: usize) -> Result<()> {
        if !self.srate_set {
            return Err(AudioError::SampleRateNotDefined);
        } else if self.channels == 0 {
            return Err(AudioError::ChannelsNotDefined);
        }

        self.samples_expected_out += num_samples as f64 / (self.rate * self.tempo);

        if self.rate <= self.settings.max_rate_for_tempo_first {
            self.rate_transposer.put_samples(samples, num_samples);
            self.td_stretch
                .move_samples(&mut self.rate_transposer.output_buffer);
        } else {
            self.td_stretch.put_samples(samples, num_samples);
            self.rate_transposer
                .move_samples(&mut self.td_stretch.output_buffer);
        }
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        let block_samples = self.settings.flush_block_samples;
        let buff = vec![0i16; block_samples * self.channels];

        let mut num_still_expected =
            ((self.samples_expected_out + 0.5) as i64 - self.samples_output) as i32;
        if num_still_expected < 0 {
            num_still_expected = 0;
        }

        let mut i = 0;
        while num_still_expected > self.num_samples() as i32 && i < self.settings.flush_max_blocks {
            self.put_samples(&buff, block_samples)?;
            i += 1;
        }

        self.output_buffer()
            .adjust_amount_of_samples(num_still_expected as usize);

        self.td_stretch.clear_input();
        Ok(())
    }

    pub fn clear(&mut self) {
        self.samples_expected_out = 0.0;
        self.samples_output = 0;
        self.rate_transposer.clear();
        self.td_stretch.clear();
    }

    pub fn num_samples(&self) -> usize {
        match self.output {
            Output::TdStretch => self.td_stretch.output_buffer.num_samples(),
            Output::RateTransposer => self.rate_transposer.output_buffer.num_samples(),
        }
    }

    pub fn receive_samples(&mut self, output: &mut [i16], max_samples: usize) -> usize {
        let ret = self.output_buffer().receive_samples_into(output, max_samples);
        self.samples_output += ret as i64;
        ret
    }
}
