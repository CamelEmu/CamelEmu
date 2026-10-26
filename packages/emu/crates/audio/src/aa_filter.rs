use std::f64::consts::PI;

use crate::error::Result;
use crate::fifo::FifoSampleBuffer;
use crate::fir::FirFilter;

struct AaFilterSettings {
    length: usize,
    cutoff_freq: f64,
    result_div_factor: u32,
}

impl Default for AaFilterSettings {
    fn default() -> Self {
        Self {
            length: 64,
            cutoff_freq: 0.5,
            result_div_factor: 14,
        }
    }
}

pub(crate) struct AaFilter {
    settings: AaFilterSettings,
    fir: FirFilter,
    cutoff_freq: f64,
}

impl AaFilter {
    pub(crate) fn new() -> Result<Self> {
        let settings = AaFilterSettings::default();
        let cutoff_freq = settings.cutoff_freq;
        let mut filter = Self {
            settings,
            fir: FirFilter::new(),
            cutoff_freq,
        };
        filter.calculate_coeffs()?;
        Ok(filter)
    }

    pub(crate) fn set_cutoff_freq(&mut self, new_cutoff_freq: f64) -> Result<()> {
        self.cutoff_freq = new_cutoff_freq;
        self.calculate_coeffs()
    }

    fn calculate_coeffs(&mut self) -> Result<()> {
        let length = self.settings.length;
        let mut work = vec![0.0f64; length];
        let mut coeffs = vec![0i16; length];

        let wc = 2.0 * PI * self.cutoff_freq;
        let temp_coeff = 2.0 * PI / length as f64;

        let mut sum = 0.0;
        for (i, slot) in work.iter_mut().enumerate() {
            let cnt_temp = i as f64 - (length / 2) as f64;

            let mut temp = cnt_temp * wc;
            let h = if temp != 0.0 { temp.sin() / temp } else { 1.0 };
            let w = 0.54 + 0.46 * (temp_coeff * cnt_temp).cos();

            temp = w * h;
            *slot = temp;
            sum += temp;
        }

        let scale_coeff = (1u32 << self.settings.result_div_factor) as f64 / sum;

        for (coeff, value) in coeffs.iter_mut().zip(&work) {
            let mut temp = value * scale_coeff;
            temp += if temp >= 0.0 { 0.5 } else { -0.5 };
            *coeff = temp as i16;
        }

        self.fir
            .set_coefficients(&coeffs, length, self.settings.result_div_factor)
    }

    pub(crate) fn evaluate(&self, dest: &mut FifoSampleBuffer, src: &mut FifoSampleBuffer) -> usize {
        let num_channels = src.channels();
        let num_src_samples = src.num_samples();
        let result = self.fir.evaluate(
            dest.ptr_end(num_src_samples),
            src.ptr_begin(),
            num_src_samples,
            num_channels,
        );
        src.receive_samples(result);
        dest.put_samples_count(result);
        result
    }

    pub(crate) fn length(&self) -> usize {
        self.fir.length()
    }
}
