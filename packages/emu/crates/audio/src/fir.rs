use crate::error::{AudioError, Result};

pub(crate) struct FirFilter {
    length: usize,
    result_div_factor: u32,
    filter_coeffs: Vec<i16>,
}

fn saturate(sum: i64) -> i16 {
    sum.clamp(i16::MIN as i64, i16::MAX as i64) as i16
}

impl FirFilter {
    pub(crate) fn new() -> Self {
        Self {
            length: 0,
            result_div_factor: 0,
            filter_coeffs: Vec::new(),
        }
    }

    fn evaluate_filter_stereo(&self, dest: &mut [i16], src: &[i16], num_samples: usize) -> usize {
        let ilength = self.length & !7;
        let end = 2 * (num_samples - ilength);

        let mut j = 0;
        while j < end {
            let ptr = &src[j..];
            let mut suml: i64 = 0;
            let mut sumr: i64 = 0;
            for i in 0..ilength {
                suml += (ptr[2 * i] as i32 * self.filter_coeffs[i] as i32) as i64;
                sumr += (ptr[2 * i + 1] as i32 * self.filter_coeffs[i] as i32) as i64;
            }
            suml >>= self.result_div_factor;
            sumr >>= self.result_div_factor;
            dest[j] = saturate(suml);
            dest[j + 1] = saturate(sumr);
            j += 2;
        }
        num_samples - ilength
    }

    fn evaluate_filter_mono(&self, dest: &mut [i16], src: &[i16], num_samples: usize) -> usize {
        let ilength = self.length & !7;
        let end = num_samples - ilength;

        for j in 0..end {
            let p_src = &src[j..];
            let mut sum: i64 = 0;
            for (sample, coeff) in p_src.iter().zip(&self.filter_coeffs).take(ilength) {
                sum += (*sample as i32 * *coeff as i32) as i64;
            }
            sum >>= self.result_div_factor;
            dest[j] = saturate(sum);
        }
        end
    }

    fn evaluate_filter_multi(
        &self,
        dest: &mut [i16],
        src: &[i16],
        num_samples: usize,
        num_channels: usize,
    ) -> usize {
        let ilength = self.length & !7;
        let end = num_channels * (num_samples - ilength);

        let mut j = 0;
        while j < end {
            let mut sums = vec![0i64; num_channels];
            let mut ptr = j;
            for i in 0..ilength {
                let coef = self.filter_coeffs[i] as i32;
                for sum in sums.iter_mut() {
                    *sum += (src[ptr] as i32 * coef) as i64;
                    ptr += 1;
                }
            }
            for c in 0..num_channels {
                dest[j + c] = (sums[c] >> self.result_div_factor) as i16;
            }
            j += num_channels;
        }
        num_samples - ilength
    }

    pub(crate) fn set_coefficients(
        &mut self,
        coeffs: &[i16],
        new_length: usize,
        result_div_factor: u32,
    ) -> Result<()> {
        if !new_length.is_multiple_of(8) {
            return Err(AudioError::FirLength { length: new_length });
        }
        self.length = new_length;
        self.result_div_factor = result_div_factor;
        self.filter_coeffs = coeffs[..new_length].to_vec();
        Ok(())
    }

    pub(crate) fn length(&self) -> usize {
        self.length
    }

    pub(crate) fn evaluate(
        &self,
        dest: &mut [i16],
        src: &[i16],
        num_samples: usize,
        num_channels: usize,
    ) -> usize {
        if num_samples < self.length {
            return 0;
        }
        match num_channels {
            1 => self.evaluate_filter_mono(dest, src, num_samples),
            2 => self.evaluate_filter_stereo(dest, src, num_samples),
            _ => self.evaluate_filter_multi(dest, src, num_samples, num_channels),
        }
    }
}
