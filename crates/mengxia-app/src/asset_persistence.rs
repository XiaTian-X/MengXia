use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use mengxia_ports::{
    AssetStoreError, AssetUnitOfWork, ExternalClaimOutcome, ExternalIngestClaim,
    ExternalIngestCompletion, ExternalIngestDisposition, MutationOutcome,
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

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::{Context, Poll, Waker};

    use mengxia_ports::{
        ASSET_INGEST_COPY_V1, AssetPortFuture, Command, CommandBinding, CreateAssetRevisionCommand,
        ExternalClaimOutcome, ExternalDispositionOutcome, ExternalIngestClaim,
        ExternalIngestCompletion, ExternalIngestDisposition, MutationOutcome,
        RecordManagedLocationCommand,
    };
    use mengxia_types::{Id, Sha256Digest};

    use super::{
        AssetIdentitySource, AssetPersistenceService, AssetStoreError, AssetUnitOfWork, Clock,
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
}
