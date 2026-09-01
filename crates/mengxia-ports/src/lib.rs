//! Provider-neutral ports owned by MengXia application boundaries.

#![forbid(unsafe_code)]

use std::fmt;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use mengxia_domain::{
    Asset, AssetKind, AssetRevision, ContentKind, Location, LogicalName, MediaType,
    NewAssetRevision, Representation, RepresentationPurpose, Resource, ResourceKind,
};
use mengxia_events::{DomainEvent, ProvenanceEvent};
use mengxia_types::{ErrorCode, Id, RevisionNo, Sha256Digest, Timestamp};

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

/// Type-level identity marker for persisted commands.
///
/// ```compile_fail
/// use mengxia_domain::Asset;
/// use mengxia_ports::Command;
/// use mengxia_types::Id;
/// let command = Id::<Command>::try_new().unwrap();
/// let _: Id<Asset> = command;
/// ```
pub enum Command {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationId(&'static str);

impl OperationId {
    const fn asset_ingest_v1() -> Self {
        Self("asset.ingest.v1")
    }
    const fn asset_revision_create_v1() -> Self {
        Self("asset.revision.create.v1")
    }
    const fn blob_location_record_v1() -> Self {
        Self("blob.location.record.v1")
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

pub const ASSET_INGEST_COPY_V1: OperationId = OperationId::asset_ingest_v1();
pub const ASSET_REVISION_CREATE_V1: OperationId = OperationId::asset_revision_create_v1();
pub const BLOB_LOCATION_RECORD_V1: OperationId = OperationId::blob_location_record_v1();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandBinding {
    command_id: Id<Command>,
    operation_id: OperationId,
    canonical_request_digest: Sha256Digest,
}

impl CommandBinding {
    #[must_use]
    pub const fn new(
        command_id: Id<Command>,
        operation_id: OperationId,
        canonical_request_digest: Sha256Digest,
    ) -> Self {
        Self {
            command_id,
            operation_id,
            canonical_request_digest,
        }
    }

    #[must_use]
    pub const fn command_id(&self) -> Id<Command> {
        self.command_id
    }
    #[must_use]
    pub const fn operation_id(&self) -> OperationId {
        self.operation_id
    }
    #[must_use]
    pub const fn canonical_request_digest(&self) -> Sha256Digest {
        self.canonical_request_digest
    }
}

pub struct ExternalIngestClaim {
    binding: CommandBinding,
    claimed_at: Timestamp,
}

impl ExternalIngestClaim {
    pub fn new(binding: CommandBinding, claimed_at: Timestamp) -> Result<Self, AssetStoreError> {
        require_operation(&binding, ASSET_INGEST_COPY_V1)?;
        Ok(Self {
            binding,
            claimed_at,
        })
    }
    #[must_use]
    pub const fn binding(&self) -> &CommandBinding {
        &self.binding
    }
    #[must_use]
    pub const fn claimed_at(&self) -> Timestamp {
        self.claimed_at
    }
}

pub struct ManagedRegistrationPlan {
    asset_id: Id<Asset>,
    asset_kind: AssetKind,
    asset_revision_id: Id<AssetRevision>,
    content_kind: ContentKind,
    representation_id: Id<Representation>,
    representation_purpose: RepresentationPurpose,
    resource_id: Id<Resource>,
    resource_kind: ResourceKind,
    logical_name: LogicalName,
    media_type: Option<MediaType>,
    candidate_location_id: Id<Location>,
}

impl ManagedRegistrationPlan {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub const fn new(
        asset_id: Id<Asset>,
        asset_kind: AssetKind,
        asset_revision_id: Id<AssetRevision>,
        content_kind: ContentKind,
        representation_id: Id<Representation>,
        representation_purpose: RepresentationPurpose,
        resource_id: Id<Resource>,
        resource_kind: ResourceKind,
        logical_name: LogicalName,
        media_type: Option<MediaType>,
        candidate_location_id: Id<Location>,
    ) -> Self {
        Self {
            asset_id,
            asset_kind,
            asset_revision_id,
            content_kind,
            representation_id,
            representation_purpose,
            resource_id,
            resource_kind,
            logical_name,
            media_type,
            candidate_location_id,
        }
    }
    #[must_use]
    pub const fn asset_id(&self) -> Id<Asset> {
        self.asset_id
    }
    #[must_use]
    pub const fn asset_kind(&self) -> &AssetKind {
        &self.asset_kind
    }
    #[must_use]
    pub const fn asset_revision_id(&self) -> Id<AssetRevision> {
        self.asset_revision_id
    }
    #[must_use]
    pub const fn content_kind(&self) -> &ContentKind {
        &self.content_kind
    }
    #[must_use]
    pub const fn representation_id(&self) -> Id<Representation> {
        self.representation_id
    }
    #[must_use]
    pub const fn representation_purpose(&self) -> &RepresentationPurpose {
        &self.representation_purpose
    }
    #[must_use]
    pub const fn resource_id(&self) -> Id<Resource> {
        self.resource_id
    }
    #[must_use]
    pub const fn resource_kind(&self) -> &ResourceKind {
        &self.resource_kind
    }
    #[must_use]
    pub const fn logical_name(&self) -> &LogicalName {
        &self.logical_name
    }
    #[must_use]
    pub const fn media_type(&self) -> Option<&MediaType> {
        self.media_type.as_ref()
    }
    #[must_use]
    pub const fn candidate_location_id(&self) -> Id<Location> {
        self.candidate_location_id
    }
}

pub struct ExternalIngestCompletion {
    binding: CommandBinding,
    durable_blob: DurableBlob,
    plan: ManagedRegistrationPlan,
    domain_event_id: Id<DomainEvent>,
    provenance_event_id: Id<ProvenanceEvent>,
    completed_at: Timestamp,
}

impl ExternalIngestCompletion {
    pub fn new(
        binding: CommandBinding,
        durable_blob: DurableBlob,
        plan: ManagedRegistrationPlan,
        domain_event_id: Id<DomainEvent>,
        provenance_event_id: Id<ProvenanceEvent>,
        completed_at: Timestamp,
    ) -> Result<Self, AssetStoreError> {
        require_operation(&binding, ASSET_INGEST_COPY_V1)?;
        let object_ids = [
            plan.asset_id().to_bytes(),
            plan.asset_revision_id().to_bytes(),
            plan.representation_id().to_bytes(),
            plan.resource_id().to_bytes(),
            plan.candidate_location_id().to_bytes(),
            domain_event_id.to_bytes(),
            provenance_event_id.to_bytes(),
        ];
        if !pairwise_unique(&object_ids) {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self {
            binding,
            durable_blob,
            plan,
            domain_event_id,
            provenance_event_id,
            completed_at,
        })
    }
    #[must_use]
    pub const fn binding(&self) -> &CommandBinding {
        &self.binding
    }
    #[must_use]
    pub const fn durable_blob(&self) -> &DurableBlob {
        &self.durable_blob
    }
    #[must_use]
    pub const fn plan(&self) -> &ManagedRegistrationPlan {
        &self.plan
    }
    #[must_use]
    pub const fn domain_event_id(&self) -> Id<DomainEvent> {
        self.domain_event_id
    }
    #[must_use]
    pub const fn provenance_event_id(&self) -> Id<ProvenanceEvent> {
        self.provenance_event_id
    }
    #[must_use]
    pub const fn completed_at(&self) -> Timestamp {
        self.completed_at
    }
}

fn pairwise_unique(values: &[[u8; 16]]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[index + 1..].contains(value))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalDisposition {
    TerminalRejected(ErrorCode),
    RecoveryRequired(ErrorCode),
}

pub struct ExternalIngestDisposition {
    binding: CommandBinding,
    disposition: ExternalDisposition,
    observed_at: Timestamp,
}

impl ExternalIngestDisposition {
    pub fn new(
        binding: CommandBinding,
        disposition: ExternalDisposition,
        observed_at: Timestamp,
    ) -> Result<Self, AssetStoreError> {
        require_operation(&binding, ASSET_INGEST_COPY_V1)?;
        let accepted = match disposition {
            ExternalDisposition::TerminalRejected(code) => matches!(
                code,
                ErrorCode::ValidationError
                    | ErrorCode::SourceModifiedDuringIngest
                    | ErrorCode::StorageIoError
                    | ErrorCode::StorageCorruption
                    | ErrorCode::StorageConfigurationError
                    | ErrorCode::Backpressure
                    | ErrorCode::InternalError
                    | ErrorCode::IdGenerationUnavailable
                    | ErrorCode::DeadlineExceeded
                    | ErrorCode::OperationCancelled
            ),
            ExternalDisposition::RecoveryRequired(code) => matches!(
                code,
                ErrorCode::StorageConfigurationError
                    | ErrorCode::StorageIoError
                    | ErrorCode::IdGenerationUnavailable
                    | ErrorCode::InternalError
            ),
        };
        if !accepted {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self {
            binding,
            disposition,
            observed_at,
        })
    }
    #[must_use]
    pub const fn binding(&self) -> &CommandBinding {
        &self.binding
    }
    #[must_use]
    pub const fn disposition(&self) -> ExternalDisposition {
        self.disposition
    }
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
}

pub struct CreateAssetRevisionCommand {
    binding: CommandBinding,
    revision: NewAssetRevision,
    domain_event_id: Id<DomainEvent>,
    provenance_event_id: Id<ProvenanceEvent>,
    operation_at: Timestamp,
}

impl CreateAssetRevisionCommand {
    pub fn new(
        binding: CommandBinding,
        revision: NewAssetRevision,
        domain_event_id: Id<DomainEvent>,
        provenance_event_id: Id<ProvenanceEvent>,
        operation_at: Timestamp,
    ) -> Result<Self, AssetStoreError> {
        require_operation(&binding, ASSET_REVISION_CREATE_V1)?;
        Ok(Self {
            binding,
            revision,
            domain_event_id,
            provenance_event_id,
            operation_at,
        })
    }
    #[must_use]
    pub const fn binding(&self) -> &CommandBinding {
        &self.binding
    }
    #[must_use]
    pub const fn revision(&self) -> &NewAssetRevision {
        &self.revision
    }
    #[must_use]
    pub const fn domain_event_id(&self) -> Id<DomainEvent> {
        self.domain_event_id
    }
    #[must_use]
    pub const fn provenance_event_id(&self) -> Id<ProvenanceEvent> {
        self.provenance_event_id
    }
    #[must_use]
    pub const fn operation_at(&self) -> Timestamp {
        self.operation_at
    }
}

pub struct RecordManagedLocationCommand {
    binding: CommandBinding,
    durable_blob: DurableBlob,
    candidate_location_id: Id<Location>,
    expected_revision: RevisionNo,
    domain_event_id: Id<DomainEvent>,
    operation_at: Timestamp,
}

impl RecordManagedLocationCommand {
    pub fn new(
        binding: CommandBinding,
        durable_blob: DurableBlob,
        candidate_location_id: Id<Location>,
        expected_revision: RevisionNo,
        domain_event_id: Id<DomainEvent>,
        operation_at: Timestamp,
    ) -> Result<Self, AssetStoreError> {
        require_operation(&binding, BLOB_LOCATION_RECORD_V1)?;
        Ok(Self {
            binding,
            durable_blob,
            candidate_location_id,
            expected_revision,
            domain_event_id,
            operation_at,
        })
    }
    #[must_use]
    pub const fn binding(&self) -> &CommandBinding {
        &self.binding
    }
    #[must_use]
    pub const fn durable_blob(&self) -> &DurableBlob {
        &self.durable_blob
    }
    #[must_use]
    pub const fn candidate_location_id(&self) -> Id<Location> {
        self.candidate_location_id
    }
    #[must_use]
    pub const fn expected_revision(&self) -> RevisionNo {
        self.expected_revision
    }
    #[must_use]
    pub const fn domain_event_id(&self) -> Id<DomainEvent> {
        self.domain_event_id
    }
    #[must_use]
    pub const fn operation_at(&self) -> Timestamp {
        self.operation_at
    }
}

fn require_operation(
    binding: &CommandBinding,
    expected: OperationId,
) -> Result<(), AssetStoreError> {
    if binding.operation_id == expected {
        Ok(())
    } else {
        Err(AssetStoreError::Validation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManagedRegistrationResult {
    asset_id: Id<Asset>,
    asset_revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
    location_id: Id<Location>,
    blob_digest: Sha256Digest,
}

impl ManagedRegistrationResult {
    #[must_use]
    pub const fn new(
        asset_id: Id<Asset>,
        asset_revision_id: Id<AssetRevision>,
        representation_id: Id<Representation>,
        resource_id: Id<Resource>,
        location_id: Id<Location>,
        blob_digest: Sha256Digest,
    ) -> Self {
        Self {
            asset_id,
            asset_revision_id,
            representation_id,
            resource_id,
            location_id,
            blob_digest,
        }
    }
    #[must_use]
    pub const fn asset_id(self) -> Id<Asset> {
        self.asset_id
    }
    #[must_use]
    pub const fn asset_revision_id(self) -> Id<AssetRevision> {
        self.asset_revision_id
    }
    #[must_use]
    pub const fn representation_id(self) -> Id<Representation> {
        self.representation_id
    }
    #[must_use]
    pub const fn resource_id(self) -> Id<Resource> {
        self.resource_id
    }
    #[must_use]
    pub const fn location_id(self) -> Id<Location> {
        self.location_id
    }
    #[must_use]
    pub const fn blob_digest(self) -> Sha256Digest {
        self.blob_digest
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetRevisionResult {
    asset_id: Id<Asset>,
    asset_revision_id: Id<AssetRevision>,
    revision: RevisionNo,
}
impl AssetRevisionResult {
    #[must_use]
    pub const fn new(
        asset_id: Id<Asset>,
        asset_revision_id: Id<AssetRevision>,
        revision: RevisionNo,
    ) -> Self {
        Self {
            asset_id,
            asset_revision_id,
            revision,
        }
    }
    #[must_use]
    pub const fn asset_id(self) -> Id<Asset> {
        self.asset_id
    }
    #[must_use]
    pub const fn asset_revision_id(self) -> Id<AssetRevision> {
        self.asset_revision_id
    }
    #[must_use]
    pub const fn revision(self) -> RevisionNo {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocationResult {
    digest: Sha256Digest,
    location_id: Id<Location>,
    revision: RevisionNo,
}
impl LocationResult {
    #[must_use]
    pub const fn new(
        digest: Sha256Digest,
        location_id: Id<Location>,
        revision: RevisionNo,
    ) -> Self {
        Self {
            digest,
            location_id,
            revision,
        }
    }
    #[must_use]
    pub const fn digest(self) -> Sha256Digest {
        self.digest
    }
    #[must_use]
    pub const fn location_id(self) -> Id<Location> {
        self.location_id
    }
    #[must_use]
    pub const fn revision(self) -> RevisionNo {
        self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandResult {
    ManagedRegistration(ManagedRegistrationResult),
    AssetRevision(AssetRevisionResult),
    Location(LocationResult),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalClaimOutcome {
    Claimed,
    InProgress,
    Replay(CommandResult),
    TerminalRejected { safe_error_code: ErrorCode },
    RecoveryRequired { safe_error_code: ErrorCode },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationOutcome {
    Applied(CommandResult),
    Replay(CommandResult),
    TerminalRejected { safe_error_code: ErrorCode },
    RecoveryRequired { safe_error_code: ErrorCode },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalDispositionOutcome {
    Stored,
    Replay { safe_error_code: ErrorCode },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AssetStoreError {
    Validation,
    NotFound,
    Conflict,
    InvalidTransition,
    RevisionExhausted,
    IdGenerationUnavailable,
    StorageBusy,
    StorageIo,
    StorageCorruption,
    StorageConfiguration,
    Backpressure,
    ShuttingDown,
    Internal,
}

impl AssetStoreError {
    #[must_use]
    pub const fn error_code(self) -> ErrorCode {
        match self {
            Self::Validation => ErrorCode::ValidationError,
            Self::NotFound => ErrorCode::NotFound,
            Self::Conflict => ErrorCode::Conflict,
            Self::InvalidTransition => ErrorCode::InvalidTransition,
            Self::RevisionExhausted => ErrorCode::RevisionExhausted,
            Self::IdGenerationUnavailable => ErrorCode::IdGenerationUnavailable,
            Self::StorageBusy => ErrorCode::StorageBusy,
            Self::StorageIo | Self::ShuttingDown => ErrorCode::StorageIoError,
            Self::StorageCorruption => ErrorCode::StorageCorruption,
            Self::StorageConfiguration => ErrorCode::StorageConfigurationError,
            Self::Backpressure => ErrorCode::Backpressure,
            Self::Internal => ErrorCode::InternalError,
        }
    }
}

impl fmt::Display for AssetStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Validation => "asset request validation failed",
            Self::NotFound => "asset object was not found",
            Self::Conflict => "asset command conflicts with durable state",
            Self::InvalidTransition => "asset transition is invalid",
            Self::RevisionExhausted => "asset revision is exhausted",
            Self::IdGenerationUnavailable => "identifier generation is unavailable",
            Self::StorageBusy => "storage is temporarily busy",
            Self::StorageIo => "storage operation failed",
            Self::StorageCorruption => "storage integrity verification failed",
            Self::StorageConfiguration => "storage configuration is unsupported or unsafe",
            Self::Backpressure => "storage admission is full",
            Self::ShuttingDown => "store is shutting down",
            Self::Internal => "internal asset persistence invariant failed",
        })
    }
}
impl std::error::Error for AssetStoreError {}

pub type AssetPortFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, AssetStoreError>> + Send + 'a>>;

pub trait AssetUnitOfWork: Send + Sync {
    fn claim_external_ingest(
        &self,
        request: ExternalIngestClaim,
    ) -> AssetPortFuture<'_, ExternalClaimOutcome>;
    fn complete_external_ingest(
        &self,
        request: ExternalIngestCompletion,
    ) -> AssetPortFuture<'_, MutationOutcome>;
    fn finish_external_ingest(
        &self,
        request: ExternalIngestDisposition,
    ) -> AssetPortFuture<'_, ExternalDispositionOutcome>;
    fn fail_current_runtime_for_unresolved_external_ingest(&self);
    fn execute_create_revision(
        &self,
        request: CreateAssetRevisionCommand,
    ) -> AssetPortFuture<'_, MutationOutcome>;
    fn execute_record_location(
        &self,
        request: RecordManagedLocationCommand,
    ) -> AssetPortFuture<'_, MutationOutcome>;
}

#[cfg(test)]
mod tests {
    use mengxia_types::{ErrorCode, Id, Sha256Digest, Timestamp};

    use super::{
        ASSET_INGEST_COPY_V1, ASSET_REVISION_CREATE_V1, AssetStoreError, BlobRetryClass,
        BlobSourceError, BlobStorageError, Command, CommandBinding, DurableBlob,
        ExternalDisposition, ExternalIngestClaim, ExternalIngestDisposition, pairwise_unique,
    };

    #[test]
    fn managed_completion_id_uniqueness_covers_all_seven_object_ids() {
        let mut values = [[0_u8; 16]; 7];
        for (index, value) in values.iter_mut().enumerate() {
            value[15] = u8::try_from(index).unwrap();
        }
        assert!(pairwise_unique(&values));
        for duplicate_index in 1..values.len() {
            let mut duplicated = values;
            duplicated[duplicate_index] = duplicated[0];
            assert!(
                !pairwise_unique(&duplicated),
                "duplicate at managed object index {duplicate_index} must fail"
            );
        }
    }

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

    #[test]
    fn asset_operation_families_dispositions_and_static_errors_are_closed() {
        assert_eq!(ASSET_INGEST_COPY_V1.as_str(), "asset.ingest.v1");
        assert_eq!(
            ASSET_REVISION_CREATE_V1.as_str(),
            "asset.revision.create.v1"
        );
        let command_id = Id::<Command>::try_new().unwrap();
        let timestamp = Timestamp::from_unix_seconds_nanos(1_700_000_000, 0).unwrap();
        let wrong = CommandBinding::new(
            command_id,
            ASSET_REVISION_CREATE_V1,
            Sha256Digest::from_bytes([1; 32]),
        );
        assert!(matches!(
            ExternalIngestClaim::new(wrong, timestamp),
            Err(AssetStoreError::Validation)
        ));

        let binding = CommandBinding::new(
            command_id,
            ASSET_INGEST_COPY_V1,
            Sha256Digest::from_bytes([2; 32]),
        );
        assert!(
            ExternalIngestDisposition::new(
                binding,
                ExternalDisposition::TerminalRejected(ErrorCode::OperationCancelled),
                timestamp,
            )
            .is_ok()
        );
        assert!(matches!(
            ExternalIngestDisposition::new(
                binding,
                ExternalDisposition::RecoveryRequired(ErrorCode::NotFound),
                timestamp,
            ),
            Err(AssetStoreError::Validation)
        ));

        for (error, code, display) in [
            (
                AssetStoreError::Validation,
                ErrorCode::ValidationError,
                "asset request validation failed",
            ),
            (
                AssetStoreError::Conflict,
                ErrorCode::Conflict,
                "asset command conflicts with durable state",
            ),
            (
                AssetStoreError::StorageCorruption,
                ErrorCode::StorageCorruption,
                "storage integrity verification failed",
            ),
            (
                AssetStoreError::Internal,
                ErrorCode::InternalError,
                "internal asset persistence invariant failed",
            ),
        ] {
            assert_eq!(error.error_code(), code);
            assert_eq!(error.to_string(), display);
        }
    }
}
