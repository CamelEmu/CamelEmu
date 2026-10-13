use emu_pica_types::{F24, Vec4};

/// Uniforms and fixed attributes are written in a packed format such that four float24 values are
/// encoded in three 32-bit numbers. Uniforms can also encode four float32 values in four 32-bit
/// numbers. We write to internal memory once a full vector is written.
#[derive(Clone, Copy, Debug, Default)]
pub struct PackedAttribute {
    pub buffer: [u32; 4],
    pub index: u32,
}

impl PackedAttribute {
    /// Places a word to the queue and returns true if the queue becomes full.
    pub fn push(&mut self, word: u32, is_float32: bool) -> bool {
        self.buffer[self.index as usize] = word;
        self.index += 1;
        (self.index >= 4 && is_float32) || (self.index >= 3 && !is_float32)
    }

    /// Resets the queue discarding previous entries.
    pub fn reset(&mut self) {
        self.index = 0;
    }

    /// Returns the queue contents with either float24 or float32 interpretation.
    pub fn get(&mut self, is_float32: bool) -> Vec4<F24> {
        self.reset();
        if is_float32 {
            self.as_float32()
        } else {
            self.as_float24()
        }
    }

    /// Decodes the queue contents with float24 transfer mode.
    fn as_float24(&self) -> Vec4<F24> {
        let buffer = &self.buffer;
        let x = buffer[2] & 0xFFFFFF;
        let y = ((buffer[1] & 0xFFFF) << 8) | ((buffer[2] >> 24) & 0xFF);
        let z = ((buffer[0] & 0xFF) << 16) | ((buffer[1] >> 16) & 0xFFFF);
        let w = buffer[0] >> 8;
        Vec4 {
            x: F24::from_raw(x),
            y: F24::from_raw(y),
            z: F24::from_raw(z),
            w: F24::from_raw(w),
        }
    }

    /// Decodes the queue contents with float32 transfer mode.
    fn as_float32(&self) -> Vec4<F24> {
        let value = |i: usize| F24::from_float32(f32::from_bits(self.buffer[i]));
        Vec4 {
            x: value(3),
            y: value(2),
            z: value(1),
            w: value(0),
        }
    }
}
