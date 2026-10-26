use crate::error::{ImageError, Result};

fn required_len(stride: usize, row_bytes: usize, rows: usize) -> usize {
    if rows == 0 {
        0
    } else {
        stride * (rows - 1) + row_bytes
    }
}

#[derive(Clone, Copy)]
pub struct Plane<'a> {
    data: &'a [u8],
    stride: usize,
}

impl<'a> Plane<'a> {
    pub fn new(data: &'a [u8], stride: usize, row_bytes: usize, rows: usize) -> Result<Self> {
        let required = required_len(stride, row_bytes, rows);
        if data.len() < required {
            return Err(ImageError::ShortPlane {
                len: data.len(),
                required,
            });
        }
        Ok(Self { data, stride })
    }

    pub(crate) fn row(&self, y: usize) -> &'a [u8] {
        &self.data[y * self.stride..]
    }

    pub(crate) fn stride(&self) -> usize {
        self.stride
    }
}

pub struct PlaneMut<'a> {
    data: &'a mut [u8],
    stride: usize,
}

impl<'a> PlaneMut<'a> {
    pub fn new(data: &'a mut [u8], stride: usize, row_bytes: usize, rows: usize) -> Result<Self> {
        let required = required_len(stride, row_bytes, rows);
        if data.len() < required {
            return Err(ImageError::ShortPlane {
                len: data.len(),
                required,
            });
        }
        Ok(Self { data, stride })
    }

    pub(crate) fn row_mut(&mut self, y: usize) -> &mut [u8] {
        &mut self.data[y * self.stride..]
    }

    pub(crate) fn stride(&self) -> usize {
        self.stride
    }
}

pub(crate) fn copy_plane(src: Plane, dst: &mut PlaneMut, width: usize, height: usize) {
    for y in 0..height {
        dst.row_mut(y)[..width].copy_from_slice(&src.row(y)[..width]);
    }
}

pub(crate) fn mirror_plane(src: Plane, dst: &mut PlaneMut, width: usize, height: usize) {
    for y in 0..height {
        let s = src.row(y);
        let d = dst.row_mut(y);
        for x in 0..width {
            d[x] = s[width - 1 - x];
        }
    }
}
