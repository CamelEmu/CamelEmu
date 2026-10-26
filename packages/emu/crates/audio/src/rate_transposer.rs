use crate::aa_filter::AaFilter;
use crate::error::Result;
use crate::fifo::{ChannelLimits, FifoSampleBuffer};

struct RateTransposerSettings {
    scale: i32,
    use_aa_filter: bool,
    nyquist_freq: f64,
}

impl Default for RateTransposerSettings {
    fn default() -> Self {
        Self {
            scale: 65536,
            use_aa_filter: true,
            nyquist_freq: 0.5,
        }
    }
}

struct InterpolateLinearInteger {
    scale: i32,
    num_channels: usize,
    rate: f64,
    i_fract: i32,
    i_rate: i32,
}

impl InterpolateLinearInteger {
    fn new(scale: i32) -> Self {
        let mut transposer = Self {
            scale,
            num_channels: 0,
            rate: 1.0,
            i_fract: 0,
            i_rate: 0,
        };
        transposer.reset_registers();
        transposer.set_rate(1.0);
        transposer
    }

    fn reset_registers(&mut self) {
        self.i_fract = 0;
    }

    fn set_channels(&mut self, channels: usize) {
        self.num_channels = channels;
        self.reset_registers();
    }

    fn set_rate(&mut self, new_rate: f64) {
        self.i_rate = (new_rate * self.scale as f64 + 0.5) as i32;
        self.rate = new_rate;
    }

    fn latency(&self) -> usize {
        0
    }

    fn transpose(&mut self, dest: &mut FifoSampleBuffer, src: &mut FifoSampleBuffer) -> usize {
        let mut num_src_samples = src.num_samples() as i32;
        let size_demand = (num_src_samples as f64 / self.rate) as usize + 8;
        let num_output = {
            let psrc = src.ptr_begin();
            let pdest = dest.ptr_end(size_demand);
            match self.num_channels {
                1 => self.transpose_mono(pdest, psrc, &mut num_src_samples),
                2 => self.transpose_stereo(pdest, psrc, &mut num_src_samples),
                _ => self.transpose_multi(pdest, psrc, &mut num_src_samples),
            }
        };
        dest.put_samples_count(num_output);
        src.receive_samples(num_src_samples as usize);
        num_output
    }

    fn transpose_mono(&mut self, dest: &mut [i16], src: &[i16], src_samples: &mut i32) -> usize {
        let scale = self.scale as i64;
        let src_sample_end = *src_samples - 1;
        let mut src_count = 0;
        let mut s = 0;

        let mut i = 0;
        while src_count < src_sample_end {
            let temp = (self.scale - self.i_fract) as i64 * src[s] as i64
                + self.i_fract as i64 * src[s + 1] as i64;
            dest[i] = (temp / scale) as i16;
            i += 1;

            self.i_fract += self.i_rate;

            let i_whole = self.i_fract / self.scale;
            self.i_fract -= i_whole * self.scale;
            src_count += i_whole;
            s += i_whole as usize;
        }
        *src_samples = src_count;

        i
    }

    fn transpose_stereo(&mut self, dest: &mut [i16], src: &[i16], src_samples: &mut i32) -> usize {
        let scale = self.scale as i64;
        let src_sample_end = *src_samples - 1;
        let mut src_count = 0;
        let mut s = 0;

        let mut i = 0;
        while src_count < src_sample_end {
            let temp0 = (self.scale - self.i_fract) as i64 * src[s] as i64
                + self.i_fract as i64 * src[s + 2] as i64;
            let temp1 = (self.scale - self.i_fract) as i64 * src[s + 1] as i64
                + self.i_fract as i64 * src[s + 3] as i64;
            dest[2 * i] = (temp0 / scale) as i16;
            dest[2 * i + 1] = (temp1 / scale) as i16;
            i += 1;

            self.i_fract += self.i_rate;

            let i_whole = self.i_fract / self.scale;
            self.i_fract -= i_whole * self.scale;
            src_count += i_whole;
            s += 2 * i_whole as usize;
        }
        *src_samples = src_count;

        i
    }

    fn transpose_multi(&mut self, dest: &mut [i16], src: &[i16], src_samples: &mut i32) -> usize {
        let scale = self.scale as i64;
        let src_sample_end = *src_samples - 1;
        let mut src_count = 0;
        let mut s = 0;
        let mut d = 0;

        let mut i = 0;
        while src_count < src_sample_end {
            let vol1 = (self.scale - self.i_fract) as i64;
            for c in 0..self.num_channels {
                let temp = vol1 * src[s + c] as i64
                    + self.i_fract as i64 * src[s + c + self.num_channels] as i64;
                dest[d] = (temp / scale) as i16;
                d += 1;
            }
            i += 1;

            self.i_fract += self.i_rate;

            let i_whole = self.i_fract / self.scale;
            self.i_fract -= i_whole * self.scale;
            src_count += i_whole;
            s += i_whole as usize * self.num_channels;
        }
        *src_samples = src_count;

        i
    }
}

pub(crate) struct RateTransposer {
    settings: RateTransposerSettings,
    limits: ChannelLimits,
    aa_filter: AaFilter,
    transposer: InterpolateLinearInteger,
    input_buffer: FifoSampleBuffer,
    mid_buffer: FifoSampleBuffer,
    pub(crate) output_buffer: FifoSampleBuffer,
    use_aa_filter: bool,
}

impl RateTransposer {
    pub(crate) fn new() -> Result<Self> {
        let settings = RateTransposerSettings::default();
        let transposer = InterpolateLinearInteger::new(settings.scale);
        let use_aa_filter = settings.use_aa_filter;
        let mut rate_transposer = Self {
            settings,
            limits: ChannelLimits::default(),
            aa_filter: AaFilter::new()?,
            transposer,
            input_buffer: FifoSampleBuffer::default(),
            mid_buffer: FifoSampleBuffer::default(),
            output_buffer: FifoSampleBuffer::default(),
            use_aa_filter,
        };
        rate_transposer.clear();
        Ok(rate_transposer)
    }

    pub(crate) fn set_rate(&mut self, new_rate: f64) -> Result<()> {
        self.transposer.set_rate(new_rate);

        let cutoff = if new_rate > 1.0 {
            self.settings.nyquist_freq / new_rate
        } else {
            self.settings.nyquist_freq * new_rate
        };
        self.aa_filter.set_cutoff_freq(cutoff)
    }

    pub(crate) fn put_samples(&mut self, samples: &[i16], num_samples: usize) {
        self.process_samples(samples, num_samples);
    }

    pub(crate) fn move_samples(&mut self, other: &mut FifoSampleBuffer) {
        let num = other.num_samples();
        self.put_samples(other.ptr_begin(), num);
        other.receive_samples(num);
    }

    fn process_samples(&mut self, src: &[i16], num_samples: usize) {
        if num_samples == 0 {
            return;
        }

        self.input_buffer.put_samples(src, num_samples);

        if !self.use_aa_filter {
            self.transposer
                .transpose(&mut self.output_buffer, &mut self.input_buffer);
            return;
        }

        if self.transposer.rate < 1.0 {
            self.transposer
                .transpose(&mut self.mid_buffer, &mut self.input_buffer);
            self.aa_filter
                .evaluate(&mut self.output_buffer, &mut self.mid_buffer);
        } else {
            self.aa_filter
                .evaluate(&mut self.mid_buffer, &mut self.input_buffer);
            self.transposer
                .transpose(&mut self.output_buffer, &mut self.mid_buffer);
        }
    }

    pub(crate) fn set_channels(&mut self, channels: usize) -> Result<()> {
        self.limits.verify(channels)?;
        if self.transposer.num_channels == channels {
            return Ok(());
        }

        self.transposer.set_channels(channels);
        self.input_buffer.set_channels(channels)?;
        self.mid_buffer.set_channels(channels)?;
        self.output_buffer.set_channels(channels)
    }

    pub(crate) fn clear(&mut self) {
        self.output_buffer.clear();
        self.mid_buffer.clear();
        self.input_buffer.clear();
        self.transposer.reset_registers();

        let prefill = self.latency();
        self.input_buffer.add_silent(prefill);
    }

    fn latency(&self) -> usize {
        self.transposer.latency()
            + if self.use_aa_filter {
                self.aa_filter.length() / 2
            } else {
                0
            }
    }
}
