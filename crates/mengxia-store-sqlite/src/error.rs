use std::fmt;

use mengxia_types::ErrorCode;

/// Redacted TASK-004 storage failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StoreError {
    Configuration,
    Busy,
    Io,
    Corruption,
    Conflict,
    Backpressure,
    Internal,
    ShuttingDown,
}

impl StoreError {
    #[must_use]
    pub const fn code(self) -> ErrorCode {
        match self {
            Self::Configuration => ErrorCode::StorageConfigurationError,
            Self::Busy => ErrorCode::StorageBusy,
            Self::Io | Self::ShuttingDown => ErrorCode::StorageIoError,
            Self::Corruption => ErrorCode::StorageCorruption,
            Self::Conflict => ErrorCode::Conflict,
            Self::Backpressure => ErrorCode::Backpressure,
            Self::Internal => ErrorCode::InternalError,
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Configuration => "storage configuration is unsupported or unsafe",
            Self::Busy => "storage is temporarily busy",
            Self::ShuttingDown => "store shutting down",
            Self::Io => "storage operation failed",
            Self::Corruption => "storage integrity verification failed",
            Self::Conflict => "storage is already in use",
            Self::Backpressure => "storage capacity is temporarily exhausted",
            Self::Internal => "internal storage invariant failed",
        })
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::StoreError;
    use mengxia_types::ErrorCode;

    #[test]
    fn accepted_new_codes_and_messages_are_exact_and_static() {
        assert_eq!(
            StoreError::Configuration.code(),
            ErrorCode::StorageConfigurationError
        );
        assert_eq!(
            StoreError::Configuration.to_string(),
            "storage configuration is unsupported or unsafe"
        );
        assert_eq!(StoreError::Busy.code(), ErrorCode::StorageBusy);
        assert_eq!(StoreError::Busy.to_string(), "storage is temporarily busy");
    }
}
