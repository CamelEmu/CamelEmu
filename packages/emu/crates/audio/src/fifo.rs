use crate::error::{AudioError, Result};

pub(crate) struct ChannelLimits {
    pub(crate) max_channels: usize,
    pub(crate) default_channels: usize,
}

impl Default for ChannelLimits {
    fn default() -> Self {
        Self {
            max_channels: 32,
            default_channels: 2,
        }
    }
}

impl ChannelLimits {
    pub(crate) fn verify(&self, channels: usize) -> Result<()> {
        if channels > 0 && channels <= self.max_channels {
            Ok(())
        } else {
            Err(AudioError::IllegalChannels { channels })
        }
    }
}

struct FifoSettings {
    initial_capacity: usize,
    growth_bytes: usize,
}

impl Default for FifoSettings {
    fn default() -> Self {
        Self {
            initial_capacity: 32,
            growth_bytes: 4096,
        }
    }
}

pub(crate) struct FifoSampleBuffer {
    settings: FifoSettings,
    limits: ChannelLimits,
    buffer: Vec<i16>,
    size_in_bytes: usize,
    samples_in_buffer: usize,
    channels: usize,
    buffer_pos: usize,
}

impl Default for FifoSampleBuffer {
    fn default() -> Self {
        let limits = ChannelLimits::default();
        let settings = FifoSettings::default();
        let channels = limits.default_channels;
        let initial_capacity = settings.initial_capacity;
        let mut buffer = Self {
            settings,
            limits,
            buffer: Vec::new(),
            size_in_bytes: 0,
            samples_in_buffer: 0,
            channels,
            buffer_pos: 0,
        };
        buffer.ensure_capacity(initial_capacity);
        buffer
    }
}

impl FifoSampleBuffer {
    pub(crate) fn channels(&self) -> usize {
        self.channels
    }

    pub(crate) fn set_channels(&mut self, channels: usize) -> Result<()> {
        self.limits.verify(channels)?;
        let used = self.channels * self.samples_in_buffer;
        self.channels = channels;
        self.samples_in_buffer = used / self.channels;
        Ok(())
    }

    fn rewind(&mut self) {
        if !self.buffer.is_empty() && self.buffer_pos != 0 {
            let start = self.buffer_pos * self.channels;
            let len = self.samples_in_buffer * self.channels;
            self.buffer.copy_within(start..start + len, 0);
            self.buffer_pos = 0;
        }
    }

    pub(crate) fn put_samples(&mut self, samples: &[i16], num_samples: usize) {
        let len = num_samples * self.channels;
        self.ptr_end(num_samples).copy_from_slice(&samples[..len]);
        self.samples_in_buffer += num_samples;
    }

    pub(crate) fn put_samples_count(&mut self, num_samples: usize) {
        let required = self.samples_in_buffer + num_samples;
        self.ensure_capacity(required);
        self.samples_in_buffer += num_samples;
    }

    pub(crate) fn ptr_end(&mut self, slack_capacity: usize) -> &mut [i16] {
        self.ensure_capacity(self.samples_in_buffer + slack_capacity);
        let start = self.samples_in_buffer * self.channels;
        &mut self.buffer[start..start + slack_capacity * self.channels]
    }

    pub(crate) fn ptr_begin(&self) -> &[i16] {
        let start = self.buffer_pos * self.channels;
        &self.buffer[start..start + self.samples_in_buffer * self.channels]
    }

    fn ensure_capacity(&mut self, capacity_requirement: usize) {
        if capacity_requirement > self.capacity() {
            let required_bytes = capacity_requirement * self.channels * size_of::<i16>();
            self.size_in_bytes = required_bytes.div_ceil(self.settings.growth_bytes)
                * self.settings.growth_bytes;
            let mut temp = vec![0i16; self.size_in_bytes / size_of::<i16>()];
            if self.samples_in_buffer != 0 {
                temp[..self.samples_in_buffer * self.channels].copy_from_slice(self.ptr_begin());
            }
            self.buffer = temp;
            self.buffer_pos = 0;
        } else {
            self.rewind();
        }
    }

    fn capacity(&self) -> usize {
        self.size_in_bytes / (self.channels * size_of::<i16>())
    }

    pub(crate) fn num_samples(&self) -> usize {
        self.samples_in_buffer
    }

    pub(crate) fn receive_samples_into(&mut self, output: &mut [i16], max_samples: usize) -> usize {
        let num = max_samples.min(self.samples_in_buffer);
        let len = self.channels * num;
        output[..len].copy_from_slice(&self.ptr_begin()[..len]);
        self.receive_samples(num)
    }

    pub(crate) fn receive_samples(&mut self, max_samples: usize) -> usize {
        if max_samples >= self.samples_in_buffer {
            let temp = self.samples_in_buffer;
            self.samples_in_buffer = 0;
            return temp;
        }
        self.samples_in_buffer -= max_samples;
        self.buffer_pos += max_samples;
        max_samples
    }

    pub(crate) fn clear(&mut self) {
        self.samples_in_buffer = 0;
        self.buffer_pos = 0;
    }

    pub(crate) fn adjust_amount_of_samples(&mut self, num_samples: usize) -> usize {
        if num_samples < self.samples_in_buffer {
            self.samples_in_buffer = num_samples;
        }
        self.samples_in_buffer
    }

    pub(crate) fn add_silent(&mut self, num_samples: usize) {
        self.ptr_end(num_samples).fill(0);
        self.samples_in_buffer += num_samples;
    }

    pub(crate) fn move_samples(&mut self, other: &mut FifoSampleBuffer) {
        let num = other.num_samples();
        self.put_samples(other.ptr_begin(), num);
        other.receive_samples(num);
    }
}
