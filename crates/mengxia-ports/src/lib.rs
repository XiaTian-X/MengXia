//! Provider-neutral ports owned by MengXia application boundaries.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use mengxia_types::{ErrorCode, Sha256Digest};

/// Non-blocking cooperative control supplied by the application layer.
pub trait IngestControl: Send + Sync + 'static {
    fn checkpoint(&self) -> IngestDirective;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngestDirective {
    Continue,
    Stop(IngestStop),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IngestStop {
    Cancelled,
    DeadlineReached,
}

pub enum IngestOutcome {
    Stored(DurableBlob),
    Stopped(IngestStop),
}

/// Provider-neutral synchronous blob custody boundary.
pub trait BlobStorage: Send + Sync {
    type Source: Send + 'static;

    fn open_source(&self, path: &Path) -> Result<Self::Source, BlobSourceError>;

    fn ingest(
        &self,
        source: Self::Source,
        expected_digest: Option<Sha256Digest>,
        control: Arc<dyn IngestControl>,
    ) -> Result<IngestOutcome, BlobStorageError>;
}

pub struct DurableBlob {
    digest: Sha256Digest,
    byte_length: u64,
    location: DurableLocationDescriptor,
}

impl DurableBlob {
    #[must_use]
    pub const fn digest(&self) -> Sha256Digest {
        self.digest
    }

    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    #[must_use]
    pub const fn location(&self) -> &DurableLocationDescriptor {
        &self.location
    }

    /// Trusted construction seam for the verified local adapter only.
    #[doc(hidden)]
    #[must_use]
    pub fn __from_verified_local_adapter(
        digest: Sha256Digest,
        byte_length: u64,
        backend_instance_digest: [u8; 32],
    ) -> Self {
        let digest_hex = lowercase_hex(digest.to_bytes());
        let backend_hex = lowercase_hex(backend_instance_digest);
        Self {
            digest,
            byte_length,
            location: DurableLocationDescriptor {
                backend_id: format!("mengxia.local-cas.v1/{backend_hex}"),
                locator: format!(
                    "sha256-v1/{}/{}/{digest_hex}.blob",
                    &digest_hex[..2],
                    &digest_hex[2..4]
                ),
            },
        }
    }
}

pub struct DurableLocationDescriptor {
    backend_id: String,
    locator: String,
}

impl DurableLocationDescriptor {
    #[must_use]
    pub fn backend_id(&self) -> &str {
        &self.backend_id
    }

    #[must_use]
    pub fn locator(&self) -> &str {
        &self.locator
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobRetryClass {
    AfterInputChange,
    AfterSourceStabilizes,
    AfterStorageConditionChanges,
    NeverAutomatically,
    AfterOperatorConfigurationChange,
    AfterOperatorReconciliation,
    AfterOwnerExit,
    FreshAdmissionWithBoundedDelay,
    AfterPlatformConditionChanges,
    SameRuntimeForbidden,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlobSourceError {
    InvalidPath,
    UnsupportedType,
    Io,
    Modified,
}

impl BlobSourceError {
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::InvalidPath | Self::UnsupportedType => ErrorCode::ValidationError,
            Self::Io => ErrorCode::StorageIoError,
            Self::Modified => ErrorCode::SourceModifiedDuringIngest,
        }
    }

    #[must_use]
    pub const fn retry_class(&self) -> BlobRetryClass {
        match self {
            Self::InvalidPath | Self::UnsupportedType => BlobRetryClass::AfterInputChange,
            Self::Io => BlobRetryClass::AfterStorageConditionChanges,
            Self::Modified => BlobRetryClass::AfterSourceStabilizes,
        }
    }
}

impl fmt::Display for BlobSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPath => "invalid source path",
            Self::UnsupportedType => "unsupported source type",
            Self::Io => "source access failed",
            Self::Modified => "source changed during ingest",
        })
    }
}

impl std::error::Error for BlobSourceError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlobStorageError {
    Validation,
    SourceModified,
    Io,
    Corruption,
    Configuration,
    RecoveryRequired,
    Conflict,
    Backpressure,
    EntropyUnavailable,
    StagingNamespaceUnavailable,
    CleanupFailed,
    ShuttingDown,
    Internal,
}

impl BlobStorageError {
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::Validation => ErrorCode::ValidationError,
            Self::SourceModified => ErrorCode::SourceModifiedDuringIngest,
            Self::Io | Self::CleanupFailed | Self::ShuttingDown => ErrorCode::StorageIoError,
            Self::Corruption => ErrorCode::StorageCorruption,
            Self::Configuration | Self::RecoveryRequired | Self::StagingNamespaceUnavailable => {
                ErrorCode::StorageConfigurationError
            }
            Self::Conflict => ErrorCode::Conflict,
            Self::Backpressure => ErrorCode::Backpressure,
            Self::EntropyUnavailable => ErrorCode::IdGenerationUnavailable,
            Self::Internal => ErrorCode::InternalError,
        }
    }

    #[must_use]
    pub const fn retry_class(&self) -> BlobRetryClass {
        match self {
            Self::Validation => BlobRetryClass::AfterInputChange,
            Self::SourceModified => BlobRetryClass::AfterSourceStabilizes,
            Self::Io => BlobRetryClass::AfterStorageConditionChanges,
            Self::Corruption => BlobRetryClass::NeverAutomatically,
            Self::Configuration => BlobRetryClass::AfterOperatorConfigurationChange,
            Self::RecoveryRequired | Self::StagingNamespaceUnavailable => {
                BlobRetryClass::AfterOperatorReconciliation
            }
            Self::Conflict => BlobRetryClass::AfterOwnerExit,
            Self::Backpressure => BlobRetryClass::FreshAdmissionWithBoundedDelay,
            Self::EntropyUnavailable => BlobRetryClass::AfterPlatformConditionChanges,
            Self::CleanupFailed | Self::ShuttingDown | Self::Internal => {
                BlobRetryClass::SameRuntimeForbidden
            }
        }
    }
}

impl fmt::Display for BlobStorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Validation => "blob input validation failed",
            Self::SourceModified => "source changed during ingest",
            Self::Io => "blob storage operation failed",
            Self::Corruption => "blob storage integrity verification failed",
            Self::Configuration => "blob storage configuration is unsupported or unsafe",
            Self::RecoveryRequired => "blob storage requires orphan reconciliation",
            Self::Conflict => "blob storage is already open",
            Self::Backpressure => "blob storage admission is full",
            Self::EntropyUnavailable => "blob staging identifier generation is unavailable",
            Self::StagingNamespaceUnavailable => "blob staging namespace is unavailable",
            Self::CleanupFailed => "blob staging cleanup did not complete durably",
            Self::ShuttingDown => "blob storage is shutting down",
            Self::Internal => "blob storage internal invariant failed",
        })
    }
}

impl std::error::Error for BlobStorageError {}

fn lowercase_hex(bytes: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(64);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    use mengxia_types::{ErrorCode, Sha256Digest};

    use super::{BlobRetryClass, BlobSourceError, BlobStorageError, DurableBlob};

    #[test]
    fn blob_error_codes_retry_classes_and_static_messages_are_exact() {
        let sources = [
            (
                BlobSourceError::InvalidPath,
                ErrorCode::ValidationError,
                BlobRetryClass::AfterInputChange,
                "invalid source path",
            ),
            (
                BlobSourceError::UnsupportedType,
                ErrorCode::ValidationError,
                BlobRetryClass::AfterInputChange,
                "unsupported source type",
            ),
            (
                BlobSourceError::Io,
                ErrorCode::StorageIoError,
                BlobRetryClass::AfterStorageConditionChanges,
                "source access failed",
            ),
            (
                BlobSourceError::Modified,
                ErrorCode::SourceModifiedDuringIngest,
                BlobRetryClass::AfterSourceStabilizes,
                "source changed during ingest",
            ),
        ];
        for (error, code, retry, display) in sources {
            assert_eq!(error.code(), code);
            assert_eq!(error.retry_class(), retry);
            assert_eq!(error.to_string(), display);
        }
        let storage = [
            (
                BlobStorageError::Validation,
                ErrorCode::ValidationError,
                BlobRetryClass::AfterInputChange,
                "blob input validation failed",
            ),
            (
                BlobStorageError::SourceModified,
                ErrorCode::SourceModifiedDuringIngest,
                BlobRetryClass::AfterSourceStabilizes,
                "source changed during ingest",
            ),
            (
                BlobStorageError::Io,
                ErrorCode::StorageIoError,
                BlobRetryClass::AfterStorageConditionChanges,
                "blob storage operation failed",
            ),
            (
                BlobStorageError::Corruption,
                ErrorCode::StorageCorruption,
                BlobRetryClass::NeverAutomatically,
                "blob storage integrity verification failed",
            ),
            (
                BlobStorageError::Configuration,
                ErrorCode::StorageConfigurationError,
                BlobRetryClass::AfterOperatorConfigurationChange,
                "blob storage configuration is unsupported or unsafe",
            ),
            (
                BlobStorageError::RecoveryRequired,
                ErrorCode::StorageConfigurationError,
                BlobRetryClass::AfterOperatorReconciliation,
                "blob storage requires orphan reconciliation",
            ),
            (
                BlobStorageError::Conflict,
                ErrorCode::Conflict,
                BlobRetryClass::AfterOwnerExit,
                "blob storage is already open",
            ),
            (
                BlobStorageError::Backpressure,
                ErrorCode::Backpressure,
                BlobRetryClass::FreshAdmissionWithBoundedDelay,
                "blob storage admission is full",
            ),
            (
                BlobStorageError::EntropyUnavailable,
                ErrorCode::IdGenerationUnavailable,
                BlobRetryClass::AfterPlatformConditionChanges,
                "blob staging identifier generation is unavailable",
            ),
            (
                BlobStorageError::StagingNamespaceUnavailable,
                ErrorCode::StorageConfigurationError,
                BlobRetryClass::AfterOperatorReconciliation,
                "blob staging namespace is unavailable",
            ),
            (
                BlobStorageError::CleanupFailed,
                ErrorCode::StorageIoError,
                BlobRetryClass::SameRuntimeForbidden,
                "blob staging cleanup did not complete durably",
            ),
            (
                BlobStorageError::ShuttingDown,
                ErrorCode::StorageIoError,
                BlobRetryClass::SameRuntimeForbidden,
                "blob storage is shutting down",
            ),
            (
                BlobStorageError::Internal,
                ErrorCode::InternalError,
                BlobRetryClass::SameRuntimeForbidden,
                "blob storage internal invariant failed",
            ),
        ];
        for (error, code, retry, display) in storage {
            assert_eq!(error.code(), code);
            assert_eq!(error.retry_class(), retry);
            assert_eq!(error.to_string(), display);
        }
    }

    #[test]
    fn verified_local_result_builds_exact_bounded_opaque_location() {
        let digest = Sha256Digest::from_bytes([0xab; 32]);
        let blob = DurableBlob::__from_verified_local_adapter(digest, 7, [0xcd; 32]);
        assert_eq!(blob.digest(), digest);
        assert_eq!(blob.byte_length(), 7);
        assert_eq!(blob.location().backend_id().len(), 85);
        assert_eq!(blob.location().locator().len(), 85);
        assert_eq!(
            blob.location().locator(),
            format!("sha256-v1/ab/ab/{}.blob", "ab".repeat(32))
        );
    }
}
