use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Screenshot3dError {
    #[error("the capture holds no drawable geometry")]
    EmptyCapture,
    #[error("the texture {index:#010x} of {width:#010x}x{height:#010x} cannot be encoded")]
    InvalidTexture {
        index: usize,
        width: u32,
        height: u32,
    },
}

pub type Result<T> = std::result::Result<T, Screenshot3dError>;
