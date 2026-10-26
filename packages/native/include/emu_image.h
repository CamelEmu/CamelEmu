#ifndef EMU_IMAGE_H
#define EMU_IMAGE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct EmuImageI420 {
    const uint8_t* y;
    const uint8_t* u;
    const uint8_t* v;
    int32_t stride_y;
    int32_t stride_u;
    int32_t stride_v;
    int32_t width;
    int32_t height;
} EmuImageI420;

typedef struct EmuImageI420Mut {
    uint8_t* y;
    uint8_t* u;
    uint8_t* v;
    int32_t stride_y;
    int32_t stride_u;
    int32_t stride_v;
    int32_t width;
    int32_t height;
} EmuImageI420Mut;

typedef struct EmuImageAndroid420 {
    const uint8_t* y;
    const uint8_t* u;
    const uint8_t* v;
    int32_t stride_y;
    int32_t stride_u;
    int32_t stride_v;
    int32_t pixel_stride_uv;
    int32_t width;
    int32_t height;
} EmuImageAndroid420;

typedef struct EmuImageArgb {
    const uint8_t* data;
    int32_t stride;
    int32_t width;
    int32_t height;
} EmuImageArgb;

enum {
    EMU_IMAGE_FILTER_NONE = 0,
    EMU_IMAGE_FILTER_LINEAR = 1,
    EMU_IMAGE_FILTER_BILINEAR = 2,
};

int32_t emu_image_android420_to_i420(const EmuImageAndroid420* src, const EmuImageI420Mut* dst);

int32_t emu_image_i420_rotate(const EmuImageI420* src, const EmuImageI420Mut* dst, int32_t rotation);

int32_t emu_image_i420_scale(const EmuImageI420* src, const EmuImageI420Mut* dst, int32_t filter);

int32_t emu_image_i420_mirror(const EmuImageI420* src, const EmuImageI420Mut* dst);

int32_t emu_image_i420_to_rgb565(const EmuImageI420* src, uint8_t* dst, int32_t dst_stride,
                                 int32_t invert);

int32_t emu_image_i420_to_yuy2(const EmuImageI420* src, uint8_t* dst, int32_t dst_stride,
                               int32_t invert);

int32_t emu_image_abgr_to_argb(const EmuImageArgb* src, uint8_t* dst, int32_t dst_stride);

int32_t emu_image_argb_mirror(const EmuImageArgb* src, uint8_t* dst, int32_t dst_stride);

int32_t emu_image_argb_to_rgb565(const EmuImageArgb* src, uint8_t* dst, int32_t dst_stride,
                                 int32_t invert);

int32_t emu_image_argb_to_yuy2(const EmuImageArgb* src, uint8_t* dst, int32_t dst_stride,
                               int32_t invert);

#ifdef __cplusplus
}
#endif

#endif
