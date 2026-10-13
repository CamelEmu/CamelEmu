#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rectangle<T> {
    pub left: T,
    pub top: T,
    pub right: T,
    pub bottom: T,
}

impl<T> Rectangle<T> {
    pub const fn new(left: T, top: T, right: T, bottom: T) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

macro_rules! impl_rectangle {
    ($ty:ty, $signed:ty) => {
        impl Rectangle<$ty> {
            pub const fn mul(self, value: $ty) -> Self {
                Self::new(
                    self.left.wrapping_mul(value),
                    self.top.wrapping_mul(value),
                    self.right.wrapping_mul(value),
                    self.bottom.wrapping_mul(value),
                )
            }

            pub const fn div(self, value: $ty) -> Self {
                Self::new(
                    self.left / value,
                    self.top / value,
                    self.right / value,
                    self.bottom / value,
                )
            }

            pub const fn get_width(&self) -> $ty {
                (self.right.wrapping_sub(self.left) as $signed).wrapping_abs() as $ty
            }

            pub const fn get_height(&self) -> $ty {
                (self.bottom.wrapping_sub(self.top) as $signed).wrapping_abs() as $ty
            }

            pub const fn translate_x(&self, x: $ty) -> Self {
                Self::new(
                    self.left.wrapping_add(x),
                    self.top,
                    self.right.wrapping_add(x),
                    self.bottom,
                )
            }

            pub const fn translate_y(&self, y: $ty) -> Self {
                Self::new(
                    self.left,
                    self.top.wrapping_add(y),
                    self.right,
                    self.bottom.wrapping_add(y),
                )
            }

            pub fn scale(&self, s: f32) -> Self {
                Self::new(
                    self.left,
                    self.top,
                    (self.left as f32 + self.get_width() as f32 * s) as $ty,
                    (self.top as f32 + self.get_height() as f32 * s) as $ty,
                )
            }

            pub const fn vertical_mirror(&self, ref_height: $ty) -> Self {
                Self::new(
                    self.left,
                    ref_height.wrapping_sub(self.bottom),
                    self.right,
                    ref_height.wrapping_sub(self.top),
                )
            }
        }
    };
}

impl_rectangle!(u32, i32);
impl_rectangle!(i32, i32);
