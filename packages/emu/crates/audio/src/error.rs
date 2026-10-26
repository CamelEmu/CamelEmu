use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AudioError {
    #[error("the sample rate {sample_rate:#010x} is excessive")]
    ExcessiveSampleRate { sample_rate: i32 },
    #[error("the number of channels {channels:#010x} is illegal")]
    IllegalChannels { channels: usize },
    #[error("the sample rate is not defined")]
    SampleRateNotDefined,
    #[error("the number of channels is not defined")]
    ChannelsNotDefined,
    #[error("the FIR filter length {length:#010x} is not divisible by 8")]
    FirLength { length: usize },
}

pub type Result<T> = std::result::Result<T, AudioError>;
