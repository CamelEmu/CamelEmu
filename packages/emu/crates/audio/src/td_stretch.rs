use crate::error::{AudioError, Result};
use crate::fifo::{ChannelLimits, FifoSampleBuffer};

struct AutoRange {
    tempo_low: f64,
    tempo_top: f64,
    seq_at_min: f64,
    seq_at_max: f64,
    seek_at_min: f64,
    seek_at_max: f64,
}

impl Default for AutoRange {
    fn default() -> Self {
        Self {
            tempo_low: 0.5,
            tempo_top: 2.0,
            seq_at_min: 90.0,
            seq_at_max: 40.0,
            seek_at_min: 20.0,
            seek_at_max: 15.0,
        }
    }
}

impl AutoRange {
    fn seq_k(&self) -> f64 {
        (self.seq_at_max - self.seq_at_min) / (self.tempo_top - self.tempo_low)
    }

    fn seq_c(&self) -> f64 {
        self.seq_at_min - self.seq_k() * self.tempo_low
    }

    fn seek_k(&self) -> f64 {
        (self.seek_at_max - self.seek_at_min) / (self.tempo_top - self.tempo_low)
    }

    fn seek_c(&self) -> f64 {
        self.seek_at_min - self.seek_k() * self.tempo_low
    }

    fn check_limits(x: f64, mi: f64, ma: f64) -> f64 {
        if x < mi {
            mi
        } else if x > ma {
            ma
        } else {
            x
        }
    }

    fn sequence_ms(&self, tempo: f64) -> i32 {
        let seq = Self::check_limits(
            self.seq_c() + self.seq_k() * tempo,
            self.seq_at_max,
            self.seq_at_min,
        );
        (seq + 0.5) as i32
    }

    fn seek_window_ms(&self, tempo: f64) -> i32 {
        let seek = Self::check_limits(
            self.seek_c() + self.seek_k() * tempo,
            self.seek_at_max,
            self.seek_at_min,
        );
        (seek + 0.5) as i32
    }
}

struct NormalizerSettings {
    initial_maxnormf: f32,
    adapt_maxnorm: u64,
    adapt_maxnormf: f32,
    average_keep: f32,
    average_new: f32,
    increase_maxnorm: u64,
    extra_increase_maxnorm: u64,
    max_divider_bits: i32,
    decrease_maxnormf: f32,
}

impl Default for NormalizerSettings {
    fn default() -> Self {
        Self {
            initial_maxnormf: 1e8,
            adapt_maxnorm: 1000,
            adapt_maxnormf: 40000000.0,
            average_keep: 0.9,
            average_new: 0.1,
            increase_maxnorm: 800000000,
            extra_increase_maxnorm: 1600000000,
            max_divider_bits: 16,
            decrease_maxnormf: 1000000.0,
        }
    }
}

struct TdStretchSettings {
    sample_rate: i32,
    sequence_ms: i32,
    seek_window_ms: i32,
    overlap_ms: i32,
    max_sample_rate: i32,
    scan_step: i32,
    scan_wind: i32,
    min_overlap_divider_bits: i32,
    max_overlap_divider_bits: i32,
    norm_floor: f64,
    corr_offset: f64,
    corr_start_weight: f64,
    corr_mid_weight: f64,
    auto_range: AutoRange,
    normalizer: NormalizerSettings,
}

impl Default for TdStretchSettings {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            sequence_ms: 0,
            seek_window_ms: 0,
            overlap_ms: 8,
            max_sample_rate: 192000,
            scan_step: 16,
            scan_wind: 8,
            min_overlap_divider_bits: 3,
            max_overlap_divider_bits: 9,
            norm_floor: 1e-9,
            corr_offset: 0.1,
            corr_start_weight: 0.75,
            corr_mid_weight: 0.25,
            auto_range: AutoRange::default(),
            normalizer: NormalizerSettings::default(),
        }
    }
}

fn get_closest_2_power(value: f64) -> i32 {
    (value.ln() / 2.0f64.ln() + 0.5) as i32
}

struct Stretch {
    settings: TdStretchSettings,
    channels: i32,
    sample_req: i32,
    overlap_length: i32,
    seek_length: i32,
    seek_window_length: i32,
    overlap_divider_bits_norm: i32,
    overlap_divider_bits_pure: i32,
    sloping_divider: i32,
    sample_rate: i32,
    sequence_ms: i32,
    seek_window_ms: i32,
    overlap_ms: i32,
    maxnorm: u64,
    maxnormf: f32,
    tempo: f64,
    nominal_skip: f64,
    skip_fract: f64,
    quick_seek: bool,
    auto_seq_setting: bool,
    auto_seek_setting: bool,
    is_beginning: bool,
    mid_buffer: Vec<i16>,
}

impl Stretch {
    fn new() -> Self {
        Self {
            settings: TdStretchSettings::default(),
            channels: ChannelLimits::default().default_channels as i32,
            sample_req: 0,
            overlap_length: 0,
            seek_length: 0,
            seek_window_length: 0,
            overlap_divider_bits_norm: 0,
            overlap_divider_bits_pure: 0,
            sloping_divider: 0,
            sample_rate: 0,
            sequence_ms: 0,
            seek_window_ms: 0,
            overlap_ms: 0,
            maxnorm: 0,
            maxnormf: 0.0,
            tempo: 1.0,
            nominal_skip: 0.0,
            skip_fract: 0.0,
            quick_seek: false,
            auto_seq_setting: true,
            auto_seek_setting: true,
            is_beginning: false,
            mid_buffer: Vec::new(),
        }
    }

    fn set_parameters(
        &mut self,
        a_sample_rate: i32,
        a_sequence_ms: i32,
        a_seek_window_ms: i32,
        a_overlap_ms: i32,
    ) -> Result<()> {
        if a_sample_rate > 0 {
            if a_sample_rate > self.settings.max_sample_rate {
                return Err(AudioError::ExcessiveSampleRate {
                    sample_rate: a_sample_rate,
                });
            }
            self.sample_rate = a_sample_rate;
        }

        if a_overlap_ms > 0 {
            self.overlap_ms = a_overlap_ms;
        }

        if a_sequence_ms > 0 {
            self.sequence_ms = a_sequence_ms;
            self.auto_seq_setting = false;
        } else if a_sequence_ms == 0 {
            self.auto_seq_setting = true;
        }

        if a_seek_window_ms > 0 {
            self.seek_window_ms = a_seek_window_ms;
            self.auto_seek_setting = false;
        } else if a_seek_window_ms == 0 {
            self.auto_seek_setting = true;
        }

        self.calc_seq_parameters();

        self.calculate_overlap_length(self.overlap_ms);

        self.set_tempo(self.tempo);
        Ok(())
    }

    fn overlap_mono(&self, output: &mut [i16], input: &[i16]) {
        let mut m1: i16 = 0;
        let mut m2: i16 = self.overlap_length as i16;

        for i in 0..self.overlap_length as usize {
            output[i] = ((input[i] as i32 * m1 as i32 + self.mid_buffer[i] as i32 * m2 as i32)
                / self.overlap_length) as i16;
            m1 = m1.wrapping_add(1);
            m2 = m2.wrapping_sub(1);
        }
    }

    fn overlap_stereo(&self, output: &mut [i16], input: &[i16]) {
        for i in 0..self.overlap_length {
            let temp = (self.overlap_length - i) as i16 as i32;
            let cnt2 = 2 * i as usize;
            output[cnt2] = ((input[cnt2] as i32 * i + self.mid_buffer[cnt2] as i32 * temp)
                / self.overlap_length) as i16;
            output[cnt2 + 1] = ((input[cnt2 + 1] as i32 * i
                + self.mid_buffer[cnt2 + 1] as i32 * temp)
                / self.overlap_length) as i16;
        }
    }

    fn overlap_multi(&self, output: &mut [i16], input: &[i16]) {
        let mut i = 0;
        for m1 in 0..self.overlap_length as i16 {
            let m2 = (self.overlap_length as i16).wrapping_sub(m1);
            for _ in 0..self.channels {
                output[i] = ((input[i] as i32 * m1 as i32 + self.mid_buffer[i] as i32 * m2 as i32)
                    / self.overlap_length) as i16;
                i += 1;
            }
        }
    }

    fn overlap(&self, output: &mut [i16], input: &[i16], ovl_pos: usize) {
        match self.channels {
            1 => self.overlap_mono(output, &input[ovl_pos..]),
            2 => self.overlap_stereo(output, &input[2 * ovl_pos..]),
            channels => self.overlap_multi(output, &input[channels as usize * ovl_pos..]),
        }
    }

    fn clear_mid_buffer(&mut self) {
        let len = (self.channels * self.overlap_length) as usize;
        self.mid_buffer[..len].fill(0);
    }

    fn seek_best_overlap_position(&mut self, ref_pos: &[i16]) -> i32 {
        let mid_buffer = std::mem::take(&mut self.mid_buffer);
        let best_offs = if self.quick_seek {
            self.seek_best_overlap_position_quick(ref_pos, &mid_buffer)
        } else {
            self.seek_best_overlap_position_full(ref_pos, &mid_buffer)
        };
        self.mid_buffer = mid_buffer;
        best_offs
    }

    fn seek_best_overlap_position_full(&mut self, ref_pos: &[i16], mid_buffer: &[i16]) -> i32 {
        let offset = self.settings.corr_offset;
        let start_weight = self.settings.corr_start_weight;
        let mid_weight = self.settings.corr_mid_weight;
        let mut best_offs = 0;
        let mut norm = 0.0;

        let mut best_corr = self.calc_cross_corr(ref_pos, 0, mid_buffer, &mut norm);
        best_corr = (best_corr + offset) * start_weight;

        for i in 1..self.seek_length {
            let mut corr = self.calc_cross_corr_accumulate(
                ref_pos,
                (self.channels * i) as usize,
                mid_buffer,
                &mut norm,
            );
            let tmp = (2 * i - self.seek_length) as f64 / self.seek_length as f64;
            corr = (corr + offset) * (1.0 - mid_weight * tmp * tmp);

            if corr > best_corr {
                best_corr = corr;
                best_offs = i;
            }
        }

        self.adapt_normalizer();

        best_offs
    }

    fn quick_corr(&mut self, ref_pos: &[i16], mid_buffer: &[i16], i: i32, norm: &mut f64) -> f32 {
        let offset = self.settings.corr_offset as f32;
        let mid_weight = self.settings.corr_mid_weight as f32;
        let corr = self.calc_cross_corr(ref_pos, (self.channels * i) as usize, mid_buffer, norm)
            as f32;
        let tmp = (2 * i - self.seek_length - 1) as f32 / self.seek_length as f32;
        (corr + offset) * (1.0 - mid_weight * tmp * tmp)
    }

    fn seek_best_overlap_position_quick(&mut self, ref_pos: &[i16], mid_buffer: &[i16]) -> i32 {
        let scan_step = self.settings.scan_step;
        let scan_wind = self.settings.scan_wind;
        let mut best_corr = -f32::MAX;
        let mut best_corr2 = -f32::MAX;
        let mut best_offs = scan_wind;
        let mut best_offs2 = scan_wind;
        let mut norm = 0.0;

        let mut i = scan_step;
        while i < self.seek_length - scan_wind - 1 {
            let corr = self.quick_corr(ref_pos, mid_buffer, i, &mut norm);

            if corr > best_corr {
                best_corr2 = best_corr;
                best_offs2 = best_offs;
                best_corr = corr;
                best_offs = i;
            } else if corr > best_corr2 {
                best_corr2 = corr;
                best_offs2 = i;
            }
            i += scan_step;
        }

        let end = (best_offs + scan_wind + 1).min(self.seek_length);
        for i in (best_offs - scan_wind)..end {
            if i == best_offs {
                continue;
            }

            let corr = self.quick_corr(ref_pos, mid_buffer, i, &mut norm);

            if corr > best_corr {
                best_corr = corr;
                best_offs = i;
            }
        }

        let end = (best_offs2 + scan_wind + 1).min(self.seek_length);
        for i in (best_offs2 - scan_wind)..end {
            if i == best_offs2 {
                continue;
            }

            let corr = self.quick_corr(ref_pos, mid_buffer, i, &mut norm);

            if corr > best_corr {
                best_corr = corr;
                best_offs = i;
            }
        }

        self.adapt_normalizer();

        best_offs
    }

    fn adapt_normalizer(&mut self) {
        let normalizer = &self.settings.normalizer;
        if self.maxnorm > normalizer.adapt_maxnorm || self.maxnormf > normalizer.adapt_maxnormf {
            self.maxnormf =
                normalizer.average_keep * self.maxnormf + normalizer.average_new * self.maxnorm as f32;

            if self.maxnorm > normalizer.increase_maxnorm
                && self.overlap_divider_bits_norm < normalizer.max_divider_bits
            {
                self.overlap_divider_bits_norm += 1;
                if self.maxnorm > normalizer.extra_increase_maxnorm {
                    self.overlap_divider_bits_norm += 1;
                }
            } else if self.maxnormf < normalizer.decrease_maxnormf
                && self.overlap_divider_bits_norm > 0
            {
                self.overlap_divider_bits_norm -= 1;
            }
        }

        self.maxnorm = 0;
    }

    fn calc_seq_parameters(&mut self) {
        if self.auto_seq_setting {
            self.sequence_ms = self.settings.auto_range.sequence_ms(self.tempo);
        }

        if self.auto_seek_setting {
            self.seek_window_ms = self.settings.auto_range.seek_window_ms(self.tempo);
        }

        self.seek_window_length = (self.sample_rate * self.sequence_ms) / 1000;
        if self.seek_window_length < 2 * self.overlap_length {
            self.seek_window_length = 2 * self.overlap_length;
        }
        self.seek_length = (self.sample_rate * self.seek_window_ms) / 1000;
    }

    fn set_tempo(&mut self, new_tempo: f64) {
        self.tempo = new_tempo;

        self.calc_seq_parameters();

        self.nominal_skip = self.tempo * (self.seek_window_length - self.overlap_length) as f64;
        let intskip = (self.nominal_skip + 0.5) as i32;

        self.sample_req =
            (intskip + self.overlap_length).max(self.seek_window_length) + self.seek_length;
    }

    fn accept_new_overlap_length(&mut self, new_overlap_length: i32) {
        let prev_ovl = self.overlap_length;
        self.overlap_length = new_overlap_length;

        if self.overlap_length > prev_ovl {
            self.mid_buffer = vec![0; (self.overlap_length * self.channels) as usize];
            self.clear_mid_buffer();
        }
    }

    fn calculate_overlap_length(&mut self, aoverlap_ms: i32) {
        self.overlap_divider_bits_pure =
            get_closest_2_power((self.sample_rate * aoverlap_ms) as f64 / 1000.0) - 1;
        if self.overlap_divider_bits_pure > self.settings.max_overlap_divider_bits {
            self.overlap_divider_bits_pure = self.settings.max_overlap_divider_bits;
        }
        if self.overlap_divider_bits_pure < self.settings.min_overlap_divider_bits {
            self.overlap_divider_bits_pure = self.settings.min_overlap_divider_bits;
        }
        let new_ovl = 2.0f64.powf((self.overlap_divider_bits_pure + 1) as f64) as i32;

        self.accept_new_overlap_length(new_ovl);

        self.overlap_divider_bits_norm = self.overlap_divider_bits_pure;

        self.sloping_divider = (new_ovl * new_ovl - 1) / 3;
    }

    fn normalizer(&self, norm: f64) -> f64 {
        if norm < self.settings.norm_floor {
            1.0
        } else {
            norm
        }
    }

    fn calc_cross_corr(
        &mut self,
        mixing: &[i16],
        pos: usize,
        compare: &[i16],
        norm: &mut f64,
    ) -> f64 {
        let ilength = ((self.channels * self.overlap_length) & !7) as usize;
        let bits = self.overlap_divider_bits_norm;

        let mut corr: i64 = 0;
        let mut lnorm: u64 = 0;
        let mut i = 0;
        while i < ilength {
            let a = mixing[pos + i] as i32;
            let b = mixing[pos + i + 1] as i32;
            let c = compare[i] as i32;
            let d = compare[i + 1] as i32;
            corr = corr
                .wrapping_add((a.wrapping_mul(c).wrapping_add(b.wrapping_mul(d)) >> bits) as i64);
            lnorm = lnorm.wrapping_add(
                (a.wrapping_mul(a).wrapping_add(b.wrapping_mul(b)) >> bits) as i64 as u64,
            );
            i += 2;
        }

        if lnorm > self.maxnorm {
            self.maxnorm = lnorm;
        }
        *norm = lnorm as f64;
        corr as f64 / self.normalizer(*norm).sqrt()
    }

    fn calc_cross_corr_accumulate(
        &mut self,
        mixing: &[i16],
        pos: usize,
        compare: &[i16],
        norm: &mut f64,
    ) -> f64 {
        let ilength = ((self.channels * self.overlap_length) & !7) as usize;
        let bits = self.overlap_divider_bits_norm;

        let mut lnorm: i64 = 0;
        for i in 1..=self.channels as usize {
            let m = mixing[pos - i] as i32;
            lnorm = lnorm.wrapping_sub(((m * m) >> bits) as i64);
        }

        let mut corr: i64 = 0;
        let mut i = 0;
        while i < ilength {
            let a = mixing[pos + i] as i32;
            let b = mixing[pos + i + 1] as i32;
            let c = compare[i] as i32;
            let d = compare[i + 1] as i32;
            corr = corr
                .wrapping_add((a.wrapping_mul(c).wrapping_add(b.wrapping_mul(d)) >> bits) as i64);
            i += 2;
        }

        for _ in 0..self.channels {
            i -= 1;
            let m = mixing[pos + i] as i32;
            lnorm = lnorm.wrapping_add(((m * m) >> bits) as i64);
        }

        *norm += lnorm as f64;
        if *norm > self.maxnorm as f64 {
            self.maxnorm = *norm as u64;
        }

        corr as f64 / self.normalizer(*norm).sqrt()
    }
}

pub(crate) struct TdStretch {
    limits: ChannelLimits,
    state: Stretch,
    pub(crate) input_buffer: FifoSampleBuffer,
    pub(crate) output_buffer: FifoSampleBuffer,
}

impl TdStretch {
    pub(crate) fn new() -> Result<Self> {
        let mut td_stretch = Self {
            limits: ChannelLimits::default(),
            state: Stretch::new(),
            input_buffer: FifoSampleBuffer::default(),
            output_buffer: FifoSampleBuffer::default(),
        };
        let settings = &td_stretch.state.settings;
        let (sample_rate, sequence_ms, seek_window_ms, overlap_ms) = (
            settings.sample_rate,
            settings.sequence_ms,
            settings.seek_window_ms,
            settings.overlap_ms,
        );
        td_stretch
            .state
            .set_parameters(sample_rate, sequence_ms, seek_window_ms, overlap_ms)?;
        td_stretch.state.set_tempo(1.0);

        td_stretch.clear();
        Ok(td_stretch)
    }

    pub(crate) fn set_sample_rate(&mut self, sample_rate: i32) -> Result<()> {
        self.state.set_parameters(sample_rate, -1, -1, -1)
    }

    pub(crate) fn clear_input(&mut self) {
        self.input_buffer.clear();
        self.state.clear_mid_buffer();
        self.state.is_beginning = true;
        self.state.maxnorm = 0;
        self.state.maxnormf = self.state.settings.normalizer.initial_maxnormf;
        self.state.skip_fract = 0.0;
    }

    pub(crate) fn clear(&mut self) {
        self.output_buffer.clear();
        self.clear_input();
    }

    pub(crate) fn set_tempo(&mut self, new_tempo: f64) {
        self.state.set_tempo(new_tempo);
    }

    pub(crate) fn set_channels(&mut self, num_channels: usize) -> Result<()> {
        self.limits.verify(num_channels)?;
        if self.state.channels as usize == num_channels {
            return Ok(());
        }

        self.state.channels = num_channels as i32;
        self.input_buffer.set_channels(num_channels)?;
        self.output_buffer.set_channels(num_channels)?;

        self.state.overlap_length = 0;
        self.state.set_parameters(self.state.sample_rate, -1, -1, -1)
    }

    fn process_samples(&mut self) {
        let state = &mut self.state;
        let channels = state.channels as usize;
        let mut offset: i32 = 0;

        while self.input_buffer.num_samples() as i32 >= state.sample_req {
            if !state.is_beginning {
                offset = state.seek_best_overlap_position(self.input_buffer.ptr_begin());

                let overlap_length = state.overlap_length as usize;
                state.overlap(
                    self.output_buffer.ptr_end(overlap_length),
                    self.input_buffer.ptr_begin(),
                    offset as usize,
                );
                self.output_buffer.put_samples_count(overlap_length);
                offset += state.overlap_length;
            } else {
                state.is_beginning = false;
                let skip = (state.tempo * state.overlap_length as f64
                    + 0.5 * state.seek_length as f64
                    + 0.5) as i32;
                state.skip_fract -= skip as f64;
                if state.skip_fract <= -state.nominal_skip {
                    state.skip_fract = -state.nominal_skip;
                }
            }

            if (self.input_buffer.num_samples() as i32)
                < offset + state.seek_window_length - state.overlap_length
            {
                continue;
            }

            let temp = state.seek_window_length - 2 * state.overlap_length;
            self.output_buffer.put_samples(
                &self.input_buffer.ptr_begin()[channels * offset as usize..],
                temp as usize,
            );

            let mid_len = channels * state.overlap_length as usize;
            let mid_start = channels * (offset + temp) as usize;
            state.mid_buffer[..mid_len]
                .copy_from_slice(&self.input_buffer.ptr_begin()[mid_start..mid_start + mid_len]);

            state.skip_fract += state.nominal_skip;
            let ovl_skip = state.skip_fract as i32;
            state.skip_fract -= ovl_skip as f64;
            self.input_buffer.receive_samples(ovl_skip as usize);
        }
    }

    pub(crate) fn put_samples(&mut self, samples: &[i16], num_samples: usize) {
        self.input_buffer.put_samples(samples, num_samples);
        self.process_samples();
    }

    pub(crate) fn move_samples(&mut self, other: &mut FifoSampleBuffer) {
        let num = other.num_samples();
        self.put_samples(other.ptr_begin(), num);
        other.receive_samples(num);
    }
}
