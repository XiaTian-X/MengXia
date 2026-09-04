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
    const fn asset_materialize_v1() -> Self {
        Self("asset.materialize.v1")
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

pub const ASSET_INGEST_COPY_V1: OperationId = OperationId::asset_ingest_v1();
pub const ASSET_REVISION_CREATE_V1: OperationId = OperationId::asset_revision_create_v1();
pub const BLOB_LOCATION_RECORD_V1: OperationId = OperationId::blob_location_record_v1();
pub const ASSET_MATERIALIZE_V1: OperationId = OperationId::asset_materialize_v1();

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

/// Validated keyset state for one bounded Asset listing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ListAssetsPosition {
    First,
    After {
        library_id: [u8; 16],
        snapshot_sequence: u64,
        last_examined_sequence: u64,
    },
}

impl ListAssetsPosition {
    pub fn after(
        library_id: [u8; 16],
        snapshot_sequence: u64,
        last_examined_sequence: u64,
    ) -> Result<Self, AssetStoreError> {
        if library_id == [0; 16]
            || snapshot_sequence == 0
            || snapshot_sequence > i64::MAX as u64
            || last_examined_sequence == 0
            || last_examined_sequence >= snapshot_sequence
        {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self::After {
            library_id,
            snapshot_sequence,
            last_examined_sequence,
        })
    }
}

/// Fully validated bounded ListAssets read request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListAssetsQuery {
    page_size: u8,
    position: ListAssetsPosition,
}

impl ListAssetsQuery {
    pub fn new(page_size: u32, position: ListAssetsPosition) -> Result<Self, AssetStoreError> {
        let page_size = u8::try_from(page_size).map_err(|_| AssetStoreError::Validation)?;
        if !(1..=64).contains(&page_size) {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self {
            page_size,
            position,
        })
    }

    #[must_use]
    pub const fn page_size(self) -> u8 {
        self.page_size
    }

    #[must_use]
    pub const fn position(self) -> ListAssetsPosition {
        self.position
    }
}

/// Provider-neutral bounded Asset summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetSummaryView {
    asset_id: Id<Asset>,
    kind: AssetKind,
    lifecycle: mengxia_domain::AssetLifecycle,
    revision: RevisionNo,
    created_at: Timestamp,
    creation_commit_sequence: u64,
}

impl AssetSummaryView {
    #[doc(hidden)]
    pub fn __from_store(
        asset_id: Id<Asset>,
        kind: AssetKind,
        lifecycle: mengxia_domain::AssetLifecycle,
        revision: RevisionNo,
        created_at: Timestamp,
        creation_commit_sequence: u64,
    ) -> Self {
        Self {
            asset_id,
            kind,
            lifecycle,
            revision,
            created_at,
            creation_commit_sequence,
        }
    }

    #[must_use]
    pub const fn asset_id(&self) -> Id<Asset> {
        self.asset_id
    }

    #[must_use]
    pub const fn kind(&self) -> &AssetKind {
        &self.kind
    }

    #[must_use]
    pub const fn lifecycle(&self) -> mengxia_domain::AssetLifecycle {
        self.lifecycle
    }

    #[must_use]
    pub const fn revision(&self) -> RevisionNo {
        self.revision
    }

    #[must_use]
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }

    #[must_use]
    pub const fn creation_commit_sequence(&self) -> u64 {
        self.creation_commit_sequence
    }
}

/// One bounded ListAssets page and its exact continuation state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetPage {
    snapshot_sequence: u64,
    assets: Vec<AssetSummaryView>,
    next: Option<ListAssetsPosition>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectMemberPhase {
    NoLocationSeen = 1,
    LocationSeen = 2,
    RequiredCustodySeen = 3,
}

/// Typed storage continuation; its byte representation remains application-private.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectAssetPosition {
    library_id: [u8; 16],
    asset_id: Id<Asset>,
    selected_revision_id: Id<AssetRevision>,
    asset_revision: RevisionNo,
    member: Option<(Id<Representation>, Id<Resource>, u32)>,
    phase: Option<InspectMemberPhase>,
    blob_revision: Option<RevisionNo>,
    last_location_id: Option<Id<Location>>,
}

impl InspectAssetPosition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        library_id: [u8; 16],
        asset_id: Id<Asset>,
        selected_revision_id: Id<AssetRevision>,
        asset_revision: RevisionNo,
        member: Option<(Id<Representation>, Id<Resource>, u32)>,
        phase: Option<InspectMemberPhase>,
        blob_revision: Option<RevisionNo>,
        last_location_id: Option<Id<Location>>,
    ) -> Result<Self, AssetStoreError> {
        let before_first = member.is_none()
            && phase.is_none()
            && blob_revision.is_none()
            && last_location_id.is_none();
        let location_phase_is_consistent = matches!(
            (phase, last_location_id),
            (Some(InspectMemberPhase::NoLocationSeen), None)
                | (
                    Some(
                        InspectMemberPhase::LocationSeen | InspectMemberPhase::RequiredCustodySeen
                    ),
                    Some(_)
                )
        );
        let enumerating = member.is_some()
            && blob_revision.is_some_and(|revision| revision.get() != 0)
            && location_phase_is_consistent;
        if library_id == [0; 16] || asset_revision.get() == 0 || !(before_first || enumerating) {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self {
            library_id,
            asset_id,
            selected_revision_id,
            asset_revision,
            member,
            phase,
            blob_revision,
            last_location_id,
        })
    }

    #[must_use]
    pub const fn library_id(self) -> [u8; 16] {
        self.library_id
    }

    #[must_use]
    pub const fn asset_id(self) -> Id<Asset> {
        self.asset_id
    }

    #[must_use]
    pub const fn selected_revision_id(self) -> Id<AssetRevision> {
        self.selected_revision_id
    }

    #[must_use]
    pub const fn asset_revision(self) -> RevisionNo {
        self.asset_revision
    }

    #[must_use]
    pub const fn member(self) -> Option<(Id<Representation>, Id<Resource>, u32)> {
        self.member
    }

    #[must_use]
    pub const fn phase(self) -> Option<InspectMemberPhase> {
        self.phase
    }

    #[must_use]
    pub const fn blob_revision(self) -> Option<RevisionNo> {
        self.blob_revision
    }

    #[must_use]
    pub const fn last_location_id(self) -> Option<Id<Location>> {
        self.last_location_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectAssetStart {
    First,
    Continue(InspectAssetPosition),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InspectAssetQuery {
    asset_id: Id<Asset>,
    selected_revision_id: Option<Id<AssetRevision>>,
    page_size: u8,
    start: InspectAssetStart,
}

impl InspectAssetQuery {
    pub fn new(
        asset_id: Id<Asset>,
        selected_revision_id: Option<Id<AssetRevision>>,
        page_size: u32,
        start: InspectAssetStart,
    ) -> Result<Self, AssetStoreError> {
        let page_size = u8::try_from(page_size).map_err(|_| AssetStoreError::Validation)?;
        if !(1..=64).contains(&page_size) {
            return Err(AssetStoreError::Validation);
        }
        if let InspectAssetStart::Continue(position) = start
            && (position.asset_id() != asset_id
                || selected_revision_id
                    .is_some_and(|revision| revision != position.selected_revision_id()))
        {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self {
            asset_id,
            selected_revision_id,
            page_size,
            start,
        })
    }

    #[must_use]
    pub const fn asset_id(self) -> Id<Asset> {
        self.asset_id
    }

    #[must_use]
    pub const fn selected_revision_id(self) -> Option<Id<AssetRevision>> {
        self.selected_revision_id
    }

    #[must_use]
    pub const fn page_size(self) -> u8 {
        self.page_size
    }

    #[must_use]
    pub const fn start(self) -> InspectAssetStart {
        self.start
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetLocationView {
    location_id: Id<Location>,
    lifecycle: mengxia_domain::LocationLifecycle,
    custody: mengxia_domain::LocationCustody,
    durability: mengxia_domain::LocationDurability,
}

impl AssetLocationView {
    #[doc(hidden)]
    pub const fn __from_store(
        location_id: Id<Location>,
        lifecycle: mengxia_domain::LocationLifecycle,
        custody: mengxia_domain::LocationCustody,
        durability: mengxia_domain::LocationDurability,
    ) -> Self {
        Self {
            location_id,
            lifecycle,
            custody,
            durability,
        }
    }

    #[must_use]
    pub const fn location_id(self) -> Id<Location> {
        self.location_id
    }

    #[must_use]
    pub const fn lifecycle(self) -> mengxia_domain::LocationLifecycle {
        self.lifecycle
    }

    #[must_use]
    pub const fn custody(self) -> mengxia_domain::LocationCustody {
        self.custody
    }

    #[must_use]
    pub const fn durability(self) -> mengxia_domain::LocationDurability {
        self.durability
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetMemberView {
    representation_id: Id<Representation>,
    representation_purpose: RepresentationPurpose,
    resource_id: Id<Resource>,
    resource_kind: ResourceKind,
    member_ordinal: u32,
    logical_name: LogicalName,
    blob_digest: Sha256Digest,
    byte_length: u64,
    media_type: Option<MediaType>,
    location: Option<AssetLocationView>,
}

impl AssetMemberView {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __from_store(
        representation_id: Id<Representation>,
        representation_purpose: RepresentationPurpose,
        resource_id: Id<Resource>,
        resource_kind: ResourceKind,
        member_ordinal: u32,
        logical_name: LogicalName,
        blob_digest: Sha256Digest,
        byte_length: u64,
        media_type: Option<MediaType>,
        location: Option<AssetLocationView>,
    ) -> Self {
        Self {
            representation_id,
            representation_purpose,
            resource_id,
            resource_kind,
            member_ordinal,
            logical_name,
            blob_digest,
            byte_length,
            media_type,
            location,
        }
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
    pub const fn member_ordinal(&self) -> u32 {
        self.member_ordinal
    }

    #[must_use]
    pub const fn logical_name(&self) -> &LogicalName {
        &self.logical_name
    }

    #[must_use]
    pub const fn blob_digest(&self) -> Sha256Digest {
        self.blob_digest
    }

    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    #[must_use]
    pub const fn media_type(&self) -> Option<&MediaType> {
        self.media_type.as_ref()
    }

    #[must_use]
    pub const fn location(&self) -> Option<AssetLocationView> {
        self.location
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssetMemberPage {
    asset: AssetSummaryView,
    selected_revision_id: Id<AssetRevision>,
    revision_sequence: u32,
    content_kind: ContentKind,
    custody: mengxia_domain::RevisionCustody,
    parent_revision_ids: Vec<Id<AssetRevision>>,
    members: Vec<AssetMemberView>,
    next: Option<InspectAssetPosition>,
}

/// Exact immutable graph selection plus the composition-owned local backend identity.
///
/// This value is constructed after transport validation; the backend identity is never a
/// caller-controlled protocol field.
#[derive(Clone)]
pub struct MaterializationSelection {
    asset_id: Id<Asset>,
    asset_revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
    member_ordinal: u32,
    current_backend_id: String,
}

impl MaterializationSelection {
    pub fn new(
        asset_id: Id<Asset>,
        asset_revision_id: Id<AssetRevision>,
        representation_id: Id<Representation>,
        resource_id: Id<Resource>,
        member_ordinal: u32,
        current_backend_id: String,
    ) -> Result<Self, AssetStoreError> {
        const PREFIX: &str = "mengxia.local-cas.v1/";
        if member_ordinal > 4095
            || current_backend_id.len() != PREFIX.len() + 64
            || !current_backend_id.starts_with(PREFIX)
            || !current_backend_id[PREFIX.len()..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self {
            asset_id,
            asset_revision_id,
            representation_id,
            resource_id,
            member_ordinal,
            current_backend_id,
        })
    }

    #[must_use]
    pub const fn asset_id(&self) -> Id<Asset> {
        self.asset_id
    }

    #[must_use]
    pub const fn asset_revision_id(&self) -> Id<AssetRevision> {
        self.asset_revision_id
    }

    #[must_use]
    pub const fn representation_id(&self) -> Id<Representation> {
        self.representation_id
    }

    #[must_use]
    pub const fn resource_id(&self) -> Id<Resource> {
        self.resource_id
    }

    #[must_use]
    pub const fn member_ordinal(&self) -> u32 {
        self.member_ordinal
    }

    #[doc(hidden)]
    #[must_use]
    pub fn __current_backend_id(&self) -> &str {
        &self.current_backend_id
    }
}

/// Store-proven managed member and opaque local Location descriptor.
///
/// Deliberately has no `Debug`, `Display` or serialization implementation so backend and locator
/// values cannot enter ordinary application diagnostics.
pub struct ResolvedManagedMember {
    asset_id: Id<Asset>,
    asset_revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
    member_ordinal: u32,
    blob_digest: Sha256Digest,
    byte_length: u64,
    location_id: Id<Location>,
    backend_id: String,
    locator: String,
}

pub enum VerificationReportIdentity {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationMode {
    Normal,
    Deep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityIssueKind {
    DatabaseIntegrityFailure,
    SchemaOrMigrationMismatch,
    LibraryAuthorityMismatch,
    CommandRecoveryRequired,
    EventOrGraphInconsistent,
    LocalBackendMismatch,
    ManagedBlobMissing,
    ManagedBlobUnsafe,
    ManagedBlobLengthMismatch,
    ManagedBlobDigestMismatch,
    UnregisteredCanonicalBlob,
    StagingOrphan,
    UnsafeCasNamespaceEntry,
    MaterializationRecoveryRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegritySeverity {
    FatalLocal,
    ReadOnlyCustody,
    DegradedCustody,
    OperatorAction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityObjectKind {
    Library,
    Command,
    Asset,
    AssetRevision,
    Blob,
    Location,
    Staging,
    Materialization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityRemediation {
    None,
    RetryExactCommand,
    RerunWhenIdle,
    OperatorConfiguration,
    FutureAdminAction,
    OperatorOrRuntimeAction,
}

#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisteredBlobObservation {
    Missing,
    Unsafe,
    LengthMismatch,
    DigestMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegrityObjectId {
    Uuid([u8; 16]),
    Digest(Sha256Digest),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegrityIssue {
    ordinal: u32,
    kind: IntegrityIssueKind,
    severity: IntegritySeverity,
    object_kind: IntegrityObjectKind,
    object_id: Option<IntegrityObjectId>,
    remediation: IntegrityRemediation,
}

impl IntegrityIssue {
    pub fn new(
        ordinal: u32,
        kind: IntegrityIssueKind,
        severity: IntegritySeverity,
        object_kind: IntegrityObjectKind,
        object_id: Option<IntegrityObjectId>,
        remediation: IntegrityRemediation,
    ) -> Result<Self, AssetStoreError> {
        let expected_severity = match kind {
            IntegrityIssueKind::DatabaseIntegrityFailure
            | IntegrityIssueKind::SchemaOrMigrationMismatch
            | IntegrityIssueKind::LibraryAuthorityMismatch
            | IntegrityIssueKind::EventOrGraphInconsistent => IntegritySeverity::FatalLocal,
            IntegrityIssueKind::CommandRecoveryRequired
            | IntegrityIssueKind::UnregisteredCanonicalBlob
            | IntegrityIssueKind::StagingOrphan
            | IntegrityIssueKind::MaterializationRecoveryRequired => {
                IntegritySeverity::OperatorAction
            }
            IntegrityIssueKind::LocalBackendMismatch
            | IntegrityIssueKind::UnsafeCasNamespaceEntry => IntegritySeverity::ReadOnlyCustody,
            IntegrityIssueKind::ManagedBlobMissing
            | IntegrityIssueKind::ManagedBlobUnsafe
            | IntegrityIssueKind::ManagedBlobLengthMismatch
            | IntegrityIssueKind::ManagedBlobDigestMismatch => IntegritySeverity::DegradedCustody,
        };
        let id_matches_kind = match (object_kind, object_id) {
            (_, None) => true,
            (IntegrityObjectKind::Blob, Some(IntegrityObjectId::Digest(_))) => true,
            (IntegrityObjectKind::Blob, Some(IntegrityObjectId::Uuid(_))) => false,
            (_, Some(IntegrityObjectId::Uuid(_))) => true,
            (_, Some(IntegrityObjectId::Digest(_))) => false,
        };
        if ordinal == 0 || severity != expected_severity || !id_matches_kind {
            return Err(AssetStoreError::Internal);
        }
        Ok(Self {
            ordinal,
            kind,
            severity,
            object_kind,
            object_id,
            remediation,
        })
    }

    #[must_use]
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    #[must_use]
    pub const fn kind(self) -> IntegrityIssueKind {
        self.kind
    }

    #[must_use]
    pub const fn severity(self) -> IntegritySeverity {
        self.severity
    }

    #[must_use]
    pub const fn object_kind(self) -> IntegrityObjectKind {
        self.object_kind
    }

    #[must_use]
    pub const fn object_id(self) -> Option<IntegrityObjectId> {
        self.object_id
    }

    #[must_use]
    pub const fn remediation(self) -> IntegrityRemediation {
        self.remediation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegrityFinding {
    kind: IntegrityIssueKind,
    severity: IntegritySeverity,
    object_kind: IntegrityObjectKind,
    object_id: Option<IntegrityObjectId>,
    remediation: IntegrityRemediation,
}

impl IntegrityFinding {
    pub fn new(
        kind: IntegrityIssueKind,
        severity: IntegritySeverity,
        object_kind: IntegrityObjectKind,
        object_id: Option<IntegrityObjectId>,
        remediation: IntegrityRemediation,
    ) -> Result<Self, AssetStoreError> {
        IntegrityIssue::new(1, kind, severity, object_kind, object_id, remediation)?;
        Ok(Self {
            kind,
            severity,
            object_kind,
            object_id,
            remediation,
        })
    }

    #[doc(hidden)]
    pub fn __local_backend_mismatch(location_id: Id<Location>) -> Result<Self, AssetStoreError> {
        Self::new(
            IntegrityIssueKind::LocalBackendMismatch,
            IntegritySeverity::ReadOnlyCustody,
            IntegrityObjectKind::Location,
            Some(IntegrityObjectId::Uuid(location_id.to_bytes())),
            IntegrityRemediation::OperatorConfiguration,
        )
    }

    #[doc(hidden)]
    pub fn __registered_blob_observation(
        observation: RegisteredBlobObservation,
        digest: Sha256Digest,
    ) -> Result<Self, AssetStoreError> {
        let kind = match observation {
            RegisteredBlobObservation::Missing => IntegrityIssueKind::ManagedBlobMissing,
            RegisteredBlobObservation::Unsafe => IntegrityIssueKind::ManagedBlobUnsafe,
            RegisteredBlobObservation::LengthMismatch => {
                IntegrityIssueKind::ManagedBlobLengthMismatch
            }
            RegisteredBlobObservation::DigestMismatch => {
                IntegrityIssueKind::ManagedBlobDigestMismatch
            }
        };
        Self::new(
            kind,
            IntegritySeverity::DegradedCustody,
            IntegrityObjectKind::Blob,
            Some(IntegrityObjectId::Digest(digest)),
            IntegrityRemediation::FutureAdminAction,
        )
    }

    #[must_use]
    pub const fn kind(self) -> IntegrityIssueKind {
        self.kind
    }

    #[must_use]
    pub const fn severity(self) -> IntegritySeverity {
        self.severity
    }

    #[must_use]
    pub const fn object_kind(self) -> IntegrityObjectKind {
        self.object_kind
    }

    #[must_use]
    pub const fn object_id(self) -> Option<IntegrityObjectId> {
        self.object_id
    }

    #[must_use]
    pub const fn remediation(self) -> IntegrityRemediation {
        self.remediation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerificationSummary {
    verification_id: Id<VerificationReportIdentity>,
    mode: VerificationMode,
    snapshot_commit_sequence: u64,
    discovered_issue_count: u64,
    stored_issue_count: u32,
    dropped_issue_count: u64,
    has_fatal_local_issue: bool,
    has_custody_degradation: bool,
    canonical_extra_classification_deferred: bool,
    first_fatal_issue: Option<IntegrityIssue>,
}

impl VerificationSummary {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __from_app(
        verification_id: Id<VerificationReportIdentity>,
        mode: VerificationMode,
        snapshot_commit_sequence: u64,
        discovered_issue_count: u64,
        stored_issue_count: u32,
        dropped_issue_count: u64,
        has_fatal_local_issue: bool,
        has_custody_degradation: bool,
        canonical_extra_classification_deferred: bool,
        first_fatal_issue: Option<IntegrityIssue>,
    ) -> Result<Self, AssetStoreError> {
        if u64::from(stored_issue_count) > discovered_issue_count
            || dropped_issue_count
                != discovered_issue_count.saturating_sub(u64::from(stored_issue_count))
            || has_fatal_local_issue != first_fatal_issue.is_some()
        {
            return Err(AssetStoreError::Internal);
        }
        Ok(Self {
            verification_id,
            mode,
            snapshot_commit_sequence,
            discovered_issue_count,
            stored_issue_count,
            dropped_issue_count,
            has_fatal_local_issue,
            has_custody_degradation,
            canonical_extra_classification_deferred,
            first_fatal_issue,
        })
    }

    #[must_use]
    pub const fn verification_id(self) -> Id<VerificationReportIdentity> {
        self.verification_id
    }

    #[must_use]
    pub const fn mode(self) -> VerificationMode {
        self.mode
    }

    #[must_use]
    pub const fn snapshot_commit_sequence(self) -> u64 {
        self.snapshot_commit_sequence
    }

    #[must_use]
    pub const fn discovered_issue_count(self) -> u64 {
        self.discovered_issue_count
    }

    #[must_use]
    pub const fn stored_issue_count(self) -> u32 {
        self.stored_issue_count
    }

    #[must_use]
    pub const fn dropped_issue_count(self) -> u64 {
        self.dropped_issue_count
    }

    #[must_use]
    pub const fn has_fatal_local_issue(self) -> bool {
        self.has_fatal_local_issue
    }

    #[must_use]
    pub const fn has_custody_degradation(self) -> bool {
        self.has_custody_degradation
    }

    #[must_use]
    pub const fn canonical_extra_classification_deferred(self) -> bool {
        self.canonical_extra_classification_deferred
    }

    #[must_use]
    pub const fn first_fatal_issue(self) -> Option<IntegrityIssue> {
        self.first_fatal_issue
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrityIssuePage {
    verification_id: Id<VerificationReportIdentity>,
    issues: Vec<IntegrityIssue>,
    next_ordinal: Option<u32>,
    discovered_issue_count: u64,
    stored_issue_count: u32,
    dropped_issue_count: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerificationSnapshot {
    library_id: [u8; 16],
    snapshot_commit_sequence: u64,
}

impl VerificationSnapshot {
    #[doc(hidden)]
    pub fn __from_store(
        library_id: [u8; 16],
        snapshot_commit_sequence: u64,
    ) -> Result<Self, AssetStoreError> {
        if library_id == [0; 16] || snapshot_commit_sequence > i64::MAX as u64 {
            return Err(AssetStoreError::StorageCorruption);
        }
        Ok(Self {
            library_id,
            snapshot_commit_sequence,
        })
    }

    #[must_use]
    pub const fn library_id(self) -> [u8; 16] {
        self.library_id
    }

    #[must_use]
    pub const fn snapshot_commit_sequence(self) -> u64 {
        self.snapshot_commit_sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerificationScanPosition {
    CommandsAfter(Option<Id<Command>>),
    ManagedLocationsAfter(Option<Id<Location>>),
    Complete,
}

/// Opaque store-proven physical candidate. Backend and locator are adapter-only values.
pub struct RegisteredBlobVerificationCandidate {
    blob_digest: Sha256Digest,
    byte_length: u64,
    blob_revision: RevisionNo,
    location_id: Id<Location>,
    location_revision: RevisionNo,
    backend_id: String,
    locator: String,
}

impl RegisteredBlobVerificationCandidate {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __from_store(
        blob_digest: Sha256Digest,
        byte_length: u64,
        blob_revision: RevisionNo,
        location_id: Id<Location>,
        location_revision: RevisionNo,
        backend_id: String,
        locator: String,
    ) -> Result<Self, AssetStoreError> {
        if blob_revision.get() == 0
            || location_revision.get() == 0
            || backend_id.is_empty()
            || backend_id.len() > 255
            || backend_id.as_bytes().contains(&0)
            || locator.is_empty()
            || locator.len() > 1024
            || locator.as_bytes().contains(&0)
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        Ok(Self {
            blob_digest,
            byte_length,
            blob_revision,
            location_id,
            location_revision,
            backend_id,
            locator,
        })
    }

    #[must_use]
    pub const fn blob_digest(&self) -> Sha256Digest {
        self.blob_digest
    }

    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    #[must_use]
    pub const fn blob_revision(&self) -> RevisionNo {
        self.blob_revision
    }

    #[must_use]
    pub const fn location_id(&self) -> Id<Location> {
        self.location_id
    }

    #[must_use]
    pub const fn location_revision(&self) -> RevisionNo {
        self.location_revision
    }

    #[doc(hidden)]
    #[must_use]
    pub fn __backend_id_for_local_adapter(&self) -> &str {
        &self.backend_id
    }

    #[doc(hidden)]
    #[must_use]
    pub fn __locator_for_local_adapter(&self) -> &str {
        &self.locator
    }
}

pub struct VerificationStorePage {
    findings: Vec<IntegrityFinding>,
    candidates: Vec<RegisteredBlobVerificationCandidate>,
    next: VerificationScanPosition,
}

impl VerificationStorePage {
    #[doc(hidden)]
    pub fn __from_store(
        findings: Vec<IntegrityFinding>,
        candidates: Vec<RegisteredBlobVerificationCandidate>,
        next: VerificationScanPosition,
    ) -> Result<Self, AssetStoreError> {
        if findings.len() > 256 || candidates.len() > 256 {
            return Err(AssetStoreError::Internal);
        }
        Ok(Self {
            findings,
            candidates,
            next,
        })
    }

    #[must_use]
    pub fn findings(&self) -> &[IntegrityFinding] {
        &self.findings
    }

    #[must_use]
    pub fn candidates(&self) -> &[RegisteredBlobVerificationCandidate] {
        &self.candidates
    }

    #[must_use]
    pub const fn next(&self) -> VerificationScanPosition {
        self.next
    }

    #[doc(hidden)]
    #[must_use]
    pub fn __into_app(
        self,
    ) -> (
        Vec<IntegrityFinding>,
        Vec<RegisteredBlobVerificationCandidate>,
        VerificationScanPosition,
    ) {
        (self.findings, self.candidates, self.next)
    }
}

impl IntegrityIssuePage {
    #[doc(hidden)]
    pub fn __from_app(
        verification_id: Id<VerificationReportIdentity>,
        issues: Vec<IntegrityIssue>,
        next_ordinal: Option<u32>,
        discovered_issue_count: u64,
        stored_issue_count: u32,
        dropped_issue_count: u64,
    ) -> Result<Self, AssetStoreError> {
        if issues.len() > 64
            || u64::from(stored_issue_count) > discovered_issue_count
            || dropped_issue_count
                != discovered_issue_count.saturating_sub(u64::from(stored_issue_count))
        {
            return Err(AssetStoreError::Internal);
        }
        Ok(Self {
            verification_id,
            issues,
            next_ordinal,
            discovered_issue_count,
            stored_issue_count,
            dropped_issue_count,
        })
    }

    #[must_use]
    pub const fn verification_id(&self) -> Id<VerificationReportIdentity> {
        self.verification_id
    }

    #[must_use]
    pub fn issues(&self) -> &[IntegrityIssue] {
        &self.issues
    }

    #[must_use]
    pub const fn next_ordinal(&self) -> Option<u32> {
        self.next_ordinal
    }

    #[must_use]
    pub const fn discovered_issue_count(&self) -> u64 {
        self.discovered_issue_count
    }

    #[must_use]
    pub const fn stored_issue_count(&self) -> u32 {
        self.stored_issue_count
    }

    #[must_use]
    pub const fn dropped_issue_count(&self) -> u64 {
        self.dropped_issue_count
    }
}

impl ResolvedManagedMember {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __from_store(
        asset_id: Id<Asset>,
        asset_revision_id: Id<AssetRevision>,
        representation_id: Id<Representation>,
        resource_id: Id<Resource>,
        member_ordinal: u32,
        blob_digest: Sha256Digest,
        byte_length: u64,
        location_id: Id<Location>,
        backend_id: String,
        locator: String,
    ) -> Result<Self, AssetStoreError> {
        if member_ordinal > 4095
            || backend_id.is_empty()
            || backend_id.len() > 255
            || backend_id.as_bytes().contains(&0)
            || locator.is_empty()
            || locator.len() > 1024
            || locator.as_bytes().contains(&0)
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        Ok(Self {
            asset_id,
            asset_revision_id,
            representation_id,
            resource_id,
            member_ordinal,
            blob_digest,
            byte_length,
            location_id,
            backend_id,
            locator,
        })
    }

    #[must_use]
    pub const fn asset_id(&self) -> Id<Asset> {
        self.asset_id
    }

    #[must_use]
    pub const fn asset_revision_id(&self) -> Id<AssetRevision> {
        self.asset_revision_id
    }

    #[must_use]
    pub const fn representation_id(&self) -> Id<Representation> {
        self.representation_id
    }

    #[must_use]
    pub const fn resource_id(&self) -> Id<Resource> {
        self.resource_id
    }

    #[must_use]
    pub const fn member_ordinal(&self) -> u32 {
        self.member_ordinal
    }

    #[must_use]
    pub const fn blob_digest(&self) -> Sha256Digest {
        self.blob_digest
    }

    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn __location_id_for_local_adapter(&self) -> Id<Location> {
        self.location_id
    }

    #[doc(hidden)]
    #[must_use]
    pub fn __backend_id_for_local_adapter(&self) -> &str {
        &self.backend_id
    }

    #[doc(hidden)]
    #[must_use]
    pub fn __locator_for_local_adapter(&self) -> &str {
        &self.locator
    }
}

impl AssetMemberPage {
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn __from_store(
        asset: AssetSummaryView,
        selected_revision_id: Id<AssetRevision>,
        revision_sequence: u32,
        content_kind: ContentKind,
        custody: mengxia_domain::RevisionCustody,
        parent_revision_ids: Vec<Id<AssetRevision>>,
        members: Vec<AssetMemberView>,
        next: Option<InspectAssetPosition>,
    ) -> Result<Self, AssetStoreError> {
        if revision_sequence == 0 || parent_revision_ids.len() > 64 || members.len() > 64 {
            return Err(AssetStoreError::StorageCorruption);
        }
        Ok(Self {
            asset,
            selected_revision_id,
            revision_sequence,
            content_kind,
            custody,
            parent_revision_ids,
            members,
            next,
        })
    }

    #[must_use]
    pub const fn asset(&self) -> &AssetSummaryView {
        &self.asset
    }

    #[must_use]
    pub const fn selected_revision_id(&self) -> Id<AssetRevision> {
        self.selected_revision_id
    }

    #[must_use]
    pub const fn revision_sequence(&self) -> u32 {
        self.revision_sequence
    }

    #[must_use]
    pub const fn content_kind(&self) -> &ContentKind {
        &self.content_kind
    }

    #[must_use]
    pub const fn custody(&self) -> mengxia_domain::RevisionCustody {
        self.custody
    }

    #[must_use]
    pub fn parent_revision_ids(&self) -> &[Id<AssetRevision>] {
        &self.parent_revision_ids
    }

    #[must_use]
    pub fn members(&self) -> &[AssetMemberView] {
        &self.members
    }

    #[must_use]
    pub const fn next(&self) -> Option<InspectAssetPosition> {
        self.next
    }
}

impl AssetPage {
    #[doc(hidden)]
    pub fn __from_store(
        snapshot_sequence: u64,
        assets: Vec<AssetSummaryView>,
        next: Option<ListAssetsPosition>,
    ) -> Result<Self, AssetStoreError> {
        if assets.len() > 64
            || assets
                .iter()
                .any(|asset| asset.creation_commit_sequence > snapshot_sequence)
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        Ok(Self {
            snapshot_sequence,
            assets,
            next,
        })
    }

    #[must_use]
    pub const fn snapshot_sequence(&self) -> u64 {
        self.snapshot_sequence
    }

    #[must_use]
    pub fn assets(&self) -> &[AssetSummaryView] {
        &self.assets
    }

    #[must_use]
    pub const fn next(&self) -> Option<ListAssetsPosition> {
        self.next
    }
}

pub trait AssetQueryPort: Send + Sync {
    fn list_assets(&self, request: ListAssetsQuery) -> AssetPortFuture<'_, AssetPage>;
    fn inspect_asset(&self, request: InspectAssetQuery) -> AssetPortFuture<'_, AssetMemberPage>;
    fn resolve_materialization(
        &self,
        request: MaterializationSelection,
    ) -> AssetPortFuture<'_, ResolvedManagedMember>;
}

pub trait VerificationStorePort: Send + Sync {
    fn capture_verification_snapshot(&self) -> AssetPortFuture<'_, VerificationSnapshot>;
    fn scan_verification_page(
        &self,
        snapshot: VerificationSnapshot,
        position: VerificationScanPosition,
    ) -> AssetPortFuture<'_, VerificationStorePage>;
}

pub trait RegisteredBlobVerificationPort: Send + Sync {
    fn verify_registered_blob(
        &self,
        candidate: RegisteredBlobVerificationCandidate,
        mode: VerificationMode,
    ) -> AssetPortFuture<'_, Option<IntegrityFinding>>;
}

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
