use std::sync::Arc;

use mengxia_domain::{Asset, AssetRevision, Representation, Resource};
use mengxia_ports::{
    ASSET_MATERIALIZE_V1, AssetQueryPort, AssetStoreError, Command, CommandBinding, IngestControl,
    IngestDirective, IngestStop, MaterializationCommandBinding, MaterializationDisposition,
    MaterializationEffectRequest, MaterializationFinish, MaterializationObservation,
    MaterializationResult, MaterializationSelection, MaterializationStoragePort,
    MaterializationTransition, MaterializationUnitOfWork,
};
use mengxia_types::{ErrorCode, Id, Sha256Digest};
use sha2::{Digest as _, Sha256};

use crate::asset_persistence::{MaterializationPersistenceService, SystemClock};

const REQUEST_DOMAIN: &[u8] = b"MENGXIA_ASSET_MATERIALIZE_MEMBER_REQUEST_V1";
const DESTINATION_DOMAIN: &[u8] = b"MENGXIA_DESTINATION_SELECTOR_V1";
const NO_REPLACE_POLICY: &[u8] = b"NO_REPLACE_FILE_V1";

pub struct MaterializeAssetRequest {
    command_id: Id<Command>,
    asset_id: Id<Asset>,
    asset_revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
    member_ordinal: u32,
    destination: Vec<u8>,
}

impl MaterializeAssetRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_id: Id<Command>,
        asset_id: Id<Asset>,
        asset_revision_id: Id<AssetRevision>,
        representation_id: Id<Representation>,
        resource_id: Id<Resource>,
        member_ordinal: u32,
        destination: Vec<u8>,
    ) -> Result<Self, ErrorCode> {
        if member_ordinal > 4095
            || destination.is_empty()
            || destination.len() > 1023
            || destination.contains(&0)
        {
            return Err(ErrorCode::ValidationError);
        }
        Ok(Self {
            command_id,
            asset_id,
            asset_revision_id,
            representation_id,
            resource_id,
            member_ordinal,
            destination,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializeAssetResult {
    command_id: Id<Command>,
    asset_revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
    member_ordinal: u32,
    blob_digest: Sha256Digest,
    byte_length: u64,
    replayed: bool,
    cleanup_pending: bool,
}

impl MaterializeAssetResult {
    fn from_stored(value: MaterializationResult, replayed: bool, cleanup_pending: bool) -> Self {
        Self {
            command_id: value.command_id(),
            asset_revision_id: value.asset_revision_id(),
            representation_id: value.representation_id(),
            resource_id: value.resource_id(),
            member_ordinal: value.member_ordinal(),
            blob_digest: value.blob_digest(),
            byte_length: value.byte_length(),
            replayed,
            cleanup_pending,
        }
    }

    #[must_use]
    pub const fn command_id(self) -> Id<Command> {
        self.command_id
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
    pub const fn member_ordinal(self) -> u32 {
        self.member_ordinal
    }
    #[must_use]
    pub const fn blob_digest(self) -> Sha256Digest {
        self.blob_digest
    }
    #[must_use]
    pub const fn byte_length(self) -> u64 {
        self.byte_length
    }
    #[must_use]
    pub const fn replayed(self) -> bool {
        self.replayed
    }
    #[must_use]
    pub const fn cleanup_pending(self) -> bool {
        self.cleanup_pending
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializeAssetFailure {
    code: ErrorCode,
}

impl MaterializeAssetFailure {
    const fn new(code: ErrorCode) -> Self {
        Self { code }
    }
    #[must_use]
    pub const fn code(self) -> ErrorCode {
        self.code
    }
}

pub struct MaterializeAssetService {
    query: Arc<dyn AssetQueryPort>,
    persistence: MaterializationPersistenceService<SystemClock>,
    storage: Arc<dyn MaterializationStoragePort>,
    current_backend_id: String,
}

struct ContinueControl;

impl IngestControl for ContinueControl {
    fn checkpoint(&self) -> IngestDirective {
        IngestDirective::Continue
    }
}

impl MaterializeAssetService {
    pub fn new(
        query: Arc<dyn AssetQueryPort>,
        unit_of_work: Arc<dyn MaterializationUnitOfWork>,
        storage: Arc<dyn MaterializationStoragePort>,
        current_backend_id: String,
    ) -> Result<Self, ErrorCode> {
        if !valid_backend_id(&current_backend_id) {
            return Err(ErrorCode::StorageConfigurationError);
        }
        Ok(Self {
            query,
            persistence: MaterializationPersistenceService::new(unit_of_work, SystemClock),
            storage,
            current_backend_id,
        })
    }

    pub async fn execute(
        &self,
        request: MaterializeAssetRequest,
    ) -> Result<MaterializeAssetResult, MaterializeAssetFailure> {
        self.execute_controlled(request, Arc::new(ContinueControl))
            .await
    }

    pub async fn execute_controlled(
        &self,
        request: MaterializeAssetRequest,
        control: Arc<dyn IngestControl>,
    ) -> Result<MaterializeAssetResult, MaterializeAssetFailure> {
        checkpoint(&control)?;
        let digest = canonical_request_digest(&request);
        let command = MaterializationCommandBinding::new(
            CommandBinding::new(request.command_id, ASSET_MATERIALIZE_V1, digest),
            request.asset_id,
            request.asset_revision_id,
            request.representation_id,
            request.resource_id,
            request.member_ordinal,
        )
        .map_err(store_failure)?;
        checkpoint(&control)?;
        let recovery_safe_code = match self
            .persistence
            .observe(command)
            .await
            .map_err(store_failure)?
        {
            MaterializationObservation::Replay(result) => {
                let selection = MaterializationSelection::new(
                    request.asset_id,
                    request.asset_revision_id,
                    request.representation_id,
                    request.resource_id,
                    request.member_ordinal,
                    self.current_backend_id.clone(),
                )
                .map_err(store_failure)?;
                let member = self
                    .query
                    .resolve_materialization(selection)
                    .await
                    .map_err(store_failure)?;
                if member.blob_digest() != result.blob_digest()
                    || member.byte_length() != result.byte_length()
                {
                    return Err(MaterializeAssetFailure::new(ErrorCode::StorageCorruption));
                }
                let effect =
                    MaterializationEffectRequest::new(command, member, request.destination)
                        .map_err(store_failure)?;
                let cleanup_pending = self
                    .storage
                    .cleanup_completed_materialization(effect)
                    .await
                    .is_err();
                return Ok(MaterializeAssetResult::from_stored(
                    result,
                    true,
                    cleanup_pending,
                ));
            }
            MaterializationObservation::TerminalRejected { safe_error_code } => {
                return Err(MaterializeAssetFailure::new(safe_error_code));
            }
            MaterializationObservation::InProgress => {
                return Err(MaterializeAssetFailure::new(ErrorCode::CommandInProgress));
            }
            MaterializationObservation::RecoveryCandidate { safe_error_code } => {
                Some(safe_error_code)
            }
            MaterializationObservation::Absent => None,
        };

        checkpoint(&control)?;

        let selection = MaterializationSelection::new(
            request.asset_id,
            request.asset_revision_id,
            request.representation_id,
            request.resource_id,
            request.member_ordinal,
            self.current_backend_id.clone(),
        )
        .map_err(store_failure)?;
        let member = self
            .query
            .resolve_materialization(selection)
            .await
            .map_err(store_failure)?;
        let effect = MaterializationEffectRequest::new(command, member, request.destination)
            .map_err(store_failure)?;
        let mut prepared = if recovery_safe_code.is_some() {
            self.storage
                .prepare_materialization_recovery(effect)
                .await
                .map_err(store_failure)?
        } else {
            self.storage
                .prepare_materialization(effect)
                .await
                .map_err(store_failure)?
        };
        checkpoint(&control)?;
        let claimed_at = self.persistence.now().map_err(id_failure)?;
        let transition = MaterializationTransition::new(command, claimed_at);
        let guard = match recovery_safe_code {
            Some(expected_safe_error_code) => self
                .persistence
                .reacquire(transition, expected_safe_error_code)
                .await
                .map_err(store_failure)?,
            None => self
                .persistence
                .claim_new(transition)
                .await
                .map_err(store_failure)?,
        };
        if let Err(failure) = checkpoint(&control) {
            let finish = MaterializationFinish::new(
                MaterializationTransition::new(command, claimed_at),
                MaterializationDisposition::TerminalRejected(failure.code()),
            )
            .map_err(store_failure)?;
            guard.finish(finish).await.map_err(store_failure)?;
            return Err(failure);
        }
        let mut published = match prepared.publish().await {
            Ok(published) => published,
            Err(error) => {
                let code = recovery_code(error);
                let finish = MaterializationFinish::new(
                    MaterializationTransition::new(command, claimed_at),
                    MaterializationDisposition::RecoveryRequired(code),
                )
                .map_err(store_failure)?;
                guard.finish(finish).await.map_err(store_failure)?;
                return Err(MaterializeAssetFailure::new(code));
            }
        };
        let completed_at = match self.persistence.now() {
            Ok(at) => at,
            Err(_) => {
                let finish = MaterializationFinish::new(
                    MaterializationTransition::new(command, claimed_at),
                    MaterializationDisposition::RecoveryRequired(
                        ErrorCode::IdGenerationUnavailable,
                    ),
                )
                .map_err(store_failure)?;
                guard.finish(finish).await.map_err(store_failure)?;
                return Err(MaterializeAssetFailure::new(
                    ErrorCode::IdGenerationUnavailable,
                ));
            }
        };
        let result = guard
            .complete(MaterializationTransition::new(command, completed_at))
            .await
            .map_err(store_failure)?;
        let cleanup_pending = published.cleanup().await.is_err();
        Ok(MaterializeAssetResult::from_stored(
            result,
            false,
            cleanup_pending,
        ))
    }
}

fn checkpoint(control: &Arc<dyn IngestControl>) -> Result<(), MaterializeAssetFailure> {
    match control.checkpoint() {
        IngestDirective::Continue => Ok(()),
        IngestDirective::Stop(IngestStop::Cancelled) => {
            Err(MaterializeAssetFailure::new(ErrorCode::OperationCancelled))
        }
        IngestDirective::Stop(IngestStop::DeadlineReached) => {
            Err(MaterializeAssetFailure::new(ErrorCode::DeadlineExceeded))
        }
    }
}

fn id_failure(_: mengxia_types::IdGenerationError) -> MaterializeAssetFailure {
    MaterializeAssetFailure::new(ErrorCode::IdGenerationUnavailable)
}

fn store_failure(error: AssetStoreError) -> MaterializeAssetFailure {
    MaterializeAssetFailure::new(error.error_code())
}

fn recovery_code(error: AssetStoreError) -> ErrorCode {
    match error {
        AssetStoreError::StorageIo | AssetStoreError::ShuttingDown => ErrorCode::StorageIoError,
        AssetStoreError::StorageConfiguration
        | AssetStoreError::Conflict
        | AssetStoreError::Validation => ErrorCode::StorageConfigurationError,
        AssetStoreError::IdGenerationUnavailable => ErrorCode::IdGenerationUnavailable,
        _ => ErrorCode::InternalError,
    }
}

fn valid_backend_id(value: &str) -> bool {
    const PREFIX: &str = "mengxia.local-cas.v1/";
    value.len() == PREFIX.len() + 64
        && value.starts_with(PREFIX)
        && value[PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn canonical_request_digest(request: &MaterializeAssetRequest) -> Sha256Digest {
    let mut destination = Sha256::new();
    destination.update(DESTINATION_DOMAIN);
    destination.update([0]);
    destination.update((request.destination.len() as u16).to_be_bytes());
    destination.update(&request.destination);
    let destination: [u8; 32] = destination.finalize().into();

    let mut hasher = Sha256::new();
    hasher.update(REQUEST_DOMAIN);
    hasher.update([0]);
    update_tlv(&mut hasher, 1, &request.asset_id.to_bytes());
    update_tlv(&mut hasher, 2, &request.asset_revision_id.to_bytes());
    update_tlv(&mut hasher, 3, &request.representation_id.to_bytes());
    update_tlv(&mut hasher, 4, &request.resource_id.to_bytes());
    update_tlv(&mut hasher, 5, &request.member_ordinal.to_be_bytes());
    update_tlv(&mut hasher, 6, &destination);
    update_tlv(&mut hasher, 7, NO_REPLACE_POLICY);
    Sha256Digest::from_bytes(hasher.finalize().into())
}

fn update_tlv(hasher: &mut Sha256, tag: u8, value: &[u8]) {
    hasher.update([tag]);
    hasher.update((value.len() as u32).to_be_bytes());
    hasher.update(value);
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    use mengxia_ports::{
        AssetMemberPage, AssetPage, AssetPortFuture, ExternalDispositionOutcome, InspectAssetQuery,
        ListAssetsQuery, MaterializationEffectRequest, PreparedMaterializationEffect,
        PublishedMaterializationEffect, ResolvedManagedMember,
    };

    use super::*;

    fn fixed_id<T>(last: u8) -> Id<T> {
        let mut bytes = [
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ];
        bytes[15] = last;
        Id::from_bytes(bytes).unwrap()
    }

    fn request() -> MaterializeAssetRequest {
        MaterializeAssetRequest::new(
            fixed_id(9),
            fixed_id(1),
            fixed_id(2),
            fixed_id(3),
            fixed_id(4),
            0,
            b"/Users/test/out.bin".to_vec(),
        )
        .unwrap()
    }

    fn block_on_ready<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("fake future must be ready"),
        }
    }

    struct FakeQuery {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    impl AssetQueryPort for FakeQuery {
        fn list_assets(&self, _request: ListAssetsQuery) -> AssetPortFuture<'_, AssetPage> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }

        fn inspect_asset(
            &self,
            _request: InspectAssetQuery,
        ) -> AssetPortFuture<'_, AssetMemberPage> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }

        fn resolve_materialization(
            &self,
            request: MaterializationSelection,
        ) -> AssetPortFuture<'_, ResolvedManagedMember> {
            self.events.lock().unwrap().push("resolve");
            Box::pin(async move {
                ResolvedManagedMember::__from_store(
                    request.asset_id(),
                    request.asset_revision_id(),
                    request.representation_id(),
                    request.resource_id(),
                    request.member_ordinal(),
                    Sha256Digest::from_bytes([0x99; 32]),
                    4,
                    Id::try_new().unwrap(),
                    format!("mengxia.local-cas.v1/{}", "55".repeat(32)),
                    format!("sha256-v1/99/99/{}.blob", "99".repeat(32)),
                )
            })
        }
    }

    struct FakeUnitOfWork {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    struct FakeRecoveryUnitOfWork {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    struct FakeReplayUnitOfWork {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    impl MaterializationUnitOfWork for FakeUnitOfWork {
        fn observe_materialization(
            &self,
            _command: MaterializationCommandBinding,
        ) -> AssetPortFuture<'_, MaterializationObservation> {
            self.events.lock().unwrap().push("observe");
            Box::pin(async { Ok(MaterializationObservation::Absent) })
        }

        fn claim_new_materialization(
            &self,
            _transition: MaterializationTransition,
        ) -> AssetPortFuture<'_, ()> {
            self.events.lock().unwrap().push("claim");
            Box::pin(async { Ok(()) })
        }

        fn reacquire_materialization(
            &self,
            _transition: MaterializationTransition,
            _expected_safe_error_code: Option<ErrorCode>,
        ) -> AssetPortFuture<'_, ()> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }

        fn complete_materialization(
            &self,
            transition: MaterializationTransition,
        ) -> AssetPortFuture<'_, MaterializationResult> {
            self.events.lock().unwrap().push("complete");
            let command = *transition.command();
            Box::pin(async move {
                MaterializationResult::__from_store(
                    command.binding().command_id(),
                    command.asset_revision_id(),
                    command.representation_id(),
                    command.resource_id(),
                    command.member_ordinal(),
                    Sha256Digest::from_bytes([0x99; 32]),
                    4,
                )
            })
        }

        fn finish_materialization(
            &self,
            _request: MaterializationFinish,
        ) -> AssetPortFuture<'_, ExternalDispositionOutcome> {
            self.events.lock().unwrap().push("finish");
            Box::pin(async { Ok(ExternalDispositionOutcome::Stored) })
        }

        fn fail_current_runtime_for_unresolved_materialization(&self) {
            self.events.lock().unwrap().push("fail-runtime");
        }
    }

    impl MaterializationUnitOfWork for FakeRecoveryUnitOfWork {
        fn observe_materialization(
            &self,
            _command: MaterializationCommandBinding,
        ) -> AssetPortFuture<'_, MaterializationObservation> {
            self.events.lock().unwrap().push("observe");
            Box::pin(async {
                Ok(MaterializationObservation::RecoveryCandidate {
                    safe_error_code: Some(ErrorCode::StorageIoError),
                })
            })
        }

        fn claim_new_materialization(
            &self,
            _transition: MaterializationTransition,
        ) -> AssetPortFuture<'_, ()> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }

        fn reacquire_materialization(
            &self,
            _transition: MaterializationTransition,
            expected_safe_error_code: Option<ErrorCode>,
        ) -> AssetPortFuture<'_, ()> {
            assert_eq!(expected_safe_error_code, Some(ErrorCode::StorageIoError));
            self.events.lock().unwrap().push("reacquire");
            Box::pin(async { Ok(()) })
        }

        fn complete_materialization(
            &self,
            transition: MaterializationTransition,
        ) -> AssetPortFuture<'_, MaterializationResult> {
            self.events.lock().unwrap().push("complete");
            let command = *transition.command();
            Box::pin(async move {
                MaterializationResult::__from_store(
                    command.binding().command_id(),
                    command.asset_revision_id(),
                    command.representation_id(),
                    command.resource_id(),
                    command.member_ordinal(),
                    Sha256Digest::from_bytes([0x99; 32]),
                    4,
                )
            })
        }

        fn finish_materialization(
            &self,
            _request: MaterializationFinish,
        ) -> AssetPortFuture<'_, ExternalDispositionOutcome> {
            self.events.lock().unwrap().push("finish");
            Box::pin(async { Ok(ExternalDispositionOutcome::Stored) })
        }

        fn fail_current_runtime_for_unresolved_materialization(&self) {
            self.events.lock().unwrap().push("fail-runtime");
        }
    }

    impl MaterializationUnitOfWork for FakeReplayUnitOfWork {
        fn observe_materialization(
            &self,
            command: MaterializationCommandBinding,
        ) -> AssetPortFuture<'_, MaterializationObservation> {
            self.events.lock().unwrap().push("observe");
            Box::pin(async move {
                Ok(MaterializationObservation::Replay(
                    MaterializationResult::__from_store(
                        command.binding().command_id(),
                        command.asset_revision_id(),
                        command.representation_id(),
                        command.resource_id(),
                        command.member_ordinal(),
                        Sha256Digest::from_bytes([0x99; 32]),
                        4,
                    )?,
                ))
            })
        }

        fn claim_new_materialization(
            &self,
            _transition: MaterializationTransition,
        ) -> AssetPortFuture<'_, ()> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }
        fn reacquire_materialization(
            &self,
            _transition: MaterializationTransition,
            _expected_safe_error_code: Option<ErrorCode>,
        ) -> AssetPortFuture<'_, ()> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }
        fn complete_materialization(
            &self,
            _transition: MaterializationTransition,
        ) -> AssetPortFuture<'_, MaterializationResult> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }
        fn finish_materialization(
            &self,
            _request: MaterializationFinish,
        ) -> AssetPortFuture<'_, ExternalDispositionOutcome> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }
        fn fail_current_runtime_for_unresolved_materialization(&self) {
            self.events.lock().unwrap().push("fail-runtime");
        }
    }

    struct FakeStorage {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    struct FakePrepared {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    struct FakePublished {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    struct StopAt {
        call: AtomicUsize,
        stop_at: usize,
        stop: IngestStop,
    }

    impl IngestControl for StopAt {
        fn checkpoint(&self) -> IngestDirective {
            if self.call.fetch_add(1, Ordering::AcqRel) + 1 == self.stop_at {
                IngestDirective::Stop(self.stop)
            } else {
                IngestDirective::Continue
            }
        }
    }

    struct FakeFailingStorage {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    struct FakeFailingPrepared {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    impl MaterializationStoragePort for FakeStorage {
        fn prepare_materialization(
            &self,
            _request: MaterializationEffectRequest,
        ) -> AssetPortFuture<'_, Box<dyn PreparedMaterializationEffect>> {
            self.events.lock().unwrap().push("prepare");
            let prepared = FakePrepared {
                events: Arc::clone(&self.events),
            };
            Box::pin(
                async move { Ok(Box::new(prepared) as Box<dyn PreparedMaterializationEffect>) },
            )
        }

        fn prepare_materialization_recovery(
            &self,
            _request: MaterializationEffectRequest,
        ) -> AssetPortFuture<'_, Box<dyn PreparedMaterializationEffect>> {
            self.events.lock().unwrap().push("prepare-recovery");
            let prepared = FakePrepared {
                events: Arc::clone(&self.events),
            };
            Box::pin(
                async move { Ok(Box::new(prepared) as Box<dyn PreparedMaterializationEffect>) },
            )
        }

        fn cleanup_completed_materialization(
            &self,
            _request: MaterializationEffectRequest,
        ) -> AssetPortFuture<'_, ()> {
            self.events.lock().unwrap().push("cleanup-replay");
            Box::pin(async { Ok(()) })
        }
    }

    impl PreparedMaterializationEffect for FakePrepared {
        fn publish(&mut self) -> AssetPortFuture<'_, Box<dyn PublishedMaterializationEffect>> {
            self.events.lock().unwrap().push("publish");
            let published = FakePublished {
                events: Arc::clone(&self.events),
            };
            Box::pin(
                async move { Ok(Box::new(published) as Box<dyn PublishedMaterializationEffect>) },
            )
        }
    }

    impl PublishedMaterializationEffect for FakePublished {
        fn cleanup(&mut self) -> AssetPortFuture<'_, ()> {
            self.events.lock().unwrap().push("cleanup");
            Box::pin(async { Ok(()) })
        }
    }

    impl MaterializationStoragePort for FakeFailingStorage {
        fn prepare_materialization(
            &self,
            _request: MaterializationEffectRequest,
        ) -> AssetPortFuture<'_, Box<dyn PreparedMaterializationEffect>> {
            self.events.lock().unwrap().push("prepare");
            let prepared = FakeFailingPrepared {
                events: Arc::clone(&self.events),
            };
            Box::pin(
                async move { Ok(Box::new(prepared) as Box<dyn PreparedMaterializationEffect>) },
            )
        }

        fn prepare_materialization_recovery(
            &self,
            _request: MaterializationEffectRequest,
        ) -> AssetPortFuture<'_, Box<dyn PreparedMaterializationEffect>> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }

        fn cleanup_completed_materialization(
            &self,
            _request: MaterializationEffectRequest,
        ) -> AssetPortFuture<'_, ()> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }
    }

    impl PreparedMaterializationEffect for FakeFailingPrepared {
        fn publish(&mut self) -> AssetPortFuture<'_, Box<dyn PublishedMaterializationEffect>> {
            self.events.lock().unwrap().push("publish");
            Box::pin(async { Err(AssetStoreError::StorageIo) })
        }
    }

    #[test]
    fn digest_golden_and_every_selector_field_are_bound() {
        let base = request();
        assert_eq!(
            canonical_request_digest(&base).to_string(),
            "e768d8ec3f9d24ceced7c573c40cf2345e730069c8846b182fd155416fb260b3"
        );
        let mut changed = request();
        changed.member_ordinal = 1;
        assert_ne!(
            canonical_request_digest(&base),
            canonical_request_digest(&changed)
        );
        let mut changed = request();
        changed.destination.push(b'x');
        assert_ne!(
            canonical_request_digest(&base),
            canonical_request_digest(&changed)
        );
    }

    #[test]
    fn new_effect_order_is_prepare_before_claim_and_complete_before_cleanup() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = MaterializeAssetService::new(
            Arc::new(FakeQuery {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeUnitOfWork {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeStorage {
                events: Arc::clone(&events),
            }),
            format!("mengxia.local-cas.v1/{}", "55".repeat(32)),
        )
        .unwrap();
        let result = block_on_ready(service.execute(request())).unwrap();
        assert!(!result.replayed());
        assert!(!result.cleanup_pending());
        assert_eq!(
            *events.lock().unwrap(),
            [
                "observe", "resolve", "prepare", "claim", "publish", "complete", "cleanup"
            ]
        );
    }

    #[test]
    fn post_claim_publish_failure_is_durably_classified_before_return() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = MaterializeAssetService::new(
            Arc::new(FakeQuery {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeUnitOfWork {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeFailingStorage {
                events: Arc::clone(&events),
            }),
            format!("mengxia.local-cas.v1/{}", "55".repeat(32)),
        )
        .unwrap();
        let error = block_on_ready(service.execute(request())).unwrap_err();
        assert_eq!(error.code(), ErrorCode::StorageIoError);
        assert_eq!(
            *events.lock().unwrap(),
            [
                "observe", "resolve", "prepare", "claim", "publish", "finish"
            ]
        );
    }

    #[test]
    fn recovery_is_physically_classified_before_cas_reacquire_and_resume() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = MaterializeAssetService::new(
            Arc::new(FakeQuery {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeRecoveryUnitOfWork {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeStorage {
                events: Arc::clone(&events),
            }),
            format!("mengxia.local-cas.v1/{}", "55".repeat(32)),
        )
        .unwrap();
        let result = block_on_ready(service.execute(request())).unwrap();
        assert!(!result.replayed());
        assert_eq!(
            *events.lock().unwrap(),
            [
                "observe",
                "resolve",
                "prepare-recovery",
                "reacquire",
                "publish",
                "complete",
                "cleanup"
            ]
        );
    }

    #[test]
    fn completed_replay_only_revalidates_graph_and_retries_sidecar_cleanup() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = MaterializeAssetService::new(
            Arc::new(FakeQuery {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeReplayUnitOfWork {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeStorage {
                events: Arc::clone(&events),
            }),
            format!("mengxia.local-cas.v1/{}", "55".repeat(32)),
        )
        .unwrap();
        let result = block_on_ready(service.execute(request())).unwrap();
        assert!(result.replayed());
        assert!(!result.cleanup_pending());
        assert_eq!(
            *events.lock().unwrap(),
            ["observe", "resolve", "cleanup-replay"]
        );
    }

    #[test]
    fn cancellation_before_claim_has_no_ledger_or_physical_effect() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = MaterializeAssetService::new(
            Arc::new(FakeQuery {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeUnitOfWork {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeStorage {
                events: Arc::clone(&events),
            }),
            format!("mengxia.local-cas.v1/{}", "55".repeat(32)),
        )
        .unwrap();
        let control = Arc::new(StopAt {
            call: AtomicUsize::new(0),
            stop_at: 4,
            stop: IngestStop::Cancelled,
        });
        let error = block_on_ready(service.execute_controlled(request(), control)).unwrap_err();
        assert_eq!(error.code(), ErrorCode::OperationCancelled);
        assert_eq!(*events.lock().unwrap(), ["observe", "resolve", "prepare"]);
    }

    #[test]
    fn deadline_after_claim_is_terminally_recorded_before_return() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = MaterializeAssetService::new(
            Arc::new(FakeQuery {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeUnitOfWork {
                events: Arc::clone(&events),
            }),
            Arc::new(FakeStorage {
                events: Arc::clone(&events),
            }),
            format!("mengxia.local-cas.v1/{}", "55".repeat(32)),
        )
        .unwrap();
        let control = Arc::new(StopAt {
            call: AtomicUsize::new(0),
            stop_at: 5,
            stop: IngestStop::DeadlineReached,
        });
        let error = block_on_ready(service.execute_controlled(request(), control)).unwrap_err();
        assert_eq!(error.code(), ErrorCode::DeadlineExceeded);
        assert_eq!(
            *events.lock().unwrap(),
            ["observe", "resolve", "prepare", "claim", "finish"]
        );
    }
}
