use std::fmt;

use mengxia_platform_fs::AuthorityError;
use mengxia_types::ErrorCode;

/// Redacted TASK-004 storage failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StoreError {
    Configuration,
    IdGenerationUnavailable,
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
            Self::IdGenerationUnavailable => ErrorCode::IdGenerationUnavailable,
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
            Self::IdGenerationUnavailable => "identifier generation is unavailable",
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

pub(crate) fn map_authority_error(error: AuthorityError) -> StoreError {
    match error {
        AuthorityError::UnsafeConfiguration => StoreError::Configuration,
        AuthorityError::Io => StoreError::Io,
        AuthorityError::Contended => StoreError::Conflict,
        AuthorityError::ConflictingData => StoreError::Corruption,
        _ => StoreError::Internal,
    }
}

pub(crate) fn map_sqlite_error(error: rusqlite::Error) -> StoreError {
    use rusqlite::ErrorCode;

    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy) => StoreError::Busy,
        Some(ErrorCode::DatabaseLocked) => StoreError::Internal,
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => StoreError::Corruption,
        Some(
            ErrorCode::CannotOpen
            | ErrorCode::SystemIoFailure
            | ErrorCode::DiskFull
            | ErrorCode::ReadOnly
            | ErrorCode::PermissionDenied
            | ErrorCode::OutOfMemory,
        ) => StoreError::Io,
        Some(_) | None => StoreError::Internal,
    }
}

pub(crate) fn map_reopen_error(error: rusqlite::Error) -> StoreError {
    if error.sqlite_error_code().is_some() {
        map_sqlite_error(error)
    } else {
        // A value/type/row-shape conversion failure after the exact schema has
        // been established is malformed persistent state, not an API defect.
        StoreError::Corruption
    }
}

#[cfg(test)]
mod tests {
    use mengxia_platform_fs::AuthorityError;
    use mengxia_types::ErrorCode;
    use rusqlite::ffi;

    use super::{StoreError, map_authority_error, map_reopen_error, map_sqlite_error};

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
        assert_eq!(
            StoreError::IdGenerationUnavailable.code(),
            ErrorCode::IdGenerationUnavailable
        );
        assert_eq!(
            StoreError::IdGenerationUnavailable.to_string(),
            "identifier generation is unavailable"
        );
        assert_eq!(StoreError::Busy.code(), ErrorCode::StorageBusy);
        assert_eq!(StoreError::Busy.to_string(), "storage is temporarily busy");
        assert_eq!(StoreError::Backpressure.code(), ErrorCode::Backpressure);
        assert_eq!(StoreError::ShuttingDown.code(), ErrorCode::StorageIoError);
        assert_eq!(StoreError::ShuttingDown.to_string(), "store shutting down");
    }

    #[test]
    fn platform_error_mapping_preserves_io_and_configuration_classes() {
        assert_eq!(
            map_authority_error(AuthorityError::UnsafeConfiguration),
            StoreError::Configuration
        );
        assert_eq!(map_authority_error(AuthorityError::Io), StoreError::Io);
        assert_eq!(
            map_authority_error(AuthorityError::Contended),
            StoreError::Conflict
        );
        assert_eq!(
            map_authority_error(AuthorityError::ConflictingData),
            StoreError::Corruption
        );
    }

    #[test]
    fn sqlite_primary_mapping_matches_the_accepted_error_matrix() {
        for (code, expected) in [
            (ffi::SQLITE_BUSY, StoreError::Busy),
            (ffi::SQLITE_BUSY_RECOVERY, StoreError::Busy),
            (ffi::SQLITE_BUSY_SNAPSHOT, StoreError::Busy),
            (ffi::SQLITE_BUSY_TIMEOUT, StoreError::Busy),
            (ffi::SQLITE_LOCKED, StoreError::Internal),
            (ffi::SQLITE_LOCKED_SHAREDCACHE, StoreError::Internal),
            (ffi::SQLITE_CORRUPT, StoreError::Corruption),
            (ffi::SQLITE_CORRUPT_VTAB, StoreError::Corruption),
            (ffi::SQLITE_NOTADB, StoreError::Corruption),
            (ffi::SQLITE_CANTOPEN, StoreError::Io),
            (ffi::SQLITE_CANTOPEN_ISDIR, StoreError::Io),
            (ffi::SQLITE_IOERR, StoreError::Io),
            (ffi::SQLITE_IOERR_READ, StoreError::Io),
            (ffi::SQLITE_IOERR_WRITE, StoreError::Io),
            (ffi::SQLITE_FULL, StoreError::Io),
            (ffi::SQLITE_READONLY, StoreError::Io),
            (ffi::SQLITE_READONLY_DBMOVED, StoreError::Io),
            (ffi::SQLITE_PERM, StoreError::Io),
            (ffi::SQLITE_NOMEM, StoreError::Io),
            (ffi::SQLITE_CONSTRAINT, StoreError::Internal),
            (ffi::SQLITE_CONSTRAINT_UNIQUE, StoreError::Internal),
            (ffi::SQLITE_INTERNAL, StoreError::Internal),
            (ffi::SQLITE_ABORT, StoreError::Internal),
            (ffi::SQLITE_INTERRUPT, StoreError::Internal),
            (ffi::SQLITE_SCHEMA, StoreError::Internal),
            (ffi::SQLITE_MISUSE, StoreError::Internal),
        ] {
            let error = rusqlite::Error::SqliteFailure(ffi::Error::new(code), None);
            assert_eq!(map_sqlite_error(error), expected, "SQLite code {code}");
        }
    }

    #[test]
    fn reopen_value_conversion_is_corruption_but_sqlite_io_is_not() {
        let conversion = rusqlite::Error::InvalidColumnType(
            0,
            "owner_uid".to_owned(),
            rusqlite::types::Type::Text,
        );
        assert_eq!(map_reopen_error(conversion), StoreError::Corruption);

        let io = rusqlite::Error::SqliteFailure(ffi::Error::new(ffi::SQLITE_IOERR), None);
        assert_eq!(map_reopen_error(io), StoreError::Io);
    }
}
