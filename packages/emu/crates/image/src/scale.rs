use crate::error::{ImageError, Result};
use crate::i420::{I420, I420Mut, half};
use crate::plane::{Plane, PlaneMut, copy_plane};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterMode {
    None,
    Linear,
    Bilinear,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendMode {
    Arm16,
    Intel7,
}

impl Default for BlendMode {
    fn default() -> Self {
        if cfg!(any(target_arch = "arm", target_arch = "aarch64")) {
            Self::Arm16
        } else {
            Self::Intel7
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScaleSettings {
    pub blend: BlendMode,
    pub half_pixel: i32,
    pub max_dimension: i32,
    pub up2_start_limit: i32,
    pub linear_ratio: i32,
}

impl Default for ScaleSettings {
    fn default() -> Self {
        Self {
            blend: BlendMode::default(),
            half_pixel: 32768,
            max_dimension: 32768,
            up2_start_limit: 0x8000,
            linear_ratio: 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub width: i32,
    pub height: i32,
}

#[derive(Default)]
struct Slope {
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
}

fn fixed_div(num: i32, div: i32) -> i32 {
    (((num as i64) << 16) / div as i64) as i32
}

fn fixed_div1(num: i32, div: i32) -> i32 {
    ((((num as i64) << 16) - 0x00010001) / (div - 1) as i64) as i32
}

fn center_start(dx: i32, s: i32) -> i32 {
    if dx < 0 {
        -((-dx >> 1) + s)
    } else {
        (dx >> 1) + s
    }
}

fn scale_row_down2(s: &[u8], dst: &mut [u8], dst_width: usize) {
    for (x, d) in dst.iter_mut().enumerate().take(dst_width) {
        *d = s[2 * x + 1];
    }
}

fn scale_row_down2_linear(s: &[u8], dst: &mut [u8], dst_width: usize) {
    for (x, d) in dst.iter_mut().enumerate().take(dst_width) {
        *d = ((s[2 * x] as i32 + s[2 * x + 1] as i32 + 1) >> 1) as u8;
    }
}

fn scale_row_down2_box(s: &[u8], t: &[u8], dst: &mut [u8], dst_width: usize) {
    for (x, d) in dst.iter_mut().enumerate().take(dst_width) {
        *d = ((s[2 * x] as i32 + s[2 * x + 1] as i32 + t[2 * x] as i32 + t[2 * x + 1] as i32 + 2)
            >> 2) as u8;
    }
}

fn scale_row_down34(s: &[u8], dst: &mut [u8], dst_width: usize) {
    for x in (0..dst_width).step_by(3) {
        let src = &s[x / 3 * 4..];
        dst[x] = src[0];
        dst[x + 1] = src[1];
        dst[x + 2] = src[3];
    }
}

fn down34_filtered(r: &[u8]) -> [i32; 3] {
    [
        (r[0] as i32 * 3 + r[1] as i32 + 2) >> 2,
        (r[1] as i32 + r[2] as i32 + 1) >> 1,
        (r[2] as i32 + r[3] as i32 * 3 + 2) >> 2,
    ]
}

fn scale_row_down34_0_box(s: &[u8], t: &[u8], dst: &mut [u8], dst_width: usize) {
    for x in (0..dst_width).step_by(3) {
        let a = down34_filtered(&s[x / 3 * 4..]);
        let b = down34_filtered(&t[x / 3 * 4..]);
        for i in 0..3 {
            dst[x + i] = ((a[i] * 3 + b[i] + 2) >> 2) as u8;
        }
    }
}

fn scale_row_down34_1_box(s: &[u8], t: &[u8], dst: &mut [u8], dst_width: usize) {
    for x in (0..dst_width).step_by(3) {
        let a = down34_filtered(&s[x / 3 * 4..]);
        let b = down34_filtered(&t[x / 3 * 4..]);
        for i in 0..3 {
            dst[x + i] = ((a[i] + b[i] + 1) >> 1) as u8;
        }
    }
}

fn scale_row_down38(s: &[u8], dst: &mut [u8], dst_width: usize) {
    for x in (0..dst_width).step_by(3) {
        let src = &s[x / 3 * 8..];
        dst[x] = src[0];
        dst[x + 1] = src[3];
        dst[x + 2] = src[6];
    }
}

fn scale_row_down38_3_box(src: &[u8], stride: usize, dst: &mut [u8], dst_width: usize) {
    for x in (0..dst_width).step_by(3) {
        let p = &src[x / 3 * 8..];
        let sum = |offsets: &[usize]| -> i32 {
            offsets
                .iter()
                .flat_map(|&o| [p[o], p[stride + o], p[stride * 2 + o]])
                .map(i32::from)
                .sum()
        };
        dst[x] = ((sum(&[0, 1, 2]) * (65536 / 9)) >> 16) as u8;
        dst[x + 1] = ((sum(&[3, 4, 5]) * (65536 / 9)) >> 16) as u8;
        let tail: i32 = [6, 7]
            .iter()
            .flat_map(|&o| [p[o], p[stride + o], p[stride * 2 + o]])
            .map(i32::from)
            .sum();
        dst[x + 2] = ((tail * (65536 / 6)) >> 16) as u8;
    }
}

fn scale_row_down38_2_box(src: &[u8], stride: usize, dst: &mut [u8], dst_width: usize) {
    for x in (0..dst_width).step_by(3) {
        let p = &src[x / 3 * 8..];
        let sum = |offsets: &[usize]| -> i32 {
            offsets
                .iter()
                .flat_map(|&o| [p[o], p[stride + o]])
                .map(i32::from)
                .sum()
        };
        dst[x] = ((sum(&[0, 1, 2]) * (65536 / 6)) >> 16) as u8;
        dst[x + 1] = ((sum(&[3, 4, 5]) * (65536 / 6)) >> 16) as u8;
        dst[x + 2] = ((sum(&[6, 7]) * (65536 / 4)) >> 16) as u8;
    }
}

fn scale_row_up2_linear(src: &[u8], dst: &mut [u8], dst_width: usize) {
    let src_width = dst_width >> 1;
    for x in 0..src_width {
        dst[2 * x] = ((src[x] as i32 * 3 + src[x + 1] as i32 + 2) >> 2) as u8;
        dst[2 * x + 1] = ((src[x] as i32 + src[x + 1] as i32 * 3 + 2) >> 2) as u8;
    }
}

fn scale_row_up2_linear_any(src: &[u8], dst: &mut [u8], dst_width: usize) {
    let work_width = (dst_width - 1) & !1;
    dst[0] = src[0];
    if work_width > 0 {
        scale_row_up2_linear(src, &mut dst[1..], work_width);
    }
    dst[dst_width - 1] = src[(dst_width - 1) / 2];
}

fn scale_row_up2_bilinear(
    s: &[u8],
    t: &[u8],
    dst: &mut [u8],
    d_off: usize,
    e_off: usize,
    dst_width: usize,
) {
    let src_width = dst_width >> 1;
    for x in 0..src_width {
        let (s0, s1) = (s[x] as i32, s[x + 1] as i32);
        let (t0, t1) = (t[x] as i32, t[x + 1] as i32);
        dst[d_off + 2 * x] = ((s0 * 9 + s1 * 3 + t0 * 3 + t1 + 8) >> 4) as u8;
        dst[d_off + 2 * x + 1] = ((s0 * 3 + s1 * 9 + t0 + t1 * 3 + 8) >> 4) as u8;
        dst[e_off + 2 * x] = ((s0 * 3 + s1 + t0 * 9 + t1 * 3 + 8) >> 4) as u8;
        dst[e_off + 2 * x + 1] = ((s0 + s1 * 3 + t0 * 3 + t1 * 9 + 8) >> 4) as u8;
    }
}

fn scale_row_up2_bilinear_any(
    sa: &[u8],
    sb: &[u8],
    dst: &mut [u8],
    da: usize,
    db: usize,
    dst_width: usize,
) {
    let work_width = (dst_width - 1) & !1;
    dst[da] = ((3 * sa[0] as i32 + sb[0] as i32 + 2) >> 2) as u8;
    dst[db] = ((sa[0] as i32 + 3 * sb[0] as i32 + 2) >> 2) as u8;
    if work_width > 0 {
        scale_row_up2_bilinear(sa, sb, dst, da + 1, db + 1, work_width);
    }
    let last = (dst_width - 1) / 2;
    dst[da + dst_width - 1] = ((3 * sa[last] as i32 + sb[last] as i32 + 2) >> 2) as u8;
    dst[db + dst_width - 1] = ((sa[last] as i32 + 3 * sb[last] as i32 + 2) >> 2) as u8;
}

fn scale_cols(dst: &mut [u8], src: &[u8], dst_width: usize, x: i32, dx: i32) {
    let mut x = x;
    for d in dst.iter_mut().take(dst_width) {
        *d = src[(x >> 16) as usize];
        x += dx;
    }
}

fn scale_cols_up2(dst: &mut [u8], src: &[u8], dst_width: usize) {
    for j in 0..dst_width / 2 {
        dst[2 * j] = src[j];
        dst[2 * j + 1] = src[j];
    }
    if dst_width & 1 != 0 {
        dst[dst_width - 1] = src[dst_width / 2];
    }
}

fn interpolate_row(dst: &mut [u8], s0: &[u8], s1: &[u8], width: usize, source_y_fraction: i32) {
    let y1_fraction = source_y_fraction;
    let y0_fraction = 256 - y1_fraction;

    if y1_fraction == 0 {
        dst[..width].copy_from_slice(&s0[..width]);
        return;
    }
    if y1_fraction == 128 {
        for x in 0..width {
            dst[x] = ((s0[x] as i32 + s1[x] as i32 + 1) >> 1) as u8;
        }
        return;
    }
    for x in 0..width {
        dst[x] = ((s0[x] as i32 * y0_fraction + s1[x] as i32 * y1_fraction + 128) >> 8) as u8;
    }
}

#[derive(Default)]
pub struct Scaler {
    settings: ScaleSettings,
}

impl Scaler {
    pub fn new(settings: ScaleSettings) -> Self {
        Self { settings }
    }

    fn blender(&self, a: i32, b: i32, f: i32) -> u8 {
        match self.settings.blend {
            BlendMode::Arm16 => (a + (((f * (b - a)) + 0x8000) >> 16)) as u8,
            BlendMode::Intel7 => (a + ((((f >> 9) * (b - a)) + 0x40) >> 7)) as u8,
        }
    }

    fn scale_filter_cols(&self, dst: &mut [u8], src: &[u8], dst_width: usize, x: i32, dx: i32) {
        let mut x = x as i64;
        for d in dst.iter_mut().take(dst_width) {
            let xi = (x >> 16) as usize;
            *d = self.blender(src[xi] as i32, src[xi + 1] as i32, (x & 0xffff) as i32);
            x += dx as i64;
        }
    }

    fn filter_reduce(&self, src: Size, dst: Size, filtering: FilterMode) -> FilterMode {
        let mut filtering = filtering;
        let ratio = self.settings.linear_ratio;
        if filtering == FilterMode::Bilinear {
            if src.height == 1 {
                filtering = FilterMode::Linear;
            }
            if dst.height == src.height || dst.height * ratio == src.height {
                filtering = FilterMode::Linear;
            }
            if src.width == 1 {
                filtering = FilterMode::None;
            }
        }
        if filtering == FilterMode::Linear {
            if src.width == 1 {
                filtering = FilterMode::None;
            }
            if dst.width == src.width || dst.width * ratio == src.width {
                filtering = FilterMode::None;
            }
        }
        filtering
    }

    fn slope(&self, src: Size, dst: Size, filtering: FilterMode) -> Slope {
        let half_pixel = self.settings.half_pixel;
        let mut slope = Slope::default();
        let mut dst_width = dst.width;
        let mut dst_height = dst.height;
        if dst_width == 1 && src.width >= self.settings.max_dimension {
            dst_width = src.width;
        }
        if dst_height == 1 && src.height >= self.settings.max_dimension {
            dst_height = src.height;
        }
        match filtering {
            FilterMode::Bilinear => {
                if dst_width <= src.width {
                    slope.dx = fixed_div(src.width, dst_width);
                    slope.x = center_start(slope.dx, -half_pixel);
                } else if src.width > 1 && dst_width > 1 {
                    slope.dx = fixed_div1(src.width, dst_width);
                    slope.x = 0;
                }
                if dst_height <= src.height {
                    slope.dy = fixed_div(src.height, dst_height);
                    slope.y = center_start(slope.dy, -half_pixel);
                } else if src.height > 1 && dst_height > 1 {
                    slope.dy = fixed_div1(src.height, dst_height);
                    slope.y = 0;
                }
            }
            FilterMode::Linear => {
                if dst_width <= src.width {
                    slope.dx = fixed_div(src.width, dst_width);
                    slope.x = center_start(slope.dx, -half_pixel);
                } else if src.width > 1 && dst_width > 1 {
                    slope.dx = fixed_div1(src.width, dst_width);
                    slope.x = 0;
                }
                slope.dy = fixed_div(src.height, dst_height);
                slope.y = slope.dy >> 1;
            }
            FilterMode::None => {
                slope.dx = fixed_div(src.width, dst_width);
                slope.dy = fixed_div(src.height, dst_height);
                slope.x = center_start(slope.dx, 0);
                slope.y = center_start(slope.dy, 0);
            }
        }
        slope
    }

    fn scale_plane_down2(
        &self,
        src: Plane,
        dst: &mut PlaneMut,
        dst_size: Size,
        filtering: FilterMode,
    ) {
        let mut r = if filtering == FilterMode::None { 1 } else { 0 };
        let width = dst_size.width as usize;
        for y in 0..dst_size.height as usize {
            let s = src.row(r);
            let d = dst.row_mut(y);
            match filtering {
                FilterMode::None => scale_row_down2(s, d, width),
                FilterMode::Linear => scale_row_down2_linear(s, d, width),
                FilterMode::Bilinear => scale_row_down2_box(s, src.row(r + 1), d, width),
            }
            r += 2;
        }
    }

    fn scale_plane_down34(
        &self,
        src: Plane,
        dst: &mut PlaneMut,
        dst_size: Size,
        filtering: FilterMode,
    ) {
        let width = dst_size.width as usize;
        let height = dst_size.height;
        let filter_rows = if filtering == FilterMode::Linear { 0 } else { 1 };
        let row0 = |s: &[u8], t: &[u8], d: &mut [u8]| {
            if filtering == FilterMode::None {
                scale_row_down34(s, d, width);
            } else {
                scale_row_down34_0_box(s, t, d, width);
            }
        };
        let row1 = |s: &[u8], t: &[u8], d: &mut [u8]| {
            if filtering == FilterMode::None {
                scale_row_down34(s, d, width);
            } else {
                scale_row_down34_1_box(s, t, d, width);
            }
        };

        let mut r = 0usize;
        let mut y = 0usize;
        while (y as i32) < height - 2 {
            row0(src.row(r), src.row(r + filter_rows), dst.row_mut(y));
            r += 1;
            y += 1;
            row1(src.row(r), src.row(r + filter_rows), dst.row_mut(y));
            r += 1;
            y += 1;
            row0(src.row(r + 1), src.row(r + 1 - filter_rows), dst.row_mut(y));
            r += 2;
            y += 1;
        }

        if height % 3 == 2 {
            row0(src.row(r), src.row(r + filter_rows), dst.row_mut(y));
            r += 1;
            y += 1;
            row1(src.row(r), src.row(r), dst.row_mut(y));
        } else if height % 3 == 1 {
            row0(src.row(r), src.row(r), dst.row_mut(y));
        }
    }

    fn scale_plane_down38(
        &self,
        src: Plane,
        dst: &mut PlaneMut,
        dst_size: Size,
        filtering: FilterMode,
    ) {
        let width = dst_size.width as usize;
        let height = dst_size.height;
        let filter_stride = if filtering == FilterMode::Linear {
            0
        } else {
            src.stride()
        };
        let row3 = |s: &[u8], stride: usize, d: &mut [u8]| {
            if filtering == FilterMode::None {
                scale_row_down38(s, d, width);
            } else {
                scale_row_down38_3_box(s, stride, d, width);
            }
        };
        let row2 = |s: &[u8], stride: usize, d: &mut [u8]| {
            if filtering == FilterMode::None {
                scale_row_down38(s, d, width);
            } else {
                scale_row_down38_2_box(s, stride, d, width);
            }
        };

        let mut r = 0usize;
        let mut y = 0usize;
        while (y as i32) < height - 2 {
            row3(src.row(r), filter_stride, dst.row_mut(y));
            r += 3;
            y += 1;
            row3(src.row(r), filter_stride, dst.row_mut(y));
            r += 3;
            y += 1;
            row2(src.row(r), filter_stride, dst.row_mut(y));
            r += 2;
            y += 1;
        }

        if height % 3 == 2 {
            row3(src.row(r), filter_stride, dst.row_mut(y));
            r += 3;
            y += 1;
            row3(src.row(r), 0, dst.row_mut(y));
        } else if height % 3 == 1 {
            row3(src.row(r), 0, dst.row_mut(y));
        }
    }

    fn scale_plane_vertical(
        &self,
        src: Plane,
        src_height: i32,
        dst: &mut PlaneMut,
        dst_size: Size,
        step: Slope,
        filtering: FilterMode,
    ) {
        let width = dst_size.width as usize;
        let max_y = if src_height > 1 {
            ((src_height - 1) << 16) - 1
        } else {
            0
        };
        let mut y = step.y;
        for j in 0..dst_size.height as usize {
            if y > max_y {
                y = max_y;
            }
            let yi = (y >> 16) as usize;
            let yf = if filtering != FilterMode::None {
                (y >> 8) & 255
            } else {
                0
            };
            let s0 = src.row(yi);
            let s1 = if yf == 0 { s0 } else { src.row(yi + 1) };
            interpolate_row(dst.row_mut(j), s0, s1, width, yf);
            y += step.dy;
        }
    }

    fn scale_plane_bilinear_down(
        &self,
        src: Plane,
        src_size: Size,
        dst: &mut PlaneMut,
        dst_size: Size,
        filtering: FilterMode,
    ) {
        let slope = self.slope(src_size, dst_size, filtering);
        let src_width = src_size.width as usize;
        let dst_width = dst_size.width as usize;
        let max_y = (src_size.height - 1) << 16;
        let mut row = vec![0u8; src_width];
        let x = slope.x;
        let dx = slope.dx;
        let mut y = slope.y;
        if y > max_y {
            y = max_y;
        }

        for j in 0..dst_size.height as usize {
            let yi = (y >> 16) as usize;
            if filtering == FilterMode::Linear {
                self.scale_filter_cols(dst.row_mut(j), src.row(yi), dst_width, x, dx);
            } else {
                let yf = (y >> 8) & 255;
                let s0 = src.row(yi);
                let s1 = if yf == 0 { s0 } else { src.row(yi + 1) };
                interpolate_row(&mut row, s0, s1, src_width, yf);
                self.scale_filter_cols(dst.row_mut(j), &row, dst_width, x, dx);
            }
            y += slope.dy;
            if y > max_y {
                y = max_y;
            }
        }
    }

    fn scale_plane_bilinear_up(
        &self,
        src: Plane,
        src_size: Size,
        dst: &mut PlaneMut,
        dst_size: Size,
        filtering: FilterMode,
    ) {
        let slope = self.slope(src_size, dst_size, filtering);
        let dst_width = dst_size.width as usize;
        let max_y = (src_size.height - 1) << 16;
        let x = slope.x;
        let dx = slope.dx;
        let mut y = slope.y;

        if y > max_y {
            y = max_y;
        }
        let mut yi = y >> 16;
        let mut src_row = yi as usize;

        let row_size = (dst_width + 31) & !31;
        let mut row = vec![0u8; row_size * 2];
        let mut rowptr: isize = 0;
        let mut rowstride: isize = row_size as isize;
        let mut lasty = yi;

        self.scale_filter_cols(&mut row[..], src.row(src_row), dst_width, x, dx);
        if src_size.height > 1 {
            src_row += 1;
        }
        self.scale_filter_cols(&mut row[row_size..], src.row(src_row), dst_width, x, dx);
        if src_size.height > 2 {
            src_row += 1;
        }

        for j in 0..dst_size.height as usize {
            yi = y >> 16;
            if yi != lasty {
                if y > max_y {
                    y = max_y;
                    yi = y >> 16;
                    src_row = yi as usize;
                }
                if yi != lasty {
                    self.scale_filter_cols(
                        &mut row[rowptr as usize..],
                        src.row(src_row),
                        dst_width,
                        x,
                        dx,
                    );
                    rowptr += rowstride;
                    rowstride = -rowstride;
                    lasty = yi;
                    if (y + 65536) < max_y {
                        src_row += 1;
                    }
                }
            }
            let s0 = &row[rowptr as usize..];
            if filtering == FilterMode::Linear {
                interpolate_row(dst.row_mut(j), s0, s0, dst_width, 0);
            } else {
                let yf = (y >> 8) & 255;
                let s1 = &row[(rowptr + rowstride) as usize..];
                interpolate_row(dst.row_mut(j), s0, s1, dst_width, yf);
            }
            y += slope.dy;
        }
    }

    fn scale_plane_up2_linear(&self, src: Plane, src_height: i32, dst: &mut PlaneMut, dst_size: Size) {
        let dst_width = dst_size.width as usize;
        if dst_size.height == 1 {
            scale_row_up2_linear_any(
                src.row(((src_height - 1) / 2) as usize),
                dst.row_mut(0),
                dst_width,
            );
        } else {
            let dy = fixed_div(src_height - 1, dst_size.height - 1);
            let mut y = (1 << 15) - 1;
            for i in 0..dst_size.height as usize {
                scale_row_up2_linear_any(src.row((y >> 16) as usize), dst.row_mut(i), dst_width);
                y += dy;
            }
        }
    }

    fn scale_plane_up2_bilinear(
        &self,
        src: Plane,
        src_height: i32,
        dst: &mut PlaneMut,
        dst_size: Size,
    ) {
        let dst_width = dst_size.width as usize;
        let stride = dst.stride();
        let mut r = 0usize;
        let mut d = 0usize;
        let s = src.row(r);
        scale_row_up2_bilinear_any(s, s, dst.row_mut(d), 0, 0, dst_width);
        d += 1;
        for _ in 0..src_height - 1 {
            scale_row_up2_bilinear_any(src.row(r), src.row(r + 1), dst.row_mut(d), 0, stride, dst_width);
            r += 1;
            d += 2;
        }
        if dst_size.height & 1 == 0 {
            let s = src.row(r);
            scale_row_up2_bilinear_any(s, s, dst.row_mut(d), 0, 0, dst_width);
        }
    }

    fn scale_plane_simple(&self, src: Plane, src_size: Size, dst: &mut PlaneMut, dst_size: Size) {
        let slope = self.slope(src_size, dst_size, FilterMode::None);
        let dst_width = dst_size.width as usize;
        let up2 = src_size.width * 2 == dst_size.width && slope.x < self.settings.up2_start_limit;
        let mut y = slope.y;
        for i in 0..dst_size.height as usize {
            let s = src.row((y >> 16) as usize);
            if up2 {
                scale_cols_up2(dst.row_mut(i), s, dst_width);
            } else {
                scale_cols(dst.row_mut(i), s, dst_width, slope.x, slope.dx);
            }
            y += slope.dy;
        }
    }

    pub fn scale_plane(
        &self,
        src: Plane,
        src_size: Size,
        dst: &mut PlaneMut,
        dst_size: Size,
        filtering: FilterMode,
    ) -> Result<()> {
        let filtering = self.filter_reduce(src_size, dst_size, filtering);
        let (src_width, src_height) = (src_size.width, src_size.height);
        let (dst_width, dst_height) = (dst_size.width, dst_size.height);

        if dst_width == src_width && dst_height == src_height {
            copy_plane(src, dst, dst_width as usize, dst_height as usize);
            return Ok(());
        }
        if dst_width == src_width {
            let mut dy = 0;
            let mut y = 0;
            if dst_height <= src_height {
                dy = fixed_div(src_height, dst_height);
                y = center_start(dy, -self.settings.half_pixel);
            } else if src_height > 1 && dst_height > 1 {
                dy = fixed_div1(src_height, dst_height);
            }
            let step = Slope {
                y,
                dy,
                ..Slope::default()
            };
            self.scale_plane_vertical(src, src_height, dst, dst_size, step, filtering);
            return Ok(());
        }
        if dst_width <= src_width && dst_height <= src_height {
            if 4 * dst_width == 3 * src_width && 4 * dst_height == 3 * src_height {
                self.scale_plane_down34(src, dst, dst_size, filtering);
                return Ok(());
            }
            if 2 * dst_width == src_width && 2 * dst_height == src_height {
                self.scale_plane_down2(src, dst, dst_size, filtering);
                return Ok(());
            }
            if 8 * dst_width == 3 * src_width && 8 * dst_height == 3 * src_height {
                self.scale_plane_down38(src, dst, dst_size, filtering);
                return Ok(());
            }
        }
        if (dst_width + 1) / 2 == src_width && filtering == FilterMode::Linear {
            self.scale_plane_up2_linear(src, src_height, dst, dst_size);
            return Ok(());
        }
        if (dst_height + 1) / 2 == src_height
            && (dst_width + 1) / 2 == src_width
            && filtering == FilterMode::Bilinear
        {
            self.scale_plane_up2_bilinear(src, src_height, dst, dst_size);
            return Ok(());
        }
        if filtering != FilterMode::None && dst_height > src_height {
            self.scale_plane_bilinear_up(src, src_size, dst, dst_size, filtering);
            return Ok(());
        }
        if filtering != FilterMode::None {
            self.scale_plane_bilinear_down(src, src_size, dst, dst_size, filtering);
            return Ok(());
        }
        self.scale_plane_simple(src, src_size, dst, dst_size);
        Ok(())
    }

    pub fn i420_scale(&self, src: &I420, dst: &mut I420Mut, filtering: FilterMode) -> Result<()> {
        let max = self.settings.max_dimension as usize;
        if src.width > max || src.height > max {
            return Err(ImageError::InvalidSize {
                width: src.width as i32,
                height: src.height as i32,
            });
        }
        let src_size = Size {
            width: src.width as i32,
            height: src.height as i32,
        };
        let dst_size = Size {
            width: dst.width as i32,
            height: dst.height as i32,
        };
        let src_half = Size {
            width: half(src.width) as i32,
            height: half(src.height) as i32,
        };
        let dst_half = Size {
            width: half(dst.width) as i32,
            height: half(dst.height) as i32,
        };

        self.scale_plane(src.y, src_size, &mut dst.y, dst_size, filtering)?;
        self.scale_plane(src.u, src_half, &mut dst.u, dst_half, filtering)?;
        self.scale_plane(src.v, src_half, &mut dst.v, dst_half, filtering)
    }
}
