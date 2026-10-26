use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ShaderError {
    #[error(
        "the fragment shader configuration is {actual:#010x} bytes long, expected {expected:#010x}"
    )]
    ConfigSize { expected: usize, actual: usize },
    #[error("the {kind} {value:#010x} is not supported")]
    UnsupportedValue { kind: &'static str, value: u32 },
}

pub type Result<T> = std::result::Result<T, ShaderError>;
