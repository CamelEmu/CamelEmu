use crate::error::{ImageError, Result};
use crate::i420::{I420Mut, half};
use crate::plane::{Plane, copy_plane};

#[derive(Clone, Copy)]
pub struct Android420<'a> {
    y: Plane<'a>,
    u: Plane<'a>,
    v: Plane<'a>,
    pixel_stride_uv: usize,
    width: usize,
    height: usize,
}

pub struct Android420Strides {
    pub y: usize,
    pub u: usize,
    pub v: usize,
    pub pixel_uv: usize,
}

impl<'a> Android420<'a> {
    pub fn new(
        y: &'a [u8],
        u: &'a [u8],
        v: &'a [u8],
        strides: Android420Strides,
        width: usize,
        height: usize,
    ) -> Result<Self> {
        if width == 0 || height == 0 || strides.pixel_uv == 0 {
            return Err(ImageError::InvalidSize {
                width: width as i32,
                height: height as i32,
            });
        }
        let chroma_row_bytes = (half(width) - 1) * strides.pixel_uv + 1;
        Ok(Self {
            y: Plane::new(y, strides.y, width, height)?,
            u: Plane::new(u, strides.u, chroma_row_bytes, half(height))?,
            v: Plane::new(v, strides.v, chroma_row_bytes, half(height))?,
            pixel_stride_uv: strides.pixel_uv,
            width,
            height,
        })
    }
}

pub fn android420_to_i420(src: &Android420, dst: &mut I420Mut) -> Result<()> {
    if dst.width != src.width || dst.height != src.height {
        return Err(ImageError::InvalidSize {
            width: dst.width as i32,
            height: dst.height as i32,
        });
    }
    let (half_width, half_height) = (half(src.width), half(src.height));

    copy_plane(src.y, &mut dst.y, src.width, src.height);

    if src.pixel_stride_uv == 1 {
        copy_plane(src.u, &mut dst.u, half_width, half_height);
        copy_plane(src.v, &mut dst.v, half_width, half_height);
        return Ok(());
    }

    for y in 0..half_height {
        let (su, sv) = (src.u.row(y), src.v.row(y));
        let (du, dv) = (dst.u.row_mut(y), dst.v.row_mut(y));
        for i in 0..half_width {
            du[i] = su[i * src.pixel_stride_uv];
            dv[i] = sv[i * src.pixel_stride_uv];
        }
    }
    Ok(())
}
