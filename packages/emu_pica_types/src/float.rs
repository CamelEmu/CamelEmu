use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A PICA float with `M` bits of mantissa and `E` bits of exponent, held as an IEEE 754 single.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Float<const M: u32, const E: u32> {
    value: f32,
}

pub type F31 = Float<23, 7>;
pub type F24 = Float<16, 7>;
pub type F20 = Float<12, 7>;
pub type F16 = Float<10, 5>;

impl<const M: u32, const E: u32> Float<M, E> {
    pub const fn from_float32(value: f32) -> Self {
        Self { value }
    }

    pub const fn from_raw(mut hex: u32) -> Self {
        let width = M + E + 1;
        let bias: i32 = 128 - (1 << (E - 1));
        let mut exponent: i32 = ((hex >> M) & ((1 << E) - 1)) as i32;
        let mantissa = hex & ((1 << M) - 1);
        let sign = (hex >> (E + M)) << 31;

        if hex & ((1 << (width - 1)) - 1) != 0 {
            if exponent == (1 << E) - 1 {
                exponent = 255;
            } else {
                exponent += bias;
            }
            hex = sign | (mantissa << (23 - M)) | ((exponent as u32) << 23);
        } else {
            hex = sign;
        }

        Self {
            value: f32::from_bits(hex),
        }
    }

    pub const fn zero() -> Self {
        Self::from_float32(0.0)
    }

    pub const fn one() -> Self {
        Self::from_float32(1.0)
    }

    pub const fn to_float32(self) -> f32 {
        self.value
    }
}

impl<const M: u32, const E: u32> Mul for Float<M, E> {
    type Output = Self;

    fn mul(self, flt: Self) -> Self {
        let mut result = self.value * flt.to_float32();
        if result.is_nan() && !self.value.is_nan() && !flt.to_float32().is_nan() {
            result = 0.0;
        }
        Self::from_float32(result)
    }
}

impl<const M: u32, const E: u32> Div for Float<M, E> {
    type Output = Self;

    fn div(self, flt: Self) -> Self {
        Self::from_float32(self.to_float32() / flt.to_float32())
    }
}

impl<const M: u32, const E: u32> Add for Float<M, E> {
    type Output = Self;

    fn add(self, flt: Self) -> Self {
        Self::from_float32(self.to_float32() + flt.to_float32())
    }
}

impl<const M: u32, const E: u32> Sub for Float<M, E> {
    type Output = Self;

    fn sub(self, flt: Self) -> Self {
        Self::from_float32(self.to_float32() - flt.to_float32())
    }
}

impl<const M: u32, const E: u32> MulAssign for Float<M, E> {
    fn mul_assign(&mut self, flt: Self) {
        self.value = (*self * flt).value;
    }
}

impl<const M: u32, const E: u32> DivAssign for Float<M, E> {
    fn div_assign(&mut self, flt: Self) {
        self.value /= flt.to_float32();
    }
}

impl<const M: u32, const E: u32> AddAssign for Float<M, E> {
    fn add_assign(&mut self, flt: Self) {
        self.value += flt.to_float32();
    }
}

impl<const M: u32, const E: u32> SubAssign for Float<M, E> {
    fn sub_assign(&mut self, flt: Self) {
        self.value -= flt.to_float32();
    }
}

impl<const M: u32, const E: u32> Neg for Float<M, E> {
    type Output = Self;

    fn neg(self) -> Self {
        Self::from_float32(-self.to_float32())
    }
}

impl<const M: u32, const E: u32> PartialEq for Float<M, E> {
    fn eq(&self, flt: &Self) -> bool {
        self.to_float32() == flt.to_float32()
    }
}

impl<const M: u32, const E: u32> PartialOrd for Float<M, E> {
    fn partial_cmp(&self, flt: &Self) -> Option<Ordering> {
        self.to_float32().partial_cmp(&flt.to_float32())
    }
}
