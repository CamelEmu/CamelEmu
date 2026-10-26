use std::slice;

use crate::{
    Android420, Android420Strides, Argb, Converter, FilterMode, I420, I420Mut, PlaneMut,
    RotationMode, Scaler, Strides, abgr_to_argb, android420_to_i420, argb_mirror, i420_mirror,
    i420_rotate,
};

#[repr(C)]
pub struct EmuImageI420 {
    y: *const u8,
    u: *const u8,
    v: *const u8,
    stride_y: i32,
    stride_u: i32,
    stride_v: i32,
    width: i32,
    height: i32,
}

#[repr(C)]
pub struct EmuImageI420Mut {
    y: *mut u8,
    u: *mut u8,
    v: *mut u8,
    stride_y: i32,
    stride_u: i32,
    stride_v: i32,
    width: i32,
    height: i32,
}

#[repr(C)]
pub struct EmuImageAndroid420 {
    y: *const u8,
    u: *const u8,
    v: *const u8,
    stride_y: i32,
    stride_u: i32,
    stride_v: i32,
    pixel_stride_uv: i32,
    width: i32,
    height: i32,
}

#[repr(C)]
pub struct EmuImageArgb {
    data: *const u8,
    stride: i32,
    width: i32,
    height: i32,
}

fn length(value: i32) -> Option<usize> {
    usize::try_from(value).ok().filter(|value| *value > 0)
}

fn stride(value: i32) -> Option<usize> {
    usize::try_from(value).ok()
}

fn plane_len(stride: usize, row_bytes: usize, rows: usize) -> Option<usize> {
    stride.checked_mul(rows - 1)?.checked_add(row_bytes)
}

unsafe fn read<'a>(ptr: *const u8, stride: usize, row_bytes: usize, rows: usize) -> Option<&'a [u8]> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(ptr, plane_len(stride, row_bytes, rows)?) })
}

unsafe fn write<'a>(
    ptr: *mut u8,
    stride: usize,
    row_bytes: usize,
    rows: usize,
) -> Option<&'a mut [u8]> {
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { slice::from_raw_parts_mut(ptr, plane_len(stride, row_bytes, rows)?) })
}

unsafe fn i420<'a>(image: *const EmuImageI420) -> Option<I420<'a>> {
    let image = unsafe { image.as_ref() }?;
    let (width, height) = (length(image.width)?, length(image.height)?);
    let (stride_y, stride_u, stride_v) = (
        stride(image.stride_y)?,
        stride(image.stride_u)?,
        stride(image.stride_v)?,
    );
    let (half_width, half_height) = (width.div_ceil(2), height.div_ceil(2));
    let y = unsafe { read(image.y, stride_y, width, height) }?;
    let u = unsafe { read(image.u, stride_u, half_width, half_height) }?;
    let v = unsafe { read(image.v, stride_v, half_width, half_height) }?;
    I420::new(
        y,
        u,
        v,
        Strides {
            y: stride_y,
            u: stride_u,
            v: stride_v,
        },
        width,
        height,
    )
    .ok()
}

unsafe fn i420_mut<'a>(image: *const EmuImageI420Mut) -> Option<I420Mut<'a>> {
    let image = unsafe { image.as_ref() }?;
    let (width, height) = (length(image.width)?, length(image.height)?);
    let (stride_y, stride_u, stride_v) = (
        stride(image.stride_y)?,
        stride(image.stride_u)?,
        stride(image.stride_v)?,
    );
    let (half_width, half_height) = (width.div_ceil(2), height.div_ceil(2));
    let y = unsafe { write(image.y, stride_y, width, height) }?;
    let u = unsafe { write(image.u, stride_u, half_width, half_height) }?;
    let v = unsafe { write(image.v, stride_v, half_width, half_height) }?;
    I420Mut::new(
        y,
        u,
        v,
        Strides {
            y: stride_y,
            u: stride_u,
            v: stride_v,
        },
        width,
        height,
    )
    .ok()
}

unsafe fn android420<'a>(image: *const EmuImageAndroid420) -> Option<Android420<'a>> {
    let image = unsafe { image.as_ref() }?;
    let (width, height) = (length(image.width)?, length(image.height)?);
    let pixel_uv = length(image.pixel_stride_uv)?;
    let (stride_y, stride_u, stride_v) = (
        stride(image.stride_y)?,
        stride(image.stride_u)?,
        stride(image.stride_v)?,
    );
    let (half_width, half_height) = (width.div_ceil(2), height.div_ceil(2));
    let chroma_row_bytes = (half_width - 1) * pixel_uv + 1;
    let y = unsafe { read(image.y, stride_y, width, height) }?;
    let u = unsafe { read(image.u, stride_u, chroma_row_bytes, half_height) }?;
    let v = unsafe { read(image.v, stride_v, chroma_row_bytes, half_height) }?;
    Android420::new(
        y,
        u,
        v,
        Android420Strides {
            y: stride_y,
            u: stride_u,
            v: stride_v,
            pixel_uv,
        },
        width,
        height,
    )
    .ok()
}

unsafe fn argb<'a>(image: *const EmuImageArgb) -> Option<Argb<'a>> {
    let image = unsafe { image.as_ref() }?;
    let (width, height) = (length(image.width)?, length(image.height)?);
    let stride = stride(image.stride)?;
    let data = unsafe { read(image.data, stride, width * 4, height) }?;
    Argb::new(data, stride, width, height).ok()
}

unsafe fn packed<'a>(
    dst: *mut u8,
    dst_stride: i32,
    row_bytes: usize,
    rows: usize,
) -> Option<PlaneMut<'a>> {
    let stride = stride(dst_stride)?;
    let data = unsafe { write(dst, stride, row_bytes, rows) }?;
    PlaneMut::new(data, stride, row_bytes, rows).ok()
}

fn status(ok: Option<bool>) -> i32 {
    match ok {
        Some(true) => 0,
        _ => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_android420_to_i420(
    src: *const EmuImageAndroid420,
    dst: *const EmuImageI420Mut,
) -> i32 {
    let result = (|| {
        let src = unsafe { android420(src) }?;
        let mut dst = unsafe { i420_mut(dst) }?;
        Some(android420_to_i420(&src, &mut dst).is_ok())
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_i420_rotate(
    src: *const EmuImageI420,
    dst: *const EmuImageI420Mut,
    rotation: i32,
) -> i32 {
    let result = (|| {
        let mode = RotationMode::try_from(rotation).ok()?;
        let src = unsafe { i420(src) }?;
        let mut dst = unsafe { i420_mut(dst) }?;
        Some(i420_rotate(&src, &mut dst, mode).is_ok())
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_i420_scale(
    src: *const EmuImageI420,
    dst: *const EmuImageI420Mut,
    filter: i32,
) -> i32 {
    let result = (|| {
        let filter = match filter {
            0 => FilterMode::None,
            1 => FilterMode::Linear,
            2 => FilterMode::Bilinear,
            _ => return None,
        };
        let src = unsafe { i420(src) }?;
        let mut dst = unsafe { i420_mut(dst) }?;
        Some(Scaler::default().i420_scale(&src, &mut dst, filter).is_ok())
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_i420_mirror(
    src: *const EmuImageI420,
    dst: *const EmuImageI420Mut,
) -> i32 {
    let result = (|| {
        let src = unsafe { i420(src) }?;
        let mut dst = unsafe { i420_mut(dst) }?;
        Some(i420_mirror(&src, &mut dst).is_ok())
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_i420_to_rgb565(
    src: *const EmuImageI420,
    dst: *mut u8,
    dst_stride: i32,
    invert: i32,
) -> i32 {
    let result = (|| {
        let src = unsafe { i420(src) }?;
        let mut dst = unsafe { packed(dst, dst_stride, src.width() * 2, src.height()) }?;
        Some(
            Converter::default()
                .i420_to_rgb565(&src, &mut dst, invert != 0)
                .is_ok(),
        )
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_i420_to_yuy2(
    src: *const EmuImageI420,
    dst: *mut u8,
    dst_stride: i32,
    invert: i32,
) -> i32 {
    let result = (|| {
        let src = unsafe { i420(src) }?;
        let mut dst =
            unsafe { packed(dst, dst_stride, src.width().div_ceil(2) * 4, src.height()) }?;
        Some(
            Converter::default()
                .i420_to_yuy2(&src, &mut dst, invert != 0)
                .is_ok(),
        )
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_abgr_to_argb(
    src: *const EmuImageArgb,
    dst: *mut u8,
    dst_stride: i32,
) -> i32 {
    let result = (|| {
        let src = unsafe { argb(src) }?;
        let mut dst = unsafe { packed(dst, dst_stride, src.width() * 4, src.height()) }?;
        Some(abgr_to_argb(&src, &mut dst).is_ok())
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_argb_mirror(
    src: *const EmuImageArgb,
    dst: *mut u8,
    dst_stride: i32,
) -> i32 {
    let result = (|| {
        let src = unsafe { argb(src) }?;
        let mut dst = unsafe { packed(dst, dst_stride, src.width() * 4, src.height()) }?;
        Some(argb_mirror(&src, &mut dst).is_ok())
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_argb_to_rgb565(
    src: *const EmuImageArgb,
    dst: *mut u8,
    dst_stride: i32,
    invert: i32,
) -> i32 {
    let result = (|| {
        let src = unsafe { argb(src) }?;
        let mut dst = unsafe { packed(dst, dst_stride, src.width() * 2, src.height()) }?;
        Some(
            Converter::default()
                .argb_to_rgb565(&src, &mut dst, invert != 0)
                .is_ok(),
        )
    })();
    status(result)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn emu_image_argb_to_yuy2(
    src: *const EmuImageArgb,
    dst: *mut u8,
    dst_stride: i32,
    invert: i32,
) -> i32 {
    let result = (|| {
        let src = unsafe { argb(src) }?;
        let mut dst =
            unsafe { packed(dst, dst_stride, src.width().div_ceil(2) * 4, src.height()) }?;
        Some(
            Converter::default()
                .argb_to_yuy2(&src, &mut dst, invert != 0)
                .is_ok(),
        )
    })();
    status(result)
}
