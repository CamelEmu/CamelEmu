use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ImageError {
    #[error("the size {width:#010x}x{height:#010x} is invalid")]
    InvalidSize { width: i32, height: i32 },
    #[error("the plane of {len:#010x} bytes is shorter than the {required:#010x} bytes it needs")]
    ShortPlane { len: usize, required: usize },
    #[error("the rotation {rotation:#010x} is not supported")]
    InvalidRotation { rotation: i32 },
}

pub type Result<T> = std::result::Result<T, ImageError>;
