use std::ops::{Deref, DerefMut};

/// A value placed on a 16-byte boundary, as `alignas(16)` does.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct Aligned<T>(pub T);

impl<T> Deref for Aligned<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Aligned<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
