pub use crate::api::error::AzaharError;

impl From<memory::MemoryError> for AzaharError {
    fn from(error: memory::MemoryError) -> Self {
        match error {
            memory::MemoryError::Unavailable => Self::InvalidState,
            memory::MemoryError::InvalidAddress { address, len } => {
                Self::InvalidAddress { address, len }
            }
            memory::MemoryError::Core(status) => Self::Core(status),
        }
    }
}

impl From<emu_storage::StorageError> for AzaharError {
    fn from(error: emu_storage::StorageError) -> Self {
        match error {
            emu_storage::StorageError::InvalidPath => Self::InvalidPath,
            emu_storage::StorageError::Create(path) => Self::Storage(path),
            emu_storage::StorageError::Commit(paths) => Self::StorageCommit(paths),
        }
    }
}

/// Result alias used across the crate. It is not named `Result` because the generated bindings glob
/// import this module and the alias would shadow the standard `Result` there.
pub type SessionResult<T> = std::result::Result<T, AzaharError>;
