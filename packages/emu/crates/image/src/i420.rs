use crate::error::{ImageError, Result};
use crate::plane::{Plane, PlaneMut};

pub(crate) fn half(value: usize) -> usize {
    (value + 1) >> 1
}

fn check_size(width: usize, height: usize) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(ImageError::InvalidSize {
            width: width as i32,
            height: height as i32,
        });
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub struct I420<'a> {
    pub(crate) y: Plane<'a>,
    pub(crate) u: Plane<'a>,
    pub(crate) v: Plane<'a>,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

pub struct Strides {
    pub y: usize,
    pub u: usize,
    pub v: usize,
}

impl<'a> I420<'a> {
    pub fn new(
        y: &'a [u8],
        u: &'a [u8],
        v: &'a [u8],
        strides: Strides,
        width: usize,
        height: usize,
    ) -> Result<Self> {
        check_size(width, height)?;
        let (half_width, half_height) = (half(width), half(height));
        Ok(Self {
            y: Plane::new(y, strides.y, width, height)?,
            u: Plane::new(u, strides.u, half_width, half_height)?,
            v: Plane::new(v, strides.v, half_width, half_height)?,
            width,
            height,
        })
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }
}

pub struct I420Mut<'a> {
    pub(crate) y: PlaneMut<'a>,
    pub(crate) u: PlaneMut<'a>,
    pub(crate) v: PlaneMut<'a>,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

impl<'a> I420Mut<'a> {
    pub fn new(
        y: &'a mut [u8],
        u: &'a mut [u8],
        v: &'a mut [u8],
        strides: Strides,
        width: usize,
        height: usize,
    ) -> Result<Self> {
        check_size(width, height)?;
        let (half_width, half_height) = (half(width), half(height));
        Ok(Self {
            y: PlaneMut::new(y, strides.y, width, height)?,
            u: PlaneMut::new(u, strides.u, half_width, half_height)?,
            v: PlaneMut::new(v, strides.v, half_width, half_height)?,
            width,
            height,
        })
    }
}
