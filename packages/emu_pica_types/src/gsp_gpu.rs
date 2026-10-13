#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameBufferInfo {
    pub active_fb: u32, // 0 = first, 1 = second
    pub address_left: u32,
    pub address_right: u32,
    pub stride: u32,   // maps to 0x1EF00X90 ?
    pub format: u32,   // maps to 0x1EF00X70 ?
    pub shown_fb: u32, // maps to 0x1EF00X78 ?
    pub unknown: u32,
}

const _: () = assert!(
    size_of::<FrameBufferInfo>() == 0x1c,
    "Struct has incorrect size"
);
