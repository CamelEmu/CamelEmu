use crate::error::{ImageError, Result};
use crate::i420::I420;
use crate::plane::{Plane, PlaneMut};

#[derive(Clone, Copy, Debug)]
pub struct YuvConstants {
    pub ub: i32,
    pub ug: i32,
    pub vg: i32,
    pub vr: i32,
    pub yg: i32,
    pub yb: i32,
    pub center: i32,
    pub shift: u32,
}

impl Default for YuvConstants {
    fn default() -> Self {
        Self {
            ub: 128,
            ug: 25,
            vg: 52,
            vr: 102,
            yg: 18997,
            yb: -1160,
            center: 128,
            shift: 6,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RgbToYuv {
    pub y_r: i32,
    pub y_g: i32,
    pub y_b: i32,
    pub y_bias: i32,
    pub u_b: i32,
    pub u_g: i32,
    pub u_r: i32,
    pub v_r: i32,
    pub v_g: i32,
    pub v_b: i32,
    pub uv_bias: i32,
}

impl Default for RgbToYuv {
    fn default() -> Self {
        Self {
            y_r: 66,
            y_g: 129,
            y_b: 25,
            y_bias: 0x1080,
            u_b: 112,
            u_g: -74,
            u_r: -38,
            v_r: 112,
            v_g: -94,
            v_b: -18,
            uv_bias: 0x8000,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Argb<'a> {
    plane: Plane<'a>,
    width: usize,
    height: usize,
}

impl<'a> Argb<'a> {
    pub fn new(data: &'a [u8], stride: usize, width: usize, height: usize) -> Result<Self> {
        if width == 0 || height == 0 {
            return Err(ImageError::InvalidSize {
                width: width as i32,
                height: height as i32,
            });
        }
        Ok(Self {
            plane: Plane::new(data, stride, width * 4, height)?,
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

fn destination_row(y: usize, height: usize, invert: bool) -> usize {
    if invert { height - 1 - y } else { y }
}

#[derive(Default)]
pub struct Converter {
    yuv: YuvConstants,
    rgb: RgbToYuv,
}

impl Converter {
    pub fn new(yuv: YuvConstants, rgb: RgbToYuv) -> Self {
        Self { yuv, rgb }
    }

    fn clamp(val: i32) -> u8 {
        val.clamp(0, 255) as u8
    }

    fn yuv_pixel(&self, y: u8, u: u8, v: u8) -> (u8, u8, u8) {
        let c = &self.yuv;
        let y32 = y as u32 * 0x0101;
        let y1 = ((y32 * c.yg as u32) >> 16) as i32 + c.yb;
        let ui = u as i32 - c.center;
        let vi = v as i32 - c.center;
        let b16 = y1 + ui * c.ub;
        let g16 = y1 - (ui * c.ug + vi * c.vg);
        let r16 = y1 + vi * c.vr;
        (
            Self::clamp(b16 >> c.shift),
            Self::clamp(g16 >> c.shift),
            Self::clamp(r16 >> c.shift),
        )
    }

    fn rgb_to_y(&self, r: i32, g: i32, b: i32) -> u8 {
        let c = &self.rgb;
        ((c.y_r * r + c.y_g * g + c.y_b * b + c.y_bias) >> 8) as u8
    }

    fn rgb_to_u(&self, r: i32, g: i32, b: i32) -> u8 {
        let c = &self.rgb;
        ((c.u_b * b + c.u_g * g + c.u_r * r + c.uv_bias) >> 8) as u8
    }

    fn rgb_to_v(&self, r: i32, g: i32, b: i32) -> u8 {
        let c = &self.rgb;
        ((c.v_r * r + c.v_g * g + c.v_b * b + c.uv_bias) >> 8) as u8
    }

    fn i422_to_rgb565_row(&self, y: &[u8], u: &[u8], v: &[u8], dst: &mut [u8], width: usize) {
        for x in 0..width {
            let (b, g, r) = self.yuv_pixel(y[x], u[x / 2], v[x / 2]);
            let pixel = (b >> 3) as u16 | (((g >> 2) as u16) << 5) | (((r >> 3) as u16) << 11);
            dst[2 * x..2 * x + 2].copy_from_slice(&pixel.to_le_bytes());
        }
    }

    pub fn i420_to_rgb565(&self, src: &I420, dst: &mut PlaneMut, invert: bool) -> Result<()> {
        for y in 0..src.height {
            self.i422_to_rgb565_row(
                src.y.row(y),
                src.u.row(y / 2),
                src.v.row(y / 2),
                dst.row_mut(destination_row(y, src.height, invert)),
                src.width,
            );
        }
        Ok(())
    }

    pub fn i420_to_yuy2(&self, src: &I420, dst: &mut PlaneMut, invert: bool) -> Result<()> {
        for y in 0..src.height {
            i422_to_yuy2_row(
                src.y.row(y),
                src.u.row(y / 2),
                src.v.row(y / 2),
                dst.row_mut(destination_row(y, src.height, invert)),
                src.width,
            );
        }
        Ok(())
    }

    pub fn argb_to_rgb565(&self, src: &Argb, dst: &mut PlaneMut, invert: bool) -> Result<()> {
        for y in 0..src.height {
            let s = src.plane.row(y);
            let d = dst.row_mut(destination_row(y, src.height, invert));
            for x in 0..src.width {
                let p = &s[4 * x..];
                let pixel = (p[0] >> 3) as u16 | (((p[1] >> 2) as u16) << 5) | (((p[2] >> 3) as u16) << 11);
                d[2 * x..2 * x + 2].copy_from_slice(&pixel.to_le_bytes());
            }
        }
        Ok(())
    }

    fn argb_to_yuy2_row(&self, src: &[u8], dst: &mut [u8], width: usize) {
        let chroma_width = width.div_ceil(2);
        let mut row_y = vec![0u8; width];
        let mut row_u = vec![0u8; chroma_width];
        let mut row_v = vec![0u8; chroma_width];

        for x in 0..width / 2 {
            let p = &src[8 * x..];
            let average = |offset: usize| (p[offset] as i32 + p[offset + 4] as i32 + 1) >> 1;
            let (ab, ag, ar) = (average(0), average(1), average(2));
            row_u[x] = self.rgb_to_u(ar, ag, ab);
            row_v[x] = self.rgb_to_v(ar, ag, ab);
        }
        if width & 1 != 0 {
            let p = &src[4 * (width - 1)..];
            let (ab, ag, ar) = (p[0] as i32, p[1] as i32, p[2] as i32);
            row_u[width / 2] = self.rgb_to_u(ar, ag, ab);
            row_v[width / 2] = self.rgb_to_v(ar, ag, ab);
        }

        for (x, y) in row_y.iter_mut().enumerate() {
            let p = &src[4 * x..];
            *y = self.rgb_to_y(p[2] as i32, p[1] as i32, p[0] as i32);
        }

        i422_to_yuy2_row(&row_y, &row_u, &row_v, dst, width);
    }

    pub fn argb_to_yuy2(&self, src: &Argb, dst: &mut PlaneMut, invert: bool) -> Result<()> {
        if !invert && src.plane.stride() == src.width * 4 && dst.stride() == src.width * 2 {
            self.argb_to_yuy2_row(src.plane.row(0), dst.row_mut(0), src.width * src.height);
            return Ok(());
        }
        for y in 0..src.height {
            self.argb_to_yuy2_row(
                src.plane.row(y),
                dst.row_mut(destination_row(y, src.height, invert)),
                src.width,
            );
        }
        Ok(())
    }
}

fn i422_to_yuy2_row(y: &[u8], u: &[u8], v: &[u8], dst: &mut [u8], width: usize) {
    for x in 0..width / 2 {
        dst[4 * x] = y[2 * x];
        dst[4 * x + 1] = u[x];
        dst[4 * x + 2] = y[2 * x + 1];
        dst[4 * x + 3] = v[x];
    }
    if width & 1 != 0 {
        let x = width / 2;
        dst[4 * x] = y[2 * x];
        dst[4 * x + 1] = u[x];
        dst[4 * x + 2] = 0;
        dst[4 * x + 3] = v[x];
    }
}

pub fn abgr_to_argb(src: &Argb, dst: &mut PlaneMut) -> Result<()> {
    for y in 0..src.height {
        let s = src.plane.row(y);
        let d = dst.row_mut(y);
        for x in 0..src.width {
            d[4 * x] = s[4 * x + 2];
            d[4 * x + 1] = s[4 * x + 1];
            d[4 * x + 2] = s[4 * x];
            d[4 * x + 3] = s[4 * x + 3];
        }
    }
    Ok(())
}

pub fn argb_mirror(src: &Argb, dst: &mut PlaneMut) -> Result<()> {
    for y in 0..src.height {
        let s = src.plane.row(y);
        let d = dst.row_mut(y);
        for x in 0..src.width {
            d[4 * x..4 * x + 4].copy_from_slice(&s[4 * (src.width - 1 - x)..4 * (src.width - x)]);
        }
    }
    Ok(())
}
