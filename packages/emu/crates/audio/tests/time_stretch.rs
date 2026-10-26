use emu_audio::{TimeStretchSettings, TimeStretcher};
use sha2::{Digest, Sha256};

struct XorShift(u32);

impl XorShift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn stretched_digest(amplitude: f64) -> String {
    let native_sample_rate = TimeStretchSettings::default().native_sample_rate as f64;
    let mut stretcher = TimeStretcher::new().unwrap();
    let mut rng = XorShift(12345);
    let mut hasher = Sha256::new();
    let mut frame: u64 = 0;

    for k in 0..600usize {
        let num_out = 900 + (k % 7) * 37;
        let num_in = 400 + ((k * 131) % 1300);
        let mut input = vec![0i16; num_in * 2];
        for i in 0..num_in {
            let t = frame as f64 / native_sample_rate;
            let v = amplitude * (2.0 * std::f64::consts::PI * 440.0 * t).sin()
                + ((rng.next() % 2001) as f64 - 1000.0) * amplitude / 8000.0;
            input[2 * i] = v as i32 as i16;
            input[2 * i + 1] = (-v / 2.0) as i32 as i16;
            frame += 1;
        }

        let mut out = vec![0i16; num_out * 2];
        let written = if k % 97 == 96 {
            stretcher.flush().unwrap();
            stretcher.process(&[], 0, &mut out, num_out).unwrap()
        } else if k % 211 == 210 {
            stretcher.clear();
            stretcher.process(&input, num_in, &mut out, num_out).unwrap()
        } else {
            stretcher.process(&input, num_in, &mut out, num_out).unwrap()
        };

        hasher.update((written as u32).to_le_bytes());
        for sample in &out[..written * 2] {
            hasher.update(sample.to_le_bytes());
        }
    }

    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn a_quiet_signal_is_stretched_like_soundtouch() {
    assert_eq!(
        stretched_digest(500.0),
        "92411b0154da78b85b46f541eddf0b7715a00ef8ece7d207e7d2339730255e72"
    );
}

#[test]
fn a_medium_signal_is_stretched_like_soundtouch() {
    assert_eq!(
        stretched_digest(8000.0),
        "80e20a24a7da9063535fb9bf9e8404e460f767facf5549d254e3d1839dc449ab"
    );
}

#[test]
fn a_loud_signal_is_stretched_like_soundtouch() {
    assert_eq!(
        stretched_digest(20000.0),
        "4fbc9d79e47e8c81117ea4aaefc4c77726262fae9581485aee107c4c1f6301a1"
    );
}

#[test]
fn a_clipping_signal_is_stretched_like_soundtouch() {
    assert_eq!(
        stretched_digest(32000.0),
        "045e1864d1aa60bd3cbceda1f33e52fc5ecdc603c5ad66c7948bd20e683a801c"
    );
}
