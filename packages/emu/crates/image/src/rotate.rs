use crate::error::{ImageError, Result};
use crate::i420::{I420, I420Mut, half};
use crate::plane::{Plane, PlaneMut, copy_plane, mirror_plane};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotationMode {
    Rotate0,
    Rotate90,
    Rotate180,
    Rotate270,
}

impl TryFrom<i32> for RotationMode {
    type Error = ImageError;

    fn try_from(rotation: i32) -> Result<Self> {
        match rotation {
            0 => Ok(Self::Rotate0),
            90 => Ok(Self::Rotate90),
            180 => Ok(Self::Rotate180),
            270 => Ok(Self::Rotate270),
            rotation => Err(ImageError::InvalidRotation { rotation }),
        }
    }
}

impl RotationMode {
    fn swaps_size(self) -> bool {
        matches!(self, Self::Rotate90 | Self::Rotate270)
    }
}

fn rotate_plane(src: Plane, dst: &mut PlaneMut, width: usize, height: usize, mode: RotationMode) {
    match mode {
        RotationMode::Rotate0 => copy_plane(src, dst, width, height),
        RotationMode::Rotate90 => {
            for i in 0..width {
                for (j, d) in dst.row_mut(i).iter_mut().enumerate().take(height) {
                    *d = src.row(height - 1 - j)[i];
                }
            }
        }
        RotationMode::Rotate180 => {
            for y in 0..height {
                let s = src.row(height - 1 - y);
                let d = dst.row_mut(y);
                for x in 0..width {
                    d[x] = s[width - 1 - x];
                }
            }
        }
        RotationMode::Rotate270 => {
            for i in 0..width {
                for (j, d) in dst.row_mut(i).iter_mut().enumerate().take(height) {
                    *d = src.row(j)[width - 1 - i];
                }
            }
        }
    }
}

pub fn i420_rotate(src: &I420, dst: &mut I420Mut, mode: RotationMode) -> Result<()> {
    let (expected_width, expected_height) = if mode.swaps_size() {
        (src.height, src.width)
    } else {
        (src.width, src.height)
    };
    if dst.width != expected_width || dst.height != expected_height {
        return Err(ImageError::InvalidSize {
            width: dst.width as i32,
            height: dst.height as i32,
        });
    }

    rotate_plane(src.y, &mut dst.y, src.width, src.height, mode);
    rotate_plane(src.u, &mut dst.u, half(src.width), half(src.height), mode);
    rotate_plane(src.v, &mut dst.v, half(src.width), half(src.height), mode);
    Ok(())
}

pub fn i420_mirror(src: &I420, dst: &mut I420Mut) -> Result<()> {
    if dst.width != src.width || dst.height != src.height {
        return Err(ImageError::InvalidSize {
            width: dst.width as i32,
            height: dst.height as i32,
        });
    }

    mirror_plane(src.y, &mut dst.y, src.width, src.height);
    mirror_plane(src.u, &mut dst.u, half(src.width), half(src.height));
    mirror_plane(src.v, &mut dst.v, half(src.width), half(src.height));
    Ok(())
}
