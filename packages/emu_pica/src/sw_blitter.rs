use std::rc::Rc;

use emu_pica_types::color::{
    decode_rgb5a1, decode_rgb8, decode_rgb565, decode_rgba4, decode_rgba8, encode_rgb5a1,
    encode_rgb8, encode_rgb565, encode_rgba4, encode_rgba8,
};
use emu_pica_types::utils::get_morton_offset;
use emu_pica_types::{DisplayTransferConfig, MemoryFillConfig, PixelFormat, ScalingMode, Vec4};
use emu_renderer::Rasterizer;

use crate::memory::PhysicalMemory;

fn decode_pixel(input_format: PixelFormat, src_pixel: *const u8) -> Vec4<u8> {
    let bytes = |length| unsafe { std::slice::from_raw_parts(src_pixel, length) };
    match input_format {
        PixelFormat::RGBA8 => decode_rgba8(bytes(4)),
        PixelFormat::RGB8 => decode_rgb8(bytes(3)),
        PixelFormat::RGB565 => decode_rgb565(bytes(2)),
        PixelFormat::RGB5A1 => decode_rgb5a1(bytes(2)),
        PixelFormat::RGBA4 => decode_rgba4(bytes(2)),
        _ => {
            log::error!(
                target: "HW_GPU",
                "Unknown source framebuffer format {:x}",
                input_format.0
            );
            Vec4::default()
        }
    }
}

fn to_i32(color: Vec4<u8>) -> Vec4<i32> {
    Vec4 {
        x: i32::from(color.x),
        y: i32::from(color.y),
        z: i32::from(color.z),
        w: i32::from(color.w),
    }
}

fn add(a: Vec4<i32>, b: Vec4<i32>) -> Vec4<i32> {
    Vec4 {
        x: a.x + b.x,
        y: a.y + b.y,
        z: a.z + b.z,
        w: a.w + b.w,
    }
}

fn divide_to_u8(color: Vec4<i32>, divisor: i32) -> Vec4<u8> {
    Vec4 {
        x: (color.x / divisor) as u8,
        y: (color.y / divisor) as u8,
        z: (color.z / divisor) as u8,
        w: (color.w / divisor) as u8,
    }
}

pub struct SwBlitter {
    memory: Rc<dyn PhysicalMemory>,
}

impl SwBlitter {
    pub fn new(memory: Rc<dyn PhysicalMemory>) -> Self {
        Self { memory }
    }

    pub fn texture_copy(&self, rasterizer: &mut dyn Rasterizer, config: &DisplayTransferConfig) {
        let src_addr = config.physical_input_address();
        let dst_addr = config.physical_output_address();

        // TODO: do hwtest with invalid addresses
        if !self.memory.is_valid_physical_address(src_addr) {
            log::error!(target: "HW_GPU", "invalid input address {:#010X}", src_addr);
            return;
        }

        if !self.memory.is_valid_physical_address(dst_addr) {
            log::error!(target: "HW_GPU", "invalid output address {:#010X}", dst_addr);
            return;
        }

        let mut src_pointer = self.memory.get_physical_pointer(src_addr);
        let mut dst_pointer = self.memory.get_physical_pointer(dst_addr);

        let mut remaining_size = config.texture_copy.size & !15;
        if remaining_size == 0 {
            log::error!(target: "HW_GPU", "zero size. Real hardware freezes on this.");
            return;
        }

        let input_gap = config.texture_copy.input_gap().wrapping_mul(16);
        let output_gap = config.texture_copy.output_gap().wrapping_mul(16);

        // Zero gap means contiguous input/output even if width = 0. To avoid infinite loop below, width
        // is assigned with the total size if gap = 0.
        let input_width = if input_gap == 0 {
            remaining_size
        } else {
            config.texture_copy.input_width().wrapping_mul(16)
        };
        let output_width = if output_gap == 0 {
            remaining_size
        } else {
            config.texture_copy.output_width().wrapping_mul(16)
        };

        if input_width == 0 {
            log::error!(target: "HW_GPU", "zero input width. Real hardware freezes on this.");
            return;
        }

        if output_width == 0 {
            log::error!(target: "HW_GPU", "zero output width. Real hardware freezes on this.");
            return;
        }

        let contiguous_input_size = config.texture_copy.size as usize / input_width as usize
            * (input_width + input_gap) as usize;
        rasterizer.flush_region(
            config.physical_input_address(),
            contiguous_input_size as u32,
        );

        let contiguous_output_size = config.texture_copy.size as usize / output_width as usize
            * (output_width + output_gap) as usize;

        // Only need to flush output if it has a gap
        if output_gap != 0 {
            rasterizer.flush_and_invalidate_region(dst_addr, contiguous_output_size as u32);
        } else {
            rasterizer.invalidate_region(dst_addr, contiguous_output_size as u32);
        }

        let mut remaining_input = input_width;
        let mut remaining_output = output_width;
        while remaining_size > 0 {
            let copy_size = remaining_input.min(remaining_output).min(remaining_size);

            unsafe {
                std::ptr::copy(src_pointer, dst_pointer, copy_size as usize);
                src_pointer = src_pointer.add(copy_size as usize);
                dst_pointer = dst_pointer.add(copy_size as usize);
            }

            remaining_input -= copy_size;
            remaining_output -= copy_size;
            remaining_size -= copy_size;

            if remaining_input == 0 {
                remaining_input = input_width;
                src_pointer = src_pointer.wrapping_add(input_gap as usize);
            }
            if remaining_output == 0 {
                remaining_output = output_width;
                dst_pointer = dst_pointer.wrapping_add(output_gap as usize);
            }
        }
    }

    pub fn display_transfer(
        &self,
        rasterizer: &mut dyn Rasterizer,
        config: &DisplayTransferConfig,
    ) {
        let src_addr = config.physical_input_address();
        let mut dst_addr = config.physical_output_address();

        // TODO: do hwtest with these cases
        if !self.memory.is_valid_physical_address(src_addr) {
            log::error!(target: "HW_GPU", "invalid input address {:#010X}", src_addr);
            return;
        }

        if !self.memory.is_valid_physical_address(dst_addr) {
            log::error!(target: "HW_GPU", "invalid output address {:#010X}", dst_addr);
            return;
        }

        if config.input_width() == 0 {
            log::error!(target: "HW_GPU", "zero input width");
            return;
        }

        if config.input_height() == 0 {
            log::error!(target: "HW_GPU", "zero input height");
            return;
        }

        if config.output_width() == 0 {
            log::error!(target: "HW_GPU", "zero output width");
            return;
        }

        if config.output_height() == 0 {
            log::error!(target: "HW_GPU", "zero output height");
            return;
        }

        // Using flip_vertically alongside crop_input_lines produces skewed output on hardware.
        // We have to emulate this because some games rely on this behaviour to render correctly.
        if config.flip_vertically() && config.crop_input_lines() {
            dst_addr = dst_addr.wrapping_add(
                config
                    .input_width()
                    .wrapping_sub(config.output_width())
                    .wrapping_mul(config.output_height().wrapping_sub(1))
                    .wrapping_mul(config.output_format().bytes_per_pixel()),
            );
        }

        let src_pointer = self.memory.get_physical_pointer(src_addr);
        let dst_pointer = self.memory.get_physical_pointer(dst_addr);

        if config.scaling().0 > ScalingMode::SCALE_XY.0 {
            log::error!(
                target: "HW_GPU",
                "Unimplemented display transfer scaling mode {}",
                config.scaling().0
            );
            log::error!(target: "Debug", "Unimplemented code!");
            return;
        }

        if config.input_linear() && config.scaling() != ScalingMode::NO_SCALE {
            log::error!(target: "HW_GPU", "Scaling is only implemented on tiled input");
            log::error!(target: "Debug", "Unimplemented code!");
            return;
        }

        let horizontal_scale = if config.scaling() != ScalingMode::NO_SCALE {
            1
        } else {
            0
        };
        let vertical_scale = if config.scaling() == ScalingMode::SCALE_XY {
            1
        } else {
            0
        };

        let output_width = config.output_width() >> horizontal_scale;
        let output_height = config.output_height() >> vertical_scale;

        let input_size = config
            .input_width()
            .wrapping_mul(config.input_height())
            .wrapping_mul(config.input_format().bytes_per_pixel());
        let output_size = output_width
            .wrapping_mul(output_height)
            .wrapping_mul(config.output_format().bytes_per_pixel());

        rasterizer.flush_region(config.physical_input_address(), input_size);
        rasterizer.invalidate_region(config.physical_output_address(), output_size);

        for y in 0..output_height {
            for x in 0..output_width {
                // Calculate the [x,y] position of the input image
                // based on the current output position and the scale
                let input_x = x << horizontal_scale;
                let input_y = y << vertical_scale;

                let output_y = if config.flip_vertically() {
                    // Flip the y value of the output data,
                    // we do this after calculating the [x,y] position of the input image
                    // to account for the scaling options.
                    output_height - y - 1
                } else {
                    y
                };

                let dst_bytes_per_pixel = config.output_format().bytes_per_pixel();
                let src_bytes_per_pixel = config.input_format().bytes_per_pixel();
                let src_offset: u32;
                let dst_offset: u32;

                if config.input_linear() {
                    if !config.dont_swizzle() {
                        // Interpret the input as linear and the output as tiled
                        let coarse_y = output_y & !7;
                        let stride = output_width * dst_bytes_per_pixel;

                        src_offset =
                            (input_x + input_y * config.input_width()) * src_bytes_per_pixel;
                        dst_offset =
                            get_morton_offset(x, output_y, dst_bytes_per_pixel) + coarse_y * stride;
                    } else {
                        // Both input and output are linear
                        src_offset =
                            (input_x + input_y * config.input_width()) * src_bytes_per_pixel;
                        dst_offset = (x + output_y * output_width) * dst_bytes_per_pixel;
                    }
                } else if !config.dont_swizzle() {
                    // Interpret the input as tiled and the output as linear
                    let coarse_y = input_y & !7;
                    let stride = config.input_width() * src_bytes_per_pixel;

                    src_offset = get_morton_offset(input_x, input_y, src_bytes_per_pixel)
                        + coarse_y * stride;
                    dst_offset = (x + output_y * output_width) * dst_bytes_per_pixel;
                } else {
                    // Both input and output are tiled
                    let out_coarse_y = output_y & !7;
                    let out_stride = output_width * dst_bytes_per_pixel;

                    let in_coarse_y = input_y & !7;
                    let in_stride = config.input_width() * src_bytes_per_pixel;

                    src_offset = get_morton_offset(input_x, input_y, src_bytes_per_pixel)
                        + in_coarse_y * in_stride;
                    dst_offset = get_morton_offset(x, output_y, dst_bytes_per_pixel)
                        + out_coarse_y * out_stride;
                }

                let src_pixel = src_pointer.wrapping_add(src_offset as usize);
                let mut src_color = decode_pixel(config.input_format(), src_pixel);
                if config.scaling() == ScalingMode::SCALE_X {
                    let pixel = decode_pixel(
                        config.input_format(),
                        src_pixel.wrapping_add(src_bytes_per_pixel as usize),
                    );
                    src_color = divide_to_u8(add(to_i32(src_color), to_i32(pixel)), 2);
                } else if config.scaling() == ScalingMode::SCALE_XY {
                    let pixel1 = decode_pixel(
                        config.input_format(),
                        src_pixel.wrapping_add(src_bytes_per_pixel as usize),
                    );
                    let pixel2 = decode_pixel(
                        config.input_format(),
                        src_pixel.wrapping_add(2 * src_bytes_per_pixel as usize),
                    );
                    let pixel3 = decode_pixel(
                        config.input_format(),
                        src_pixel.wrapping_add(3 * src_bytes_per_pixel as usize),
                    );
                    src_color = divide_to_u8(
                        add(
                            add(to_i32(src_color), to_i32(pixel1)),
                            add(to_i32(pixel2), to_i32(pixel3)),
                        ),
                        4,
                    );
                }

                let dst_pixel = unsafe {
                    std::slice::from_raw_parts_mut(
                        dst_pointer.wrapping_add(dst_offset as usize),
                        dst_bytes_per_pixel as usize,
                    )
                };
                match config.output_format() {
                    PixelFormat::RGBA8 => encode_rgba8(&src_color, dst_pixel),
                    PixelFormat::RGB8 => encode_rgb8(&src_color, dst_pixel),
                    PixelFormat::RGB565 => encode_rgb565(&src_color, dst_pixel),
                    PixelFormat::RGB5A1 => encode_rgb5a1(&src_color, dst_pixel),
                    PixelFormat::RGBA4 => encode_rgba4(&src_color, dst_pixel),
                    _ => {
                        log::error!(
                            target: "HW_GPU",
                            "Unknown destination framebuffer format {:x}",
                            config.output_format().0
                        );
                    }
                }
            }
        }
    }

    pub fn memory_fill(&self, rasterizer: &mut dyn Rasterizer, config: &MemoryFillConfig) {
        let start_addr = config.start_address();
        let end_addr = config.end_address();

        // TODO: do hwtest with these cases
        if !self.memory.is_valid_physical_address(start_addr) {
            log::error!(target: "HW_GPU", "invalid start address {:#010X}", start_addr);
            return;
        }

        if !self.memory.is_valid_physical_address(end_addr) {
            log::error!(target: "HW_GPU", "invalid end address {:#010X}", end_addr);
            return;
        }

        if end_addr <= start_addr {
            log::error!(
                target: "HW_GPU",
                "invalid memory range from {:#010X} to {:#010X}",
                start_addr,
                end_addr
            );
            return;
        }

        let start = self.memory.get_physical_pointer(start_addr);
        let end = self.memory.get_physical_pointer(end_addr);

        rasterizer.invalidate_region(start_addr, end_addr - start_addr);

        if config.fill_24bit() {
            // Fill with 24-bit values
            let mut ptr = start;
            while ptr < end {
                unsafe {
                    *ptr = config.value_24bit_r() as u8;
                    *ptr.add(1) = config.value_24bit_g() as u8;
                    *ptr.add(2) = config.value_24bit_b() as u8;
                }
                ptr = ptr.wrapping_add(3);
            }
        } else if config.fill_32bit() {
            // Fill with 32-bit values
            if end > start {
                let value = config.value_32bit;
                let len = (end as usize - start as usize) / size_of::<u32>();
                for i in 0..len {
                    unsafe {
                        start
                            .add(i * size_of::<u32>())
                            .cast::<u32>()
                            .write_unaligned(value)
                    };
                }
            }
        } else {
            // Fill with 16-bit values
            let value_16bit = config.value_16bit() as u16;
            let mut ptr = start;
            while ptr < end {
                unsafe { ptr.cast::<u16>().write_unaligned(value_16bit) };
                ptr = ptr.wrapping_add(size_of::<u16>());
            }
        }
    }
}
