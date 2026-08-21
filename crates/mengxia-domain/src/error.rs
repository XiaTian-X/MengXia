use std::fmt;

use mengxia_types::{ErrorCode, IdGenerationError, RevisionOverflow, ValueError};

/// The minimal typed domain-error baseline for foundation values.
#[derive(Debug)]
#[non_exhaustive]
pub enum DomainError {
    InvalidValue(ValueError),
    IdGeneration(IdGenerationError),
    RevisionOverflow(RevisionOverflow),
}

impl DomainError {
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidValue(_) => ErrorCode::ValidationError,
            Self::IdGeneration(_) => ErrorCode::IdGenerationUnavailable,
            Self::RevisionOverflow(_) => ErrorCode::RevisionExhausted,
        }
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue(error) => error.fmt(formatter),
            Self::IdGeneration(error) => error.fmt(formatter),
            Self::RevisionOverflow(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DomainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidValue(error) => Some(error),
            Self::IdGeneration(error) => Some(error),
            Self::RevisionOverflow(error) => Some(error),
        }
    }
}
