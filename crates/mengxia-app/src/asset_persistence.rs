use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use mengxia_ports::{
    AssetStoreError, AssetUnitOfWork, ExternalClaimOutcome, ExternalIngestClaim,
    ExternalIngestCompletion, ExternalIngestDisposition, InterruptibleSqliteControl,
    MaterializationCommandBinding, MaterializationFinish, MaterializationObservation,
    MaterializationResult, MaterializationTransition, MaterializationUnitOfWork, MutationOutcome,
    StartupMutationClassifierPort, StartupMutationPageRequest, StartupMutationState,
};
use mengxia_types::{Id, IdGenerationError, Timestamp};

pub(crate) trait AssetIdentitySource {
    fn next_id<T>(&self) -> Result<Id<T>, IdGenerationError>;
}

pub(crate) trait Clock {
    fn now(&self) -> Result<Timestamp, IdGenerationError>;
}

pub(crate) struct SystemAssetIdentitySource;

impl AssetIdentitySource for SystemAssetIdentitySource {
    fn next_id<T>(&self) -> Result<Id<T>, IdGenerationError> {
        Id::try_new()
    }
}

pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Result<Timestamp, IdGenerationError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| IdGenerationError::ClockBeforeUnixEpoch)?;
        let seconds = i64::try_from(duration.as_secs())
            .map_err(|_| IdGenerationError::TimestampOutOfRange)?;
        Timestamp::from_unix_seconds_nanos(seconds, duration.subsec_nanos())
            .map_err(|_| IdGenerationError::TimestampOutOfRange)
    }
}

pub(crate) struct AssetPersistenceService<I, C> {
    store: Arc<dyn AssetUnitOfWork>,
    identities: I,
    clock: C,
}

impl<I, C> AssetPersistenceService<I, C>
where
    I: AssetIdentitySource,
    C: Clock,
{
    pub(crate) fn new(store: Arc<dyn AssetUnitOfWork>, identities: I, clock: C) -> Self {
        Self {
            store,
            identities,
            clock,
        }
    }

    pub(crate) fn next_id<T>(&self) -> Result<Id<T>, IdGenerationError> {
        self.identities.next_id()
    }

    pub(crate) fn now(&self) -> Result<Timestamp, IdGenerationError> {
        self.clock.now()
    }

    pub(crate) async fn claim_external(
        &self,
        request: ExternalIngestClaim,
    ) -> Result<(ExternalClaimOutcome, Option<ExternalClaimGuard>), AssetStoreError> {
        let mut guard = ExternalClaimGuard {
            store: Arc::clone(&self.store),
            armed: true,
        };
        match self.store.claim_external_ingest(request).await {
            Ok(ExternalClaimOutcome::Claimed) => Ok((ExternalClaimOutcome::Claimed, Some(guard))),
            Ok(outcome) => {
                guard.disarm();
                Ok((outcome, None))
            }
            Err(error) => {
                guard.disarm();
                Err(error)
            }
        }
    }

    pub(crate) fn fail_current_runtime(&self) {
        self.store
            .fail_current_runtime_for_unresolved_external_ingest();
    }
}

pub(crate) struct ExternalClaimGuard {
    store: Arc<dyn AssetUnitOfWork>,
    armed: bool,
}

impl ExternalClaimGuard {
    pub(crate) async fn complete(
        mut self,
        request: ExternalIngestCompletion,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let result = self.store.complete_external_ingest(request).await;
        if result.is_ok() {
            self.disarm();
        }
        result
    }

    pub(crate) async fn finish(
        mut self,
        request: ExternalIngestDisposition,
    ) -> Result<mengxia_ports::ExternalDispositionOutcome, AssetStoreError> {
        let outcome = self.store.finish_external_ingest(request).await?;
        self.disarm();
        Ok(outcome)
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ExternalClaimGuard {
    fn drop(&mut self) {
        if self.armed {
            self.store
                .fail_current_runtime_for_unresolved_external_ingest();
        }
    }
}

pub(crate) struct MaterializationPersistenceService<C> {
    store: Arc<dyn MaterializationUnitOfWork>,
    clock: C,
}

impl<C: Clock> MaterializationPersistenceService<C> {
    pub(crate) fn new(store: Arc<dyn MaterializationUnitOfWork>, clock: C) -> Self {
        Self { store, clock }
    }

    pub(crate) fn now(&self) -> Result<Timestamp, IdGenerationError> {
        self.clock.now()
    }

    pub(crate) async fn observe(
        &self,
        command: MaterializationCommandBinding,
    ) -> Result<MaterializationObservation, AssetStoreError> {
        self.store.observe_materialization(command).await
    }

    pub(crate) async fn claim_new(
        &self,
        transition: MaterializationTransition,
    ) -> Result<MaterializationClaimGuard, AssetStoreError> {
        self.store.claim_new_materialization(transition).await?;
        Ok(MaterializationClaimGuard {
            store: Arc::clone(&self.store),
            armed: true,
        })
    }

    pub(crate) async fn reacquire(
        &self,
        transition: MaterializationTransition,
        expected_safe_error_code: Option<mengxia_types::ErrorCode>,
    ) -> Result<MaterializationClaimGuard, AssetStoreError> {
        self.store
            .reacquire_materialization(transition, expected_safe_error_code)
            .await?;
        Ok(MaterializationClaimGuard {
            store: Arc::clone(&self.store),
            armed: true,
        })
    }
}

pub(crate) struct MaterializationClaimGuard {
    store: Arc<dyn MaterializationUnitOfWork>,
    armed: bool,
}

impl MaterializationClaimGuard {
    pub(crate) async fn complete(
        mut self,
        transition: MaterializationTransition,
    ) -> Result<MaterializationResult, AssetStoreError> {
        let result = self.store.complete_materialization(transition).await;
        if result.is_ok() {
            self.armed = false;
        }
        result
    }

    pub(crate) async fn finish(
        mut self,
        request: MaterializationFinish,
    ) -> Result<mengxia_ports::ExternalDispositionOutcome, AssetStoreError> {
        let result = self.store.finish_materialization(request).await;
        if result.is_ok() {
            self.armed = false;
        }
        result
    }
}

impl Drop for MaterializationClaimGuard {
    fn drop(&mut self) {
        if self.armed {
            self.store
                .fail_current_runtime_for_unresolved_materialization();
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StartupMutationClassification {
    recovery_required_command_count: u64,
}

impl StartupMutationClassification {
    #[must_use]
    pub const fn recovery_required_command_count(self) -> u64 {
        self.recovery_required_command_count
    }
}

pub struct StartupMutationClassificationService {
    store: Arc<dyn StartupMutationClassifierPort>,
}

impl StartupMutationClassificationService {
    #[must_use]
    pub fn new(store: Arc<dyn StartupMutationClassifierPort>) -> Self {
        Self { store }
    }

    pub async fn classify(
        &self,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> Result<StartupMutationClassification, AssetStoreError> {
        self.classify_with_clock(control, &SystemClock).await
    }

    async fn classify_with_clock<C: Clock>(
        &self,
        control: Arc<dyn InterruptibleSqliteControl>,
        clock: &C,
    ) -> Result<StartupMutationClassification, AssetStoreError> {
        startup_checkpoint(&control)?;
        let boundary = self
            .store
            .capture_startup_mutation_boundary(Arc::clone(&control))
            .await?;
        startup_checkpoint(&control)?;
        let mut recovery_required_command_count = 0_u64;
        for state in [
            StartupMutationState::Claimed,
            StartupMutationState::RecoveryRequired,
        ] {
            let mut after = None;
            loop {
                startup_checkpoint(&control)?;
                let classified_at = clock
                    .now()
                    .map_err(|_| AssetStoreError::IdGenerationUnavailable)?;
                let page = self
                    .store
                    .classify_startup_mutation_page(
                        StartupMutationPageRequest::new(boundary, state, after, classified_at),
                        Arc::clone(&control),
                    )
                    .await?;
                if state == StartupMutationState::Claimed && page.recovery_required_count() != 0 {
                    return Err(AssetStoreError::StorageCorruption);
                }
                if let Some(next) = page.last_command_id()
                    && (after.is_some_and(|previous| next <= previous)
                        || boundary
                            .maximum_command_id()
                            .is_some_and(|maximum| next > maximum))
                {
                    return Err(AssetStoreError::StorageCorruption);
                }
                recovery_required_command_count = recovery_required_command_count
                    .checked_add(page.recovery_required_count())
                    .ok_or(AssetStoreError::StorageCorruption)?;
                if page.complete() {
                    break;
                }
                let next = page
                    .last_command_id()
                    .ok_or(AssetStoreError::StorageCorruption)?;
                after = Some(next);
            }
        }
        Ok(StartupMutationClassification {
            recovery_required_command_count,
        })
    }
}

fn startup_checkpoint(
    control: &Arc<dyn InterruptibleSqliteControl>,
) -> Result<(), AssetStoreError> {
    match control.checkpoint() {
        mengxia_ports::IngestDirective::Continue => Ok(()),
        mengxia_ports::IngestDirective::Stop(mengxia_ports::IngestStop::Cancelled) => {
            Err(AssetStoreError::OperationCancelled)
        }
        mengxia_ports::IngestDirective::Stop(mengxia_ports::IngestStop::DeadlineReached) => {
            Err(AssetStoreError::DeadlineExceeded)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll, Waker};

    use mengxia_ports::{
        ASSET_INGEST_COPY_V1, ASSET_MATERIALIZE_V1, AssetPortFuture, Command, CommandBinding,
        CreateAssetRevisionCommand, ExternalClaimOutcome, ExternalDispositionOutcome,
        ExternalIngestClaim, ExternalIngestCompletion, ExternalIngestDisposition, IngestDirective,
        InterruptibleSqliteControl, MaterializationCommandBinding, MaterializationFinish,
        MaterializationObservation, MaterializationResult, MaterializationTransition,
        MaterializationUnitOfWork, MutationOutcome, RecordManagedLocationCommand, SqliteInterrupt,
        StartupMutationBoundary, StartupMutationClassifierPort, StartupMutationPage,
        StartupMutationPageRequest, StartupMutationState,
    };
    use mengxia_types::{Id, Sha256Digest};

    use super::{
        AssetIdentitySource, AssetPersistenceService, AssetStoreError, AssetUnitOfWork, Clock,
        StartupMutationClassificationService,
    };

    struct FakeStore {
        outcome: ExternalClaimOutcome,
        failures: AtomicUsize,
    }

    impl AssetUnitOfWork for FakeStore {
        fn claim_external_ingest(
            &self,
            _request: ExternalIngestClaim,
        ) -> AssetPortFuture<'_, ExternalClaimOutcome> {
            Box::pin(async move { Ok(self.outcome) })
        }

        fn complete_external_ingest(
            &self,
            _request: ExternalIngestCompletion,
        ) -> AssetPortFuture<'_, MutationOutcome> {
            Box::pin(async { Err(AssetStoreError::Internal) })
        }

        fn finish_external_ingest(
            &self,
            _request: ExternalIngestDisposition,
        ) -> AssetPortFuture<'_, ExternalDispositionOutcome> {
            Box::pin(async { Ok(ExternalDispositionOutcome::Stored) })
        }

        fn fail_current_runtime_for_unresolved_external_ingest(&self) {
            self.failures.fetch_add(1, Ordering::Relaxed);
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

    struct FakeIdentities;
    impl AssetIdentitySource for FakeIdentities {
        fn next_id<T>(&self) -> Result<Id<T>, mengxia_types::IdGenerationError> {
            Id::from_bytes([
                0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
                0x77, 0x88,
            ])
            .map_err(|_| mengxia_types::IdGenerationError::EntropyUnavailable)
        }
    }

    struct FakeClock;
    impl Clock for FakeClock {
        fn now(&self) -> Result<mengxia_types::Timestamp, mengxia_types::IdGenerationError> {
            mengxia_types::Timestamp::from_unix_seconds_nanos(1_700_000_000, 7)
                .map_err(|_| mengxia_types::IdGenerationError::TimestampOutOfRange)
        }
    }

    struct FakeStartupClassifier {
        requests: Mutex<Vec<(StartupMutationState, Option<Id<Command>>)>>,
    }

    impl StartupMutationClassifierPort for FakeStartupClassifier {
        fn capture_startup_mutation_boundary(
            &self,
            _control: Arc<dyn InterruptibleSqliteControl>,
        ) -> AssetPortFuture<'_, StartupMutationBoundary> {
            Box::pin(async {
                Ok(StartupMutationBoundary::__from_store(Some(
                    fixed_command_id(0x99),
                )))
            })
        }

        fn classify_startup_mutation_page(
            &self,
            request: StartupMutationPageRequest,
            _control: Arc<dyn InterruptibleSqliteControl>,
        ) -> AssetPortFuture<'_, StartupMutationPage> {
            self.requests
                .lock()
                .unwrap()
                .push((request.state(), request.after_command_id()));
            Box::pin(async move {
                match (request.state(), request.after_command_id()) {
                    (StartupMutationState::Claimed, None) => {
                        StartupMutationPage::__from_store(Some(fixed_command_id(0x88)), 0, false)
                    }
                    (StartupMutationState::Claimed, Some(after))
                        if after == fixed_command_id(0x88) =>
                    {
                        StartupMutationPage::__from_store(None, 0, true)
                    }
                    (StartupMutationState::RecoveryRequired, None) => {
                        StartupMutationPage::__from_store(Some(fixed_command_id(0x99)), 2, true)
                    }
                    _ => Err(AssetStoreError::StorageCorruption),
                }
            })
        }
    }

    struct ContinueSqliteControl;

    impl mengxia_ports::IngestControl for ContinueSqliteControl {
        fn checkpoint(&self) -> IngestDirective {
            IngestDirective::Continue
        }
    }

    impl InterruptibleSqliteControl for ContinueSqliteControl {
        fn register_interrupt(
            &self,
            _interrupt: Box<dyn SqliteInterrupt>,
        ) -> Result<IngestDirective, mengxia_ports::SqliteInterruptControlError> {
            Ok(IngestDirective::Continue)
        }

        fn clear_interrupt(&self) -> Result<(), mengxia_ports::SqliteInterruptControlError> {
            Ok(())
        }
    }

    fn fixed_command_id(last: u8) -> Id<Command> {
        Id::from_bytes([
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, last,
        ])
        .unwrap()
    }

    fn block_on_ready<F: Future>(future: F) -> F::Output {
        let mut future = Box::pin(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => output,
            Poll::Pending => panic!("fake future must be immediately ready"),
        }
    }

    fn claim() -> ExternalIngestClaim {
        let binding = CommandBinding::new(
            Id::<Command>::try_new().unwrap(),
            ASSET_INGEST_COPY_V1,
            Sha256Digest::from_bytes([9; 32]),
        );
        ExternalIngestClaim::new(
            binding,
            mengxia_types::Timestamp::from_unix_seconds_nanos(1_700_000_000, 0).unwrap(),
        )
        .unwrap()
    }

    struct FakeMaterializationStore {
        failures: AtomicUsize,
    }

    impl MaterializationUnitOfWork for FakeMaterializationStore {
        fn observe_materialization(
            &self,
            _command: MaterializationCommandBinding,
        ) -> AssetPortFuture<'_, MaterializationObservation> {
            Box::pin(async { Ok(MaterializationObservation::Absent) })
        }

        fn claim_new_materialization(
            &self,
            _transition: MaterializationTransition,
        ) -> AssetPortFuture<'_, ()> {
            Box::pin(async { Ok(()) })
        }

        fn reacquire_materialization(
            &self,
            _transition: MaterializationTransition,
            _expected_safe_error_code: Option<mengxia_types::ErrorCode>,
        ) -> AssetPortFuture<'_, ()> {
            Box::pin(async { Ok(()) })
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
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn materialization_transition() -> MaterializationTransition {
        use mengxia_domain::{Asset, AssetRevision, Representation, Resource};

        let command = MaterializationCommandBinding::new(
            CommandBinding::new(
                Id::<Command>::try_new().unwrap(),
                ASSET_MATERIALIZE_V1,
                Sha256Digest::from_bytes([0x77; 32]),
            ),
            Id::<Asset>::try_new().unwrap(),
            Id::<AssetRevision>::try_new().unwrap(),
            Id::<Representation>::try_new().unwrap(),
            Id::<Resource>::try_new().unwrap(),
            0,
        )
        .unwrap();
        MaterializationTransition::new(command, FakeClock.now().unwrap())
    }

    #[test]
    fn unresolved_claim_guard_fails_runtime_but_non_owner_outcome_does_not() {
        let claimed_store = Arc::new(FakeStore {
            outcome: ExternalClaimOutcome::Claimed,
            failures: AtomicUsize::new(0),
        });
        let service = AssetPersistenceService::new(
            Arc::clone(&claimed_store) as Arc<dyn AssetUnitOfWork>,
            FakeIdentities,
            FakeClock,
        );
        let (outcome, guard) = block_on_ready(service.claim_external(claim())).unwrap();
        assert_eq!(outcome, ExternalClaimOutcome::Claimed);
        drop(guard);
        assert_eq!(claimed_store.failures.load(Ordering::Relaxed), 1);

        let in_progress_store = Arc::new(FakeStore {
            outcome: ExternalClaimOutcome::InProgress,
            failures: AtomicUsize::new(0),
        });
        let service = AssetPersistenceService::new(
            Arc::clone(&in_progress_store) as Arc<dyn AssetUnitOfWork>,
            FakeIdentities,
            FakeClock,
        );
        let (outcome, guard) = block_on_ready(service.claim_external(claim())).unwrap();
        assert_eq!(outcome, ExternalClaimOutcome::InProgress);
        assert!(guard.is_none());
        assert_eq!(in_progress_store.failures.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn injected_identity_and_clock_are_typed_and_deterministic() {
        let store = Arc::new(FakeStore {
            outcome: ExternalClaimOutcome::InProgress,
            failures: AtomicUsize::new(0),
        });
        let service = AssetPersistenceService::new(
            store as Arc<dyn AssetUnitOfWork>,
            FakeIdentities,
            FakeClock,
        );
        let identity = service.next_id::<Command>().unwrap();
        assert_eq!(identity.to_bytes()[6] >> 4, 7);
        assert_eq!(service.now().unwrap().unix_seconds(), 1_700_000_000);
    }

    #[test]
    fn unresolved_materialization_guard_fails_only_its_dedicated_runtime_path() {
        let store = Arc::new(FakeMaterializationStore {
            failures: AtomicUsize::new(0),
        });
        let service = super::MaterializationPersistenceService::new(
            Arc::clone(&store) as Arc<dyn MaterializationUnitOfWork>,
            FakeClock,
        );
        assert_eq!(
            block_on_ready(service.observe(*materialization_transition().command())).unwrap(),
            MaterializationObservation::Absent
        );
        assert_eq!(service.now().unwrap().unix_seconds(), 1_700_000_000);
        let guard = block_on_ready(service.claim_new(materialization_transition())).unwrap();
        drop(guard);
        assert_eq!(store.failures.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn startup_classifier_runs_non_overlapping_claimed_then_recovery_passes() {
        let store = Arc::new(FakeStartupClassifier {
            requests: Mutex::new(Vec::new()),
        });
        let service = StartupMutationClassificationService::new(
            Arc::clone(&store) as Arc<dyn StartupMutationClassifierPort>
        );
        let result = block_on_ready(
            service.classify_with_clock(Arc::new(ContinueSqliteControl), &FakeClock),
        )
        .unwrap();
        assert_eq!(result.recovery_required_command_count(), 2);
        assert_eq!(
            *store.requests.lock().unwrap(),
            vec![
                (StartupMutationState::Claimed, None),
                (StartupMutationState::Claimed, Some(fixed_command_id(0x88))),
                (StartupMutationState::RecoveryRequired, None),
            ]
        );
    }
}
