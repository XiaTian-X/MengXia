use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mengxia_domain::{
    Asset, AssetKind, AssetRevision, ContentKind, Location, LogicalName, Representation,
    RepresentationPurpose, Resource, ResourceKind,
};
use mengxia_events::{DomainEvent, ProvenanceEvent};
use mengxia_ports::{
    ASSET_INGEST_COPY_V1, AssetStoreError, AssetUnitOfWork, BlobSourceError, BlobStorage,
    BlobStorageError, Command, CommandBinding, CommandResult, ExternalClaimOutcome,
    ExternalDisposition, ExternalIngestClaim, ExternalIngestCompletion, ExternalIngestDisposition,
    IngestControl, IngestDirective, IngestOutcome, IngestStop, ManagedRegistrationPlan,
    ManagedRegistrationResult, MutationOutcome,
};
use mengxia_types::{ErrorCode, Id, Sha256Digest};
use sha2::{Digest as _, Sha256};

use crate::asset_persistence::{
    AssetPersistenceService, ExternalClaimGuard, SystemAssetIdentitySource, SystemClock,
};

pub struct IngestAssetCopyRequest {
    command_id: Id<Command>,
    source_path: PathBuf,
    asset_kind: AssetKind,
    content_kind: ContentKind,
    representation_purpose: RepresentationPurpose,
    resource_kind: ResourceKind,
    logical_name: LogicalName,
    expected_digest: Option<Sha256Digest>,
}

impl IngestAssetCopyRequest {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        command_id: Id<Command>,
        source_path: PathBuf,
        asset_kind: AssetKind,
        content_kind: ContentKind,
        representation_purpose: RepresentationPurpose,
        resource_kind: ResourceKind,
        logical_name: LogicalName,
        expected_digest: Option<Sha256Digest>,
    ) -> Self {
        Self {
            command_id,
            source_path,
            asset_kind,
            content_kind,
            representation_purpose,
            resource_kind,
            logical_name,
            expected_digest,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IngestAssetCopyResult {
    asset_id: Id<Asset>,
    asset_revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
    location_id: Id<Location>,
    blob_digest: Sha256Digest,
}

impl IngestAssetCopyResult {
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
pub enum IngestRetry {
    No,
    SameCommandAfterBoundedDelay,
    FreshCommandAfterBoundedDelay,
    AfterSourceStabilizesWithSameCommand,
    AfterSourceStabilizesWithFreshCommand,
    AfterOperatorOrRuntimeAction,
}

pub struct IngestAssetFailure {
    code: ErrorCode,
    retry: IngestRetry,
}

impl IngestAssetFailure {
    const fn new(code: ErrorCode, retry: IngestRetry) -> Self {
        Self { code, retry }
    }
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        self.code
    }
    #[must_use]
    pub const fn retry(&self) -> IngestRetry {
        self.retry
    }
}

pub enum IngestAssetExecutionError {
    Respond(IngestAssetFailure),
    RuntimeFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IngestAdmissionLimits {
    binding_capacity: usize,
    execution_capacity: usize,
}

impl IngestAdmissionLimits {
    pub const fn new(binding_capacity: usize, execution_capacity: usize) -> Option<Self> {
        if binding_capacity == 0 || execution_capacity == 0 {
            None
        } else {
            Some(Self {
                binding_capacity,
                execution_capacity,
            })
        }
    }
}

struct AdmissionState {
    bindings: HashMap<Id<Command>, Sha256Digest>,
    executions: usize,
}

struct AdmissionRegistry {
    limits: IngestAdmissionLimits,
    state: Mutex<AdmissionState>,
}

pub struct IngestAssetCopyService<S: BlobStorage> {
    persistence: AssetPersistenceService<SystemAssetIdentitySource, SystemClock>,
    storage: Arc<S>,
    admission: Arc<AdmissionRegistry>,
}

impl<S: BlobStorage> IngestAssetCopyService<S> {
    #[must_use]
    pub fn new(
        store: Arc<dyn AssetUnitOfWork>,
        storage: Arc<S>,
        admission: IngestAdmissionLimits,
    ) -> Self {
        Self {
            persistence: AssetPersistenceService::new(
                store,
                SystemAssetIdentitySource,
                SystemClock,
            ),
            storage,
            admission: Arc::new(AdmissionRegistry {
                limits: admission,
                state: Mutex::new(AdmissionState {
                    bindings: HashMap::new(),
                    executions: 0,
                }),
            }),
        }
    }

    pub async fn execute(
        &self,
        request: IngestAssetCopyRequest,
        control: Arc<dyn IngestControl>,
    ) -> Result<IngestAssetCopyResult, IngestAssetExecutionError> {
        checkpoint(&control, RetryStage::PreClaim)?;
        validate_source_selector(&request.source_path)?;
        let source = self
            .storage
            .open_source(&request.source_path)
            .map_err(map_source_error)?;
        checkpoint(&control, RetryStage::PreClaim)?;
        let digest = canonical_request_digest(&request);
        let binding = CommandBinding::new(request.command_id, ASSET_INGEST_COPY_V1, digest);
        let _admission = AdmissionOwner::acquire(Arc::clone(&self.admission), binding)?;
        checkpoint(&control, RetryStage::PreClaim)?;
        let claimed_at = self.persistence.now().map_err(|_| {
            respond(
                ErrorCode::IdGenerationUnavailable,
                IngestRetry::AfterOperatorOrRuntimeAction,
            )
        })?;
        let claim = ExternalIngestClaim::new(binding, claimed_at)
            .map_err(|_| IngestAssetExecutionError::RuntimeFailed)?;
        let (outcome, guard) = self
            .persistence
            .claim_external(claim)
            .await
            .map_err(map_claim_error)?;
        let guard = match outcome {
            ExternalClaimOutcome::Claimed => guard.ok_or_else(runtime_failed)?,
            ExternalClaimOutcome::InProgress => {
                return Err(respond(
                    ErrorCode::CommandInProgress,
                    IngestRetry::SameCommandAfterBoundedDelay,
                ));
            }
            ExternalClaimOutcome::Replay(result) => {
                let replay = result_from_command(result);
                if replay.is_err() {
                    self.persistence.fail_current_runtime();
                }
                return replay;
            }
            ExternalClaimOutcome::TerminalRejected { safe_error_code } => {
                return Err(respond(
                    safe_error_code,
                    retry_for_terminal(safe_error_code),
                ));
            }
            ExternalClaimOutcome::RecoveryRequired { safe_error_code } => {
                return Err(respond(
                    safe_error_code,
                    IngestRetry::AfterOperatorOrRuntimeAction,
                ));
            }
        };
        if let Err(error) = checkpoint(&control, RetryStage::PostClaim) {
            return finish_stop(&self.persistence, guard, binding, error).await;
        }
        match self
            .storage
            .ingest(source, request.expected_digest, control)
        {
            Ok(IngestOutcome::Stored(blob)) => self.complete(guard, binding, request, blob).await,
            Ok(IngestOutcome::Stopped(stop)) => {
                let failure = stop_failure(stop, RetryStage::PostClaim);
                finish_stop(&self.persistence, guard, binding, failure).await
            }
            Err(error) => self.finish_storage_error(guard, binding, error).await,
        }
    }

    async fn complete(
        &self,
        guard: ExternalClaimGuard,
        binding: CommandBinding,
        request: IngestAssetCopyRequest,
        blob: mengxia_ports::DurableBlob,
    ) -> Result<IngestAssetCopyResult, IngestAssetExecutionError> {
        let completed_at = self.persistence.now().map_err(|_| runtime_failed())?;
        let ids = self.registration_ids().map_err(|_| {
            respond(
                ErrorCode::IdGenerationUnavailable,
                IngestRetry::AfterOperatorOrRuntimeAction,
            )
        });
        let ids = match ids {
            Ok(ids) => ids,
            Err(failure) => {
                return finish_disposition(
                    &self.persistence,
                    guard,
                    binding,
                    ExternalDisposition::RecoveryRequired(ErrorCode::IdGenerationUnavailable),
                    completed_at,
                    failure,
                )
                .await;
            }
        };
        let plan = ManagedRegistrationPlan::new(
            ids.asset_id,
            request.asset_kind,
            ids.asset_revision_id,
            request.content_kind,
            ids.representation_id,
            request.representation_purpose,
            ids.resource_id,
            request.resource_kind,
            request.logical_name,
            None,
            ids.location_id,
        );
        let completion = ExternalIngestCompletion::new(
            binding,
            blob,
            plan,
            ids.domain_event_id,
            ids.provenance_event_id,
            completed_at,
        )
        .map_err(|_| runtime_failed())?;
        match guard.complete(completion).await {
            Ok(MutationOutcome::Applied(result) | MutationOutcome::Replay(result)) => {
                let completed = result_from_command(result);
                if completed.is_err() {
                    self.persistence.fail_current_runtime();
                }
                completed
            }
            Ok(MutationOutcome::RecoveryRequired { safe_error_code }) => Err(respond(
                safe_error_code,
                IngestRetry::AfterOperatorOrRuntimeAction,
            )),
            Ok(MutationOutcome::TerminalRejected { .. }) | Err(_) => {
                self.persistence.fail_current_runtime();
                Err(IngestAssetExecutionError::RuntimeFailed)
            }
        }
    }

    async fn finish_storage_error(
        &self,
        guard: ExternalClaimGuard,
        binding: CommandBinding,
        error: BlobStorageError,
    ) -> Result<IngestAssetCopyResult, IngestAssetExecutionError> {
        let (disposition, failure) = match error {
            BlobStorageError::Validation => terminal(
                ErrorCode::ValidationError,
                IngestRetry::FreshCommandAfterBoundedDelay,
            ),
            BlobStorageError::SourceModified => terminal(
                ErrorCode::SourceModifiedDuringIngest,
                IngestRetry::AfterSourceStabilizesWithFreshCommand,
            ),
            BlobStorageError::Io => terminal(
                ErrorCode::StorageIoError,
                IngestRetry::FreshCommandAfterBoundedDelay,
            ),
            BlobStorageError::Corruption => terminal(
                ErrorCode::StorageCorruption,
                IngestRetry::AfterOperatorOrRuntimeAction,
            ),
            BlobStorageError::Configuration => terminal(
                ErrorCode::StorageConfigurationError,
                IngestRetry::AfterOperatorOrRuntimeAction,
            ),
            BlobStorageError::Backpressure => terminal(
                ErrorCode::Backpressure,
                IngestRetry::FreshCommandAfterBoundedDelay,
            ),
            BlobStorageError::EntropyUnavailable => terminal(
                ErrorCode::IdGenerationUnavailable,
                IngestRetry::AfterOperatorOrRuntimeAction,
            ),
            BlobStorageError::RecoveryRequired | BlobStorageError::StagingNamespaceUnavailable => (
                ExternalDisposition::RecoveryRequired(ErrorCode::StorageConfigurationError),
                respond(
                    ErrorCode::StorageConfigurationError,
                    IngestRetry::AfterOperatorOrRuntimeAction,
                ),
            ),
            BlobStorageError::Conflict
            | BlobStorageError::CleanupFailed
            | BlobStorageError::ShuttingDown
            | BlobStorageError::Internal => {
                return Err(IngestAssetExecutionError::RuntimeFailed);
            }
            _ => return Err(IngestAssetExecutionError::RuntimeFailed),
        };
        let observed_at = self.persistence.now().map_err(|_| runtime_failed())?;
        finish_disposition(
            &self.persistence,
            guard,
            binding,
            disposition,
            observed_at,
            failure,
        )
        .await
    }

    fn registration_ids(&self) -> Result<RegistrationIds, ()> {
        let ids = RegistrationIds {
            asset_id: self.persistence.next_id().map_err(|_| ())?,
            asset_revision_id: self.persistence.next_id().map_err(|_| ())?,
            representation_id: self.persistence.next_id().map_err(|_| ())?,
            resource_id: self.persistence.next_id().map_err(|_| ())?,
            location_id: self.persistence.next_id().map_err(|_| ())?,
            domain_event_id: self.persistence.next_id().map_err(|_| ())?,
            provenance_event_id: self.persistence.next_id().map_err(|_| ())?,
        };
        if !ids.are_pairwise_unique() {
            return Err(());
        }
        Ok(ids)
    }
}

struct RegistrationIds {
    asset_id: Id<Asset>,
    asset_revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
    location_id: Id<Location>,
    domain_event_id: Id<DomainEvent>,
    provenance_event_id: Id<ProvenanceEvent>,
}

impl RegistrationIds {
    fn are_pairwise_unique(&self) -> bool {
        let values = [
            self.asset_id.to_bytes(),
            self.asset_revision_id.to_bytes(),
            self.representation_id.to_bytes(),
            self.resource_id.to_bytes(),
            self.location_id.to_bytes(),
            self.domain_event_id.to_bytes(),
            self.provenance_event_id.to_bytes(),
        ];
        values
            .iter()
            .enumerate()
            .all(|(index, value)| !values[index + 1..].contains(value))
    }
}

struct AdmissionOwner {
    registry: Arc<AdmissionRegistry>,
    command_id: Id<Command>,
}

impl AdmissionOwner {
    fn acquire(
        registry: Arc<AdmissionRegistry>,
        binding: CommandBinding,
    ) -> Result<Self, IngestAssetExecutionError> {
        let mut state = registry.state.lock().map_err(|_| runtime_failed())?;
        if let Some(existing) = state.bindings.get(&binding.command_id()) {
            return Err(if *existing == binding.canonical_request_digest() {
                respond(
                    ErrorCode::CommandInProgress,
                    IngestRetry::SameCommandAfterBoundedDelay,
                )
            } else {
                respond(ErrorCode::Conflict, IngestRetry::No)
            });
        }
        if state.bindings.len() >= registry.limits.binding_capacity
            || state.executions >= registry.limits.execution_capacity
        {
            return Err(respond(
                ErrorCode::Backpressure,
                IngestRetry::SameCommandAfterBoundedDelay,
            ));
        }
        state
            .bindings
            .insert(binding.command_id(), binding.canonical_request_digest());
        state.executions += 1;
        drop(state);
        Ok(Self {
            registry,
            command_id: binding.command_id(),
        })
    }
}

impl Drop for AdmissionOwner {
    fn drop(&mut self) {
        if let Ok(mut state) = self.registry.state.lock() {
            state.bindings.remove(&self.command_id);
            state.executions = state.executions.saturating_sub(1);
        }
    }
}

fn canonical_request_digest(request: &IngestAssetCopyRequest) -> Sha256Digest {
    let path = request.source_path.as_os_str().as_bytes();
    let selector = source_selector_digest(path);
    let mut digest = Sha256::new();
    digest.update(b"MENGXIA_ASSET_INGEST_COPY_REQUEST_V1\0");
    tlv(&mut digest, 1, b"COPY_V1");
    tlv(&mut digest, 2, &selector);
    tlv(&mut digest, 3, request.asset_kind.as_str().as_bytes());
    tlv(&mut digest, 4, request.content_kind.as_str().as_bytes());
    tlv(
        &mut digest,
        5,
        request.representation_purpose.as_str().as_bytes(),
    );
    tlv(&mut digest, 6, request.resource_kind.as_str().as_bytes());
    tlv(&mut digest, 7, request.logical_name.as_str().as_bytes());
    tlv(&mut digest, 8, &[0]);
    let mut expected = Vec::with_capacity(33);
    match request.expected_digest {
        Some(value) => {
            expected.push(1);
            expected.extend_from_slice(&value.to_bytes());
        }
        None => expected.push(0),
    }
    tlv(&mut digest, 9, &expected);
    Sha256Digest::from_bytes(digest.finalize().into())
}

fn source_selector_digest(path: &[u8]) -> [u8; 32] {
    let mut selector = Sha256::new();
    selector.update(b"MENGXIA_SOURCE_SELECTOR_V1\0");
    selector.update(
        u16::try_from(path.len())
            .expect("validated source selector length fits u16")
            .to_be_bytes(),
    );
    selector.update(path);
    selector.finalize().into()
}

fn validate_source_selector(path: &std::path::Path) -> Result<(), IngestAssetExecutionError> {
    let bytes = path.as_os_str().as_bytes();
    let valid = (1..=1023).contains(&bytes.len())
        && !bytes.contains(&0)
        && bytes.first() == Some(&b'/')
        && bytes.len() > 1
        && !bytes.ends_with(b"/")
        && bytes[1..]
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..");
    if valid {
        Ok(())
    } else {
        Err(respond(ErrorCode::ValidationError, IngestRetry::No))
    }
}

fn tlv(digest: &mut Sha256, tag: u8, value: &[u8]) {
    digest.update([tag]);
    digest.update(u32::try_from(value.len()).unwrap_or(u32::MAX).to_be_bytes());
    digest.update(value);
}

#[derive(Clone, Copy)]
enum RetryStage {
    PreClaim,
    PostClaim,
}

fn checkpoint(
    control: &Arc<dyn IngestControl>,
    stage: RetryStage,
) -> Result<(), IngestAssetExecutionError> {
    match control.checkpoint() {
        IngestDirective::Continue => Ok(()),
        IngestDirective::Stop(stop) => Err(stop_failure(stop, stage)),
    }
}

fn stop_failure(stop: IngestStop, stage: RetryStage) -> IngestAssetExecutionError {
    let code = match stop {
        IngestStop::Cancelled => ErrorCode::OperationCancelled,
        IngestStop::DeadlineReached => ErrorCode::DeadlineExceeded,
    };
    let retry = match stage {
        RetryStage::PreClaim => IngestRetry::SameCommandAfterBoundedDelay,
        RetryStage::PostClaim => IngestRetry::FreshCommandAfterBoundedDelay,
    };
    respond(code, retry)
}

async fn finish_stop<I, C>(
    persistence: &AssetPersistenceService<I, C>,
    guard: ExternalClaimGuard,
    binding: CommandBinding,
    failure: IngestAssetExecutionError,
) -> Result<IngestAssetCopyResult, IngestAssetExecutionError>
where
    I: crate::asset_persistence::AssetIdentitySource,
    C: crate::asset_persistence::Clock,
{
    let code = match &failure {
        IngestAssetExecutionError::Respond(value) => value.code(),
        IngestAssetExecutionError::RuntimeFailed => return Err(failure),
    };
    let observed_at = persistence.now().map_err(|_| runtime_failed())?;
    finish_disposition(
        persistence,
        guard,
        binding,
        ExternalDisposition::TerminalRejected(code),
        observed_at,
        failure,
    )
    .await
}

async fn finish_disposition<I, C>(
    persistence: &AssetPersistenceService<I, C>,
    guard: ExternalClaimGuard,
    binding: CommandBinding,
    disposition: ExternalDisposition,
    observed_at: mengxia_types::Timestamp,
    failure: IngestAssetExecutionError,
) -> Result<IngestAssetCopyResult, IngestAssetExecutionError>
where
    I: crate::asset_persistence::AssetIdentitySource,
    C: crate::asset_persistence::Clock,
{
    let request = ExternalIngestDisposition::new(binding, disposition, observed_at)
        .map_err(|_| runtime_failed())?;
    let expected_code = match disposition {
        ExternalDisposition::TerminalRejected(code)
        | ExternalDisposition::RecoveryRequired(code) => code,
    };
    match guard.finish(request).await.map_err(|_| runtime_failed())? {
        mengxia_ports::ExternalDispositionOutcome::Stored => Err(failure),
        mengxia_ports::ExternalDispositionOutcome::Replay { safe_error_code }
            if safe_error_code == expected_code =>
        {
            Err(failure)
        }
        mengxia_ports::ExternalDispositionOutcome::Replay { .. } => {
            persistence.fail_current_runtime();
            Err(runtime_failed())
        }
    }
}

fn terminal(
    code: ErrorCode,
    retry: IngestRetry,
) -> (ExternalDisposition, IngestAssetExecutionError) {
    (
        ExternalDisposition::TerminalRejected(code),
        respond(code, retry),
    )
}

fn map_source_error(error: BlobSourceError) -> IngestAssetExecutionError {
    match error {
        BlobSourceError::InvalidPath | BlobSourceError::UnsupportedType => {
            respond(ErrorCode::ValidationError, IngestRetry::No)
        }
        BlobSourceError::Io => respond(
            ErrorCode::StorageIoError,
            IngestRetry::SameCommandAfterBoundedDelay,
        ),
        BlobSourceError::Modified => respond(
            ErrorCode::SourceModifiedDuringIngest,
            IngestRetry::AfterSourceStabilizesWithSameCommand,
        ),
        _ => runtime_failed(),
    }
}

fn map_claim_error(error: AssetStoreError) -> IngestAssetExecutionError {
    match error {
        AssetStoreError::Validation => respond(ErrorCode::ValidationError, IngestRetry::No),
        AssetStoreError::Conflict => respond(ErrorCode::Conflict, IngestRetry::No),
        AssetStoreError::IdGenerationUnavailable => respond(
            ErrorCode::IdGenerationUnavailable,
            IngestRetry::AfterOperatorOrRuntimeAction,
        ),
        AssetStoreError::StorageBusy => respond(
            ErrorCode::StorageBusy,
            IngestRetry::SameCommandAfterBoundedDelay,
        ),
        AssetStoreError::StorageConfiguration => respond(
            ErrorCode::StorageConfigurationError,
            IngestRetry::AfterOperatorOrRuntimeAction,
        ),
        AssetStoreError::Backpressure => respond(
            ErrorCode::Backpressure,
            IngestRetry::SameCommandAfterBoundedDelay,
        ),
        AssetStoreError::ShuttingDown => respond(
            ErrorCode::StorageIoError,
            IngestRetry::AfterOperatorOrRuntimeAction,
        ),
        _ => runtime_failed(),
    }
}

fn result_from_command(
    result: CommandResult,
) -> Result<IngestAssetCopyResult, IngestAssetExecutionError> {
    match result {
        CommandResult::ManagedRegistration(value) => Ok(result_from_registration(value)),
        _ => Err(runtime_failed()),
    }
}

fn result_from_registration(value: ManagedRegistrationResult) -> IngestAssetCopyResult {
    IngestAssetCopyResult {
        asset_id: value.asset_id(),
        asset_revision_id: value.asset_revision_id(),
        representation_id: value.representation_id(),
        resource_id: value.resource_id(),
        location_id: value.location_id(),
        blob_digest: value.blob_digest(),
    }
}

fn retry_for_terminal(code: ErrorCode) -> IngestRetry {
    match code {
        ErrorCode::SourceModifiedDuringIngest => IngestRetry::AfterSourceStabilizesWithFreshCommand,
        ErrorCode::ValidationError
        | ErrorCode::StorageIoError
        | ErrorCode::Backpressure
        | ErrorCode::DeadlineExceeded
        | ErrorCode::OperationCancelled => IngestRetry::FreshCommandAfterBoundedDelay,
        _ => IngestRetry::AfterOperatorOrRuntimeAction,
    }
}

fn respond(code: ErrorCode, retry: IngestRetry) -> IngestAssetExecutionError {
    IngestAssetExecutionError::Respond(IngestAssetFailure::new(code, retry))
}

fn runtime_failed() -> IngestAssetExecutionError {
    IngestAssetExecutionError::RuntimeFailed
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Condvar, Mutex};
    use std::task::{Context, Poll, Waker};

    use mengxia_domain::{
        Asset, AssetKind, AssetRevision, ContentKind, Location, LogicalName, Representation,
        RepresentationPurpose, Resource, ResourceKind,
    };
    use mengxia_events::{DomainEvent, ProvenanceEvent};
    use mengxia_ports::{
        AssetPortFuture, AssetStoreError, AssetUnitOfWork, BlobSourceError, BlobStorage,
        BlobStorageError, Command, CommandBinding, CommandResult, CreateAssetRevisionCommand,
        DurableBlob, ExternalClaimOutcome, ExternalDisposition, ExternalDispositionOutcome,
        ExternalIngestClaim, ExternalIngestCompletion, ExternalIngestDisposition, IngestControl,
        IngestDirective, IngestOutcome, IngestStop, ManagedRegistrationResult, MutationOutcome,
        RecordManagedLocationCommand,
    };
    use mengxia_types::{ErrorCode, Id, Sha256Digest};

    use super::{
        IngestAdmissionLimits, IngestAssetCopyRequest, IngestAssetCopyService,
        IngestAssetExecutionError, IngestRetry, RegistrationIds, RetryStage,
        canonical_request_digest, map_claim_error, map_source_error, retry_for_terminal,
        source_selector_digest, stop_failure,
    };

    #[test]
    fn generated_registration_ids_reject_every_cross_kind_duplicate() {
        fn raw(tail: u8) -> [u8; 16] {
            let mut bytes = [0x5a; 16];
            bytes[6] = 0x7a;
            bytes[8] = 0x9a;
            bytes[15] = tail;
            bytes
        }

        fn registration_ids(values: [[u8; 16]; 7]) -> RegistrationIds {
            RegistrationIds {
                asset_id: Id::<Asset>::from_bytes(values[0]).unwrap(),
                asset_revision_id: Id::<AssetRevision>::from_bytes(values[1]).unwrap(),
                representation_id: Id::<Representation>::from_bytes(values[2]).unwrap(),
                resource_id: Id::<Resource>::from_bytes(values[3]).unwrap(),
                location_id: Id::<Location>::from_bytes(values[4]).unwrap(),
                domain_event_id: Id::<DomainEvent>::from_bytes(values[5]).unwrap(),
                provenance_event_id: Id::<ProvenanceEvent>::from_bytes(values[6]).unwrap(),
            }
        }

        let unique = [raw(1), raw(2), raw(3), raw(4), raw(5), raw(6), raw(7)];
        assert!(registration_ids(unique).are_pairwise_unique());

        for left in 0..unique.len() {
            for right in left + 1..unique.len() {
                let mut duplicated = unique;
                duplicated[right] = duplicated[left];
                assert!(
                    !registration_ids(duplicated).are_pairwise_unique(),
                    "duplicate managed object pair {left}/{right} must fail"
                );
            }
        }
    }

    #[derive(Default)]
    struct FakeStoreState {
        binding: Option<CommandBinding>,
        completed: Option<ManagedRegistrationResult>,
        claims: usize,
        completions: usize,
        dispositions: Vec<ExternalDisposition>,
    }

    #[derive(Default)]
    struct FakeStore {
        state: Mutex<FakeStoreState>,
        runtime_failures: AtomicUsize,
    }

    impl AssetUnitOfWork for FakeStore {
        fn claim_external_ingest(
            &self,
            request: ExternalIngestClaim,
        ) -> AssetPortFuture<'_, ExternalClaimOutcome> {
            let mut state = self.state.lock().expect("fake store lock");
            state.claims += 1;
            let binding = *request.binding();
            let outcome = match state.binding {
                None => {
                    state.binding = Some(binding);
                    Ok(ExternalClaimOutcome::Claimed)
                }
                Some(existing)
                    if existing.command_id() != binding.command_id()
                        || existing.operation_id() != binding.operation_id()
                        || existing.canonical_request_digest()
                            != binding.canonical_request_digest() =>
                {
                    Err(AssetStoreError::Conflict)
                }
                Some(_) => state
                    .completed
                    .map_or(Ok(ExternalClaimOutcome::InProgress), |result| {
                        Ok(ExternalClaimOutcome::Replay(
                            CommandResult::ManagedRegistration(result),
                        ))
                    }),
            };
            Box::pin(async move { outcome })
        }

        fn complete_external_ingest(
            &self,
            request: ExternalIngestCompletion,
        ) -> AssetPortFuture<'_, MutationOutcome> {
            let result = ManagedRegistrationResult::new(
                request.plan().asset_id(),
                request.plan().asset_revision_id(),
                request.plan().representation_id(),
                request.plan().resource_id(),
                request.plan().candidate_location_id(),
                request.durable_blob().digest(),
            );
            let mut state = self.state.lock().expect("fake store lock");
            assert!(request.plan().media_type().is_none());
            state.completions += 1;
            state.completed = Some(result);
            Box::pin(async move {
                Ok(MutationOutcome::Applied(
                    CommandResult::ManagedRegistration(result),
                ))
            })
        }

        fn finish_external_ingest(
            &self,
            request: ExternalIngestDisposition,
        ) -> AssetPortFuture<'_, ExternalDispositionOutcome> {
            self.state
                .lock()
                .expect("fake store lock")
                .dispositions
                .push(request.disposition());
            Box::pin(async { Ok(ExternalDispositionOutcome::Stored) })
        }

        fn fail_current_runtime_for_unresolved_external_ingest(&self) {
            self.runtime_failures.fetch_add(1, Ordering::Relaxed);
        }

        fn execute_create_revision(
            &self,
            _request: CreateAssetRevisionCommand,
        ) -> AssetPortFuture<'_, MutationOutcome> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }

        fn execute_record_location(
            &self,
            _request: RecordManagedLocationCommand,
        ) -> AssetPortFuture<'_, MutationOutcome> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }
    }

    enum FakeBlobResult {
        Stored,
        Error(BlobStorageError),
    }

    type FakeGate = Arc<(Mutex<(bool, bool)>, Condvar)>;

    struct FakeBlob {
        result: FakeBlobResult,
        opens: AtomicUsize,
        ingests: AtomicUsize,
        gate: Option<FakeGate>,
    }

    impl FakeBlob {
        fn stored() -> Self {
            Self {
                result: FakeBlobResult::Stored,
                opens: AtomicUsize::new(0),
                ingests: AtomicUsize::new(0),
                gate: None,
            }
        }

        fn failing(error: BlobStorageError) -> Self {
            Self {
                result: FakeBlobResult::Error(error),
                opens: AtomicUsize::new(0),
                ingests: AtomicUsize::new(0),
                gate: None,
            }
        }
    }

    impl BlobStorage for FakeBlob {
        type Source = PathBuf;

        fn open_source(&self, path: &std::path::Path) -> Result<Self::Source, BlobSourceError> {
            self.opens.fetch_add(1, Ordering::Relaxed);
            Ok(path.to_path_buf())
        }

        fn ingest(
            &self,
            _source: Self::Source,
            _expected_digest: Option<Sha256Digest>,
            _control: Arc<dyn IngestControl>,
        ) -> Result<IngestOutcome, BlobStorageError> {
            self.ingests.fetch_add(1, Ordering::Relaxed);
            if let Some(gate) = &self.gate {
                let (lock, changed) = &**gate;
                let mut state = lock.lock().expect("fake blob gate");
                state.0 = true;
                changed.notify_all();
                while !state.1 {
                    state = changed.wait(state).expect("fake blob gate wait");
                }
            }
            match self.result {
                FakeBlobResult::Stored => Ok(IngestOutcome::Stored(
                    DurableBlob::__from_verified_local_adapter(
                        Sha256Digest::from_bytes([0x5a; 32]),
                        4096,
                        [0x6b; 32],
                    ),
                )),
                FakeBlobResult::Error(error) => Err(error),
            }
        }
    }

    struct ContinueControl;
    impl IngestControl for ContinueControl {
        fn checkpoint(&self) -> IngestDirective {
            IngestDirective::Continue
        }
    }

    struct StopControl(IngestStop);
    impl IngestControl for StopControl {
        fn checkpoint(&self) -> IngestDirective {
            IngestDirective::Stop(self.0)
        }
    }

    fn block_on_ready<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("fake future must be immediately ready"),
        }
    }

    fn request_for(
        command_id: &str,
        logical_name: &str,
        expected: Option<Sha256Digest>,
    ) -> IngestAssetCopyRequest {
        IngestAssetCopyRequest::new(
            Id::<Command>::from_str(command_id).unwrap(),
            PathBuf::from("/private/tmp/source"),
            AssetKind::new("file").unwrap(),
            ContentKind::new("binary").unwrap(),
            RepresentationPurpose::new("original").unwrap(),
            ResourceKind::new("blob").unwrap(),
            LogicalName::new(logical_name).unwrap(),
            expected,
        )
    }

    fn request(logical_name: &str, expected: Option<Sha256Digest>) -> IngestAssetCopyRequest {
        request_for(
            "018d442f-c000-7a11-8022-334455667788",
            logical_name,
            expected,
        )
    }

    fn stress_iterations() -> usize {
        let iterations = std::env::var("MENGXIA_TASK007_STRESS_ITERATIONS")
            .map_or(Ok(1_usize), |value| value.parse())
            .expect("TASK-007 stress iteration count must be an unsigned integer");
        assert!((1..=100).contains(&iterations));
        iterations
    }

    #[test]
    fn canonical_request_digest_matches_frozen_golden_and_every_field_changes_it() {
        assert_eq!(
            Sha256Digest::from_bytes(source_selector_digest(b"/private/tmp/source")).to_string(),
            "22c73558801d43acb6a0622d1dfb494094cba73e24bb3e9a336f568e96369f6f"
        );
        let canonical = canonical_request_digest(&request("source", None));
        assert_eq!(
            canonical.to_string(),
            "2e8d91b0bfa7e8e6a23662bfdc7d19675994f6f92f0075ab0d345192c01347b2"
        );
        assert_ne!(
            canonical,
            canonical_request_digest(&request("different", None))
        );
        assert_ne!(
            canonical,
            canonical_request_digest(&request("source", Some(Sha256Digest::from_bytes([7; 32]))))
        );
        let mut changed = request("source", None);
        changed.source_path = PathBuf::from("/private/tmp/different");
        assert_ne!(canonical, canonical_request_digest(&changed));
        let mut changed = request("source", None);
        changed.asset_kind = AssetKind::new("document").unwrap();
        assert_ne!(canonical, canonical_request_digest(&changed));
        let mut changed = request("source", None);
        changed.content_kind = ContentKind::new("text").unwrap();
        assert_ne!(canonical, canonical_request_digest(&changed));
        let mut changed = request("source", None);
        changed.representation_purpose = RepresentationPurpose::new("preview").unwrap();
        assert_ne!(canonical, canonical_request_digest(&changed));
        let mut changed = request("source", None);
        changed.resource_kind = ResourceKind::new("stream").unwrap();
        assert_ne!(canonical, canonical_request_digest(&changed));
    }

    #[test]
    fn admission_limits_are_nonzero_and_execution_is_bounded_by_bindings() {
        assert_eq!(IngestAdmissionLimits::new(0, 1), None);
        assert_eq!(IngestAdmissionLimits::new(1, 0), None);
        assert!(IngestAdmissionLimits::new(1, 2).is_some());
        assert!(IngestAdmissionLimits::new(32, 2).is_some());
    }

    fn assert_response(failure: IngestAssetExecutionError, code: ErrorCode, retry: IngestRetry) {
        assert!(matches!(
            failure,
            IngestAssetExecutionError::Respond(value)
                if value.code() == code && value.retry() == retry
        ));
    }

    #[test]
    fn error_and_retry_mapping_is_total_and_fail_closed_at_the_claim_boundary() {
        for (source, code, retry) in [
            (
                BlobSourceError::InvalidPath,
                ErrorCode::ValidationError,
                IngestRetry::No,
            ),
            (
                BlobSourceError::UnsupportedType,
                ErrorCode::ValidationError,
                IngestRetry::No,
            ),
            (
                BlobSourceError::Io,
                ErrorCode::StorageIoError,
                IngestRetry::SameCommandAfterBoundedDelay,
            ),
            (
                BlobSourceError::Modified,
                ErrorCode::SourceModifiedDuringIngest,
                IngestRetry::AfterSourceStabilizesWithSameCommand,
            ),
        ] {
            assert_response(map_source_error(source), code, retry);
        }

        for (store, code, retry) in [
            (
                AssetStoreError::Validation,
                ErrorCode::ValidationError,
                IngestRetry::No,
            ),
            (
                AssetStoreError::Conflict,
                ErrorCode::Conflict,
                IngestRetry::No,
            ),
            (
                AssetStoreError::IdGenerationUnavailable,
                ErrorCode::IdGenerationUnavailable,
                IngestRetry::AfterOperatorOrRuntimeAction,
            ),
            (
                AssetStoreError::StorageBusy,
                ErrorCode::StorageBusy,
                IngestRetry::SameCommandAfterBoundedDelay,
            ),
            (
                AssetStoreError::StorageConfiguration,
                ErrorCode::StorageConfigurationError,
                IngestRetry::AfterOperatorOrRuntimeAction,
            ),
            (
                AssetStoreError::Backpressure,
                ErrorCode::Backpressure,
                IngestRetry::SameCommandAfterBoundedDelay,
            ),
            (
                AssetStoreError::ShuttingDown,
                ErrorCode::StorageIoError,
                IngestRetry::AfterOperatorOrRuntimeAction,
            ),
        ] {
            assert_response(map_claim_error(store), code, retry);
        }
        for fatal in [
            AssetStoreError::NotFound,
            AssetStoreError::InvalidTransition,
            AssetStoreError::RevisionExhausted,
            AssetStoreError::StorageIo,
            AssetStoreError::StorageCorruption,
            AssetStoreError::Internal,
        ] {
            assert!(matches!(
                map_claim_error(fatal),
                IngestAssetExecutionError::RuntimeFailed
            ));
        }

        assert_response(
            stop_failure(IngestStop::DeadlineReached, RetryStage::PreClaim),
            ErrorCode::DeadlineExceeded,
            IngestRetry::SameCommandAfterBoundedDelay,
        );
        assert_response(
            stop_failure(IngestStop::Cancelled, RetryStage::PostClaim),
            ErrorCode::OperationCancelled,
            IngestRetry::FreshCommandAfterBoundedDelay,
        );
        assert_eq!(
            retry_for_terminal(ErrorCode::SourceModifiedDuringIngest),
            IngestRetry::AfterSourceStabilizesWithFreshCommand
        );
        for code in [
            ErrorCode::ValidationError,
            ErrorCode::StorageIoError,
            ErrorCode::Backpressure,
            ErrorCode::DeadlineExceeded,
            ErrorCode::OperationCancelled,
        ] {
            assert_eq!(
                retry_for_terminal(code),
                IngestRetry::FreshCommandAfterBoundedDelay
            );
        }
        assert_eq!(
            retry_for_terminal(ErrorCode::StorageCorruption),
            IngestRetry::AfterOperatorOrRuntimeAction
        );
    }

    #[test]
    fn complete_replay_and_conflict_have_one_physical_and_logical_effect() {
        let store = Arc::new(FakeStore::default());
        let blob = Arc::new(FakeBlob::stored());
        let service = IngestAssetCopyService::new(
            Arc::clone(&store) as Arc<dyn AssetUnitOfWork>,
            Arc::clone(&blob),
            IngestAdmissionLimits::new(4, 2).unwrap(),
        );
        let control: Arc<dyn IngestControl> = Arc::new(ContinueControl);
        let first = block_on_ready(service.execute(request("source", None), Arc::clone(&control)))
            .unwrap_or_else(|_| panic!("first ingest must succeed"));
        let replay = block_on_ready(service.execute(request("source", None), Arc::clone(&control)))
            .unwrap_or_else(|_| panic!("exact replay must succeed"));
        assert_eq!(first, replay);
        assert_eq!(blob.ingests.load(Ordering::Relaxed), 1);

        let conflict = block_on_ready(service.execute(request("different", None), control));
        assert!(matches!(
            conflict,
            Err(IngestAssetExecutionError::Respond(failure))
                if failure.code() == ErrorCode::Conflict && failure.retry() == IngestRetry::No
        ));
        let state = store.state.lock().expect("fake store state");
        assert_eq!(state.completions, 1);
        assert_eq!(state.claims, 3);
        assert!(state.dispositions.is_empty());
    }

    fn active_duplicate_case() {
        let store = Arc::new(FakeStore::default());
        let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
        let blob = Arc::new(FakeBlob {
            gate: Some(Arc::clone(&gate)),
            ..FakeBlob::stored()
        });
        let service = Arc::new(IngestAssetCopyService::new(
            Arc::clone(&store) as Arc<dyn AssetUnitOfWork>,
            Arc::clone(&blob),
            IngestAdmissionLimits::new(4, 2).unwrap(),
        ));
        let first_service = Arc::clone(&service);
        let first = std::thread::spawn(move || {
            block_on_ready(
                first_service.execute(request("source", None), Arc::new(ContinueControl)),
            )
        });
        {
            let (lock, changed) = &*gate;
            let mut state = lock.lock().expect("fake blob gate");
            while !state.0 {
                state = changed.wait(state).expect("fake blob entered wait");
            }
        }
        let duplicate =
            block_on_ready(service.execute(request("source", None), Arc::new(ContinueControl)));
        assert!(matches!(
            duplicate,
            Err(IngestAssetExecutionError::Respond(failure))
                if failure.code() == ErrorCode::CommandInProgress
                    && failure.retry() == IngestRetry::SameCommandAfterBoundedDelay
        ));
        {
            let (lock, changed) = &*gate;
            let mut state = lock.lock().expect("fake blob gate");
            state.1 = true;
            changed.notify_all();
        }
        assert!(first.join().expect("first ingest thread").is_ok());
        assert_eq!(blob.ingests.load(Ordering::Relaxed), 1);
        assert_eq!(store.state.lock().expect("fake store state").completions, 1);
    }

    #[test]
    fn exact_active_duplicate_uses_one_shared_service_and_one_ingest() {
        for _ in 0..stress_iterations() {
            active_duplicate_case();
        }
    }

    fn saturation_case(limits: IngestAdmissionLimits) {
        let store = Arc::new(FakeStore::default());
        let gate = Arc::new((Mutex::new((false, false)), Condvar::new()));
        let blob = Arc::new(FakeBlob {
            gate: Some(Arc::clone(&gate)),
            ..FakeBlob::stored()
        });
        let service = Arc::new(IngestAssetCopyService::new(
            Arc::clone(&store) as Arc<dyn AssetUnitOfWork>,
            Arc::clone(&blob),
            limits,
        ));
        let first_service = Arc::clone(&service);
        let first = std::thread::spawn(move || {
            block_on_ready(
                first_service.execute(request("source", None), Arc::new(ContinueControl)),
            )
        });
        {
            let (lock, changed) = &*gate;
            let mut state = lock.lock().expect("fake blob gate");
            while !state.0 {
                state = changed.wait(state).expect("fake blob entered wait");
            }
        }
        let saturated = block_on_ready(service.execute(
            request_for("018d442f-c000-7a11-8022-334455667789", "second", None),
            Arc::new(ContinueControl),
        ));
        assert!(matches!(
            saturated,
            Err(IngestAssetExecutionError::Respond(failure))
                if failure.code() == ErrorCode::Backpressure
                    && failure.retry() == IngestRetry::SameCommandAfterBoundedDelay
        ));
        assert_eq!(store.state.lock().expect("fake store state").claims, 1);
        {
            let (lock, changed) = &*gate;
            let mut state = lock.lock().expect("fake blob gate");
            state.1 = true;
            changed.notify_all();
        }
        assert!(first.join().expect("first ingest thread").is_ok());
        assert_eq!(blob.ingests.load(Ordering::Relaxed), 1);
        assert_eq!(store.state.lock().expect("fake store state").completions, 1);
    }

    #[test]
    fn binding_and_execution_saturation_are_preclaim_and_leave_no_second_record() {
        for _ in 0..stress_iterations() {
            saturation_case(IngestAdmissionLimits::new(1, 2).unwrap());
            saturation_case(IngestAdmissionLimits::new(4, 1).unwrap());
        }
    }

    #[test]
    fn stops_and_storage_failures_preserve_claim_boundary_and_retry_identity() {
        let preclaim_store = Arc::new(FakeStore::default());
        let preclaim_blob = Arc::new(FakeBlob::stored());
        let preclaim = IngestAssetCopyService::new(
            Arc::clone(&preclaim_store) as Arc<dyn AssetUnitOfWork>,
            Arc::clone(&preclaim_blob),
            IngestAdmissionLimits::new(2, 1).unwrap(),
        );
        let stopped = block_on_ready(preclaim.execute(
            request("source", None),
            Arc::new(StopControl(IngestStop::DeadlineReached)),
        ));
        assert!(matches!(
            stopped,
            Err(IngestAssetExecutionError::Respond(failure))
                if failure.code() == ErrorCode::DeadlineExceeded
                    && failure.retry() == IngestRetry::SameCommandAfterBoundedDelay
        ));
        assert_eq!(preclaim_blob.opens.load(Ordering::Relaxed), 0);
        let mut invalid_source = request("source", None);
        invalid_source.source_path = PathBuf::from("relative/source");
        let invalid = block_on_ready(preclaim.execute(invalid_source, Arc::new(ContinueControl)));
        assert!(matches!(
            invalid,
            Err(IngestAssetExecutionError::Respond(failure))
                if failure.code() == ErrorCode::ValidationError
                    && failure.retry() == IngestRetry::No
        ));
        assert_eq!(preclaim_blob.opens.load(Ordering::Relaxed), 0);
        assert_eq!(
            preclaim_store
                .state
                .lock()
                .expect("fake store state")
                .claims,
            0
        );

        let store = Arc::new(FakeStore::default());
        let blob = Arc::new(FakeBlob::failing(BlobStorageError::SourceModified));
        let service = IngestAssetCopyService::new(
            Arc::clone(&store) as Arc<dyn AssetUnitOfWork>,
            blob,
            IngestAdmissionLimits::new(2, 1).unwrap(),
        );
        let failed =
            block_on_ready(service.execute(request("source", None), Arc::new(ContinueControl)));
        assert!(matches!(
            failed,
            Err(IngestAssetExecutionError::Respond(failure))
                if failure.code() == ErrorCode::SourceModifiedDuringIngest
                    && failure.retry() == IngestRetry::AfterSourceStabilizesWithFreshCommand
        ));
        assert_eq!(
            store.state.lock().expect("fake store state").dispositions,
            [ExternalDisposition::TerminalRejected(
                ErrorCode::SourceModifiedDuringIngest
            )]
        );
        assert_eq!(store.runtime_failures.load(Ordering::Relaxed), 0);
    }
}
