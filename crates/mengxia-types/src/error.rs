use std::fmt;
use std::str::FromStr;

const MIN_ERROR_CODE_BYTES: usize = "CONFLICT".len();
const MAX_ERROR_CODE_BYTES: usize = "SOURCE_MODIFIED_DURING_INGEST".len();

/// Stable, transport-independent error classification.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ErrorCode {
    ValidationError,
    AuthenticationError,
    AuthorizationDenied,
    NotFound,
    Conflict,
    InvalidTransition,
    SourceModifiedDuringIngest,
    StorageIoError,
    StorageCorruption,
    StorageBusy,
    StorageConfigurationError,
    IpcTransportError,
    ProtocolVersionUnsupported,
    DeadlineExceeded,
    OperationCancelled,
    ProviderValidation,
    InvalidCredential,
    ProviderRateLimited,
    ProviderTimeout,
    ProviderUnavailable,
    SubmissionUnknown,
    PluginProtocolViolation,
    SandboxUnavailable,
    PluginRevoked,
    Backpressure,
    InternalError,
    CommandInProgress,
    AdminAuthUnavailable,
    UnsupportedCapability,
    IdGenerationUnavailable,
    RevisionExhausted,
}

impl ErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ValidationError => "VALIDATION_ERROR",
            Self::AuthenticationError => "AUTHENTICATION_ERROR",
            Self::AuthorizationDenied => "AUTHORIZATION_DENIED",
            Self::NotFound => "NOT_FOUND",
            Self::Conflict => "CONFLICT",
            Self::InvalidTransition => "INVALID_TRANSITION",
            Self::SourceModifiedDuringIngest => "SOURCE_MODIFIED_DURING_INGEST",
            Self::StorageIoError => "STORAGE_IO_ERROR",
            Self::StorageCorruption => "STORAGE_CORRUPTION",
            Self::StorageBusy => "STORAGE_BUSY",
            Self::StorageConfigurationError => "STORAGE_CONFIGURATION_ERROR",
            Self::IpcTransportError => "IPC_TRANSPORT_ERROR",
            Self::ProtocolVersionUnsupported => "PROTOCOL_VERSION_UNSUPPORTED",
            Self::DeadlineExceeded => "DEADLINE_EXCEEDED",
            Self::OperationCancelled => "OPERATION_CANCELLED",
            Self::ProviderValidation => "PROVIDER_VALIDATION",
            Self::InvalidCredential => "INVALID_CREDENTIAL",
            Self::ProviderRateLimited => "PROVIDER_RATE_LIMITED",
            Self::ProviderTimeout => "PROVIDER_TIMEOUT",
            Self::ProviderUnavailable => "PROVIDER_UNAVAILABLE",
            Self::SubmissionUnknown => "SUBMISSION_UNKNOWN",
            Self::PluginProtocolViolation => "PLUGIN_PROTOCOL_VIOLATION",
            Self::SandboxUnavailable => "SANDBOX_UNAVAILABLE",
            Self::PluginRevoked => "PLUGIN_REVOKED",
            Self::Backpressure => "BACKPRESSURE",
            Self::InternalError => "INTERNAL_ERROR",
            Self::CommandInProgress => "COMMAND_IN_PROGRESS",
            Self::AdminAuthUnavailable => "ADMIN_AUTH_UNAVAILABLE",
            Self::UnsupportedCapability => "UNSUPPORTED_CAPABILITY",
            Self::IdGenerationUnavailable => "ID_GENERATION_UNAVAILABLE",
            Self::RevisionExhausted => "REVISION_EXHAUSTED",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ErrorCode {
    type Err = ValueError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !(MIN_ERROR_CODE_BYTES..=MAX_ERROR_CODE_BYTES).contains(&value.len())
            || !value.is_ascii()
        {
            return Err(ValueError::UnknownErrorCode);
        }
        match value {
            "VALIDATION_ERROR" => Ok(Self::ValidationError),
            "AUTHENTICATION_ERROR" => Ok(Self::AuthenticationError),
            "AUTHORIZATION_DENIED" => Ok(Self::AuthorizationDenied),
            "NOT_FOUND" => Ok(Self::NotFound),
            "CONFLICT" => Ok(Self::Conflict),
            "INVALID_TRANSITION" => Ok(Self::InvalidTransition),
            "SOURCE_MODIFIED_DURING_INGEST" => Ok(Self::SourceModifiedDuringIngest),
            "STORAGE_IO_ERROR" => Ok(Self::StorageIoError),
            "STORAGE_CORRUPTION" => Ok(Self::StorageCorruption),
            "STORAGE_BUSY" => Ok(Self::StorageBusy),
            "STORAGE_CONFIGURATION_ERROR" => Ok(Self::StorageConfigurationError),
            "IPC_TRANSPORT_ERROR" => Ok(Self::IpcTransportError),
            "PROTOCOL_VERSION_UNSUPPORTED" => Ok(Self::ProtocolVersionUnsupported),
            "DEADLINE_EXCEEDED" => Ok(Self::DeadlineExceeded),
            "OPERATION_CANCELLED" => Ok(Self::OperationCancelled),
            "PROVIDER_VALIDATION" => Ok(Self::ProviderValidation),
            "INVALID_CREDENTIAL" => Ok(Self::InvalidCredential),
            "PROVIDER_RATE_LIMITED" => Ok(Self::ProviderRateLimited),
            "PROVIDER_TIMEOUT" => Ok(Self::ProviderTimeout),
            "PROVIDER_UNAVAILABLE" => Ok(Self::ProviderUnavailable),
            "SUBMISSION_UNKNOWN" => Ok(Self::SubmissionUnknown),
            "PLUGIN_PROTOCOL_VIOLATION" => Ok(Self::PluginProtocolViolation),
            "SANDBOX_UNAVAILABLE" => Ok(Self::SandboxUnavailable),
            "PLUGIN_REVOKED" => Ok(Self::PluginRevoked),
            "BACKPRESSURE" => Ok(Self::Backpressure),
            "INTERNAL_ERROR" => Ok(Self::InternalError),
            "COMMAND_IN_PROGRESS" => Ok(Self::CommandInProgress),
            "ADMIN_AUTH_UNAVAILABLE" => Ok(Self::AdminAuthUnavailable),
            "UNSUPPORTED_CAPABILITY" => Ok(Self::UnsupportedCapability),
            "ID_GENERATION_UNAVAILABLE" => Ok(Self::IdGenerationUnavailable),
            "REVISION_EXHAUSTED" => Ok(Self::RevisionExhausted),
            _ => Err(ValueError::UnknownErrorCode),
        }
    }
}

/// Safe classification for invalid foundation values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ValueError {
    InvalidId,
    InvalidDigest,
    InvalidTimestamp,
    InvalidRevision,
    UnknownErrorCode,
}

impl fmt::Display for ValueError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidId => "invalid typed UUIDv7",
            Self::InvalidDigest => "invalid SHA-256 digest",
            Self::InvalidTimestamp => "invalid timestamp",
            Self::InvalidRevision => "invalid revision number",
            Self::UnknownErrorCode => "unknown error code",
        })
    }
}

impl std::error::Error for ValueError {}

/// Safe reason that UUIDv7 generation could not obtain valid OS inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum IdGenerationError {
    ClockBeforeUnixEpoch,
    TimestampOutOfRange,
    EntropyUnavailable,
}

impl fmt::Display for IdGenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ClockBeforeUnixEpoch => "system clock is before the Unix epoch",
            Self::TimestampOutOfRange => "system clock is outside the UUIDv7 range",
            Self::EntropyUnavailable => "operating-system entropy is unavailable",
        })
    }
}

impl std::error::Error for IdGenerationError {}

/// A revision cannot advance beyond `u64::MAX`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevisionOverflow;

impl fmt::Display for RevisionOverflow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("revision number is exhausted")
    }
}

impl std::error::Error for RevisionOverflow {}
