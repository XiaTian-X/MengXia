use std::error::Error;

use mengxia_domain::DomainError;
use mengxia_types::{ErrorCode, IdGenerationError, RevisionOverflow, ValueError};

#[test]
fn domain_errors_map_to_stable_codes_and_safe_sources() {
    let cases = [
        (
            DomainError::InvalidValue(ValueError::InvalidId),
            ErrorCode::ValidationError,
            "invalid typed UUIDv7",
        ),
        (
            DomainError::IdGeneration(IdGenerationError::EntropyUnavailable),
            ErrorCode::IdGenerationUnavailable,
            "operating-system entropy is unavailable",
        ),
        (
            DomainError::RevisionOverflow(RevisionOverflow),
            ErrorCode::RevisionExhausted,
            "revision number is exhausted",
        ),
    ];

    for (error, code, safe_message) in cases {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), safe_message);
        assert_eq!(
            error.source().expect("typed inner error").to_string(),
            safe_message
        );
    }
}
