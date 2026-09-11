//! MengXia daemon composition root.

#![forbid(unsafe_code)]

use std::env;
use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant as StdInstant};

use mengxia_app::{
    AssetMetadataCommandService, AssetQueryService, CoreAvailability as AppCoreAvailability,
    CoreLiveness as AppCoreLiveness, CoreReadiness as AppCoreReadiness, CreativeCommandService,
    CreativeQueryService, CustodyObservation as AppCustodyObservation, IngestAdmissionLimits,
    IngestAssetCopyRequest as AppIngestRequest, IngestAssetCopyService, IngestAssetExecutionError,
    IngestRetry, LibraryConfigDocument, LibraryConfigKey, LibraryHealthInput, LibraryHealthState,
    LocalSecurityBaseline as AppLocalSecurityBaseline, MaterializeAssetFailure,
    MaterializeAssetRequest, MaterializeAssetService,
    ReadinessBlockReason as AppReadinessBlockReason, StartupMutationClassification,
    StartupMutationClassificationService, Task008RuntimeConfig, Task009RuntimeConfig,
    VerificationReportOwner, VerificationService,
};
#[cfg(test)]
use mengxia_core_proto::serve_handshake;
use mengxia_core_proto::{
    CoreRequest, CoreResponse, DecodeDepth, HandshakeLimits, IngestMode, OperationLimits,
    RetryAction, ServerNegotiation, TASK_009_MIN_OPERATION_DECODE_DEPTH, core_request,
    core_response, core_response_encoded_len, operation_error_response, read_core_request,
    serve_daemon_handshake, validate_core_request_for_minor, write_core_response,
};
use mengxia_domain::{
    Asset, AssetKind, AssetLifecycle, AssetRevision, ContentKind, LocationCustody,
    LocationDurability, LocationLifecycle, LogicalName, PositiveRatio, Project, ProjectName,
    ProjectSpecification, Representation, RepresentationPurpose, Resolution, Resource,
    ResourceKind, RevisionCustody, Subject, SubjectKind, SubjectName, Take, TakeReason,
    TakeTransition, WorkCode, WorkItem, WorkKind, WorkRevision, WorkSpecification,
};
use mengxia_framing::FrameLimit;
use mengxia_platform_fs::{
    AuthorityError, bind_runtime_endpoint, read_library_config, validate_runtime_endpoint_path,
};
use mengxia_ports::{
    AssetQueryPort, AssetRevisionMemberInput, AssetRevisionRepresentationInput,
    AssetRevisionResourceInput, AssetStoreError, AssetUnitOfWork, Command as PersistedCommand,
    CommandResult, CreativeUnitOfWork, IngestControl, IngestDirective, IngestStop, IntegrityIssue,
    IntegrityIssueKind, IntegrityObjectId, IntegrityObjectKind, IntegrityRemediation,
    IntegritySeverity, InterruptibleSqliteControl, MaterializationStoragePort,
    MaterializationUnitOfWork, SqliteInterrupt, SqliteInterruptControlError,
    StartupMutationClassifierPort, VerificationMode, VerificationReportIdentity,
    VersionedResultPayload,
};
use mengxia_storage_local::{
    BlobConfigSource, BlobIngestState, BlobStorageConfig, LocalBlobStorage,
    ResolvedBlobStorageConfig,
};
use mengxia_store_sqlite::{
    ConfigSource, OpenedLibrary, ResolvedStoreConfig, SqliteAssetStoreHandle, StoreConfig,
    StoreError,
};
use mengxia_types::{ErrorCode, Id, Sha256Digest};
use tokio::io::AsyncReadExt as _;
use tokio::net::UnixListener;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinSet;

const STARTUP_LOCAL_CLASSIFICATION_TIMEOUT: Duration = Duration::from_millis(300_000);
const MAX_QUERY_OPERATION_TIMEOUT: Duration = Duration::from_millis(86_400_000);
const TASK_009_MAX_RESPONSE_BYTES: usize = 1_048_576;
// A Project or Work row may carry up to 256 KiB of canonical JSON. Three
// maximally sized rows plus all bounded scalar/ID/protobuf overhead remain below
// the one-MiB response contract, while four JSON payloads alone can reach it.
const TASK_009_LARGE_ROW_PAGE_MAX: u32 = 3;

const HELP: &str = "mengxiad serve [--library-root PATH] [--blob-root PATH] [--client-endpoint PATH]\n  [--max-frame-bytes ASCII_U64] [--max-decode-depth ASCII_U32]\n  [--client-handshake-timeout-ms ASCII_U64] [--max-pending-handshakes ASCII_U32]\n  [--max-client-sessions ASCII_U32] [--max-ingest-operation-timeout-ms ASCII_U64]\n  [--max-verify-operation-timeout-ms ASCII_U64]\n  [--max-materialize-operation-timeout-ms ASCII_U64]\n  [--max-metadata-operation-timeout-ms ASCII_U64] [--log-level LEVEL]\n  [--ingest-shutdown-timeout-ms ASCII_U64]\n";

#[derive(Clone, Copy)]
struct HealthBaseline {
    staging_orphan_count: u16,
    staging_orphan_bytes: u64,
    local_backend_matches: bool,
}

enum DaemonTaskCompletion {
    Session(Result<(), ErrorCode>),
    StartupClassification(Result<StartupMutationClassification, StartupClassificationFailure>),
}

enum StartupClassificationFailure {
    Deadline,
    Store(AssetStoreError),
}

fn main() -> ExitCode {
    match parse_command(env::args_os().skip(1).collect()) {
        Ok(Command::Help) => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Command::Serve(cli)) => match resolve(*cli) {
            Ok(config) => run(config),
            Err(code) => fail(code, 2),
        },
        Err(code) => fail(code, 2),
    }
}

fn run(config: DaemonConfig) -> ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return fail(ErrorCode::InternalError, 1),
    };
    match runtime.block_on(serve(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => fail(code, 1),
    }
}

async fn serve(config: DaemonConfig) -> Result<(), ErrorCode> {
    let task_008 = config.task_008;
    let task_009 = config.task_009;
    let opened = OpenedLibrary::open_or_bootstrap(&config.store).map_err(StoreError::code)?;
    let identity = opened.identity();
    let authority = opened
        .authorize_blob_root(config.blob.blob_root_request())
        .map_err(|error| error.code())?;
    let execution_capacity = config
        .blob
        .storage_io_concurrency()
        .min(config.blob.hash_concurrency())
        .min(config.blob.max_concurrent_ingests());
    let (storage, startup) =
        LocalBlobStorage::start(config.blob, authority).map_err(|error| error.code())?;
    if startup.ingest_state() == BlobIngestState::OrphanReconciliationRequired {
        eprintln!(
            "MENGXIA_STORAGE_STATUS state=ORPHAN_RECONCILIATION_REQUIRED orphan_count={} orphan_bytes={}",
            startup.staging_orphan_count(),
            startup.staging_orphan_bytes()
        );
    }
    let store = Arc::new(opened.asset_store_handle());
    let local_backend_matches = match store
        .validate_local_managed_backend(startup.backend_id())
        .await
    {
        Ok(()) => true,
        Err(AssetStoreError::StorageConfiguration) => false,
        Err(error) => {
            let _ = storage.shutdown();
            let _ = opened.shutdown();
            return Err(error.error_code());
        }
    };
    let health_baseline = HealthBaseline {
        staging_orphan_count: startup.staging_orphan_count(),
        staging_orphan_bytes: startup.staging_orphan_bytes(),
        local_backend_matches,
    };
    let health = Arc::new(RwLock::new(health_state(
        health_baseline,
        AppReadinessBlockReason::LocalRecoveryPending,
        false,
        0,
    )?));
    let classifier = StartupMutationClassificationService::new(
        Arc::clone(&store) as Arc<dyn StartupMutationClassifierPort>
    );
    let query_service = Arc::new(
        AssetQueryService::new(Arc::clone(&store), identity.library_id_bytes())
            .map_err(|error| error.error_code())?,
    );
    let creative_command_service = Arc::new(CreativeCommandService::new(
        Arc::clone(&store) as Arc<dyn CreativeUnitOfWork>
    ));
    let asset_metadata_command_service = Arc::new(AssetMetadataCommandService::new(Arc::clone(
        &store,
    )
        as Arc<dyn AssetUnitOfWork>));
    let creative_query_service = Arc::new(
        CreativeQueryService::new(Arc::clone(&store), identity.library_id_bytes())
            .map_err(|error| error.error_code())?,
    );
    let storage = Arc::new(storage);
    let verification_service = Arc::new(VerificationService::new(
        Arc::clone(&store),
        Arc::clone(&storage),
        VerificationReportOwner::new(identity.library_id_bytes())
            .map_err(|error| error.error_code())?,
    ));
    let materialize_service = Arc::new(MaterializeAssetService::new(
        Arc::clone(&store) as Arc<dyn AssetQueryPort>,
        Arc::clone(&store) as Arc<dyn MaterializationUnitOfWork>,
        Arc::clone(&storage) as Arc<dyn MaterializationStoragePort>,
        startup.backend_id().to_owned(),
    )?);
    let service = Arc::new(IngestAssetCopyService::new(
        Arc::clone(&store) as Arc<dyn AssetUnitOfWork>,
        Arc::clone(&storage),
        IngestAdmissionLimits::new(config.max_sessions, execution_capacity)
            .ok_or(ErrorCode::StorageConfigurationError)?,
    ));
    let endpoint = match bind_runtime_endpoint(
        &config.endpoint,
        identity.library_id_bytes(),
        identity.owner_uid(),
    ) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            let primary = authority_code(error);
            drop(classifier);
            drop(query_service);
            drop(creative_query_service);
            drop(asset_metadata_command_service);
            drop(creative_command_service);
            drop(verification_service);
            drop(materialize_service);
            drop(service);
            drop(store);
            let _ = take_last_owner(storage).shutdown();
            let _ = opened.shutdown();
            return Err(primary);
        }
    };
    let std_listener = match endpoint.try_clone_listener() {
        Ok(listener) => listener,
        Err(error) => {
            let primary = authority_code(error);
            let _ = endpoint.cleanup();
            drop(classifier);
            drop(query_service);
            drop(creative_query_service);
            drop(asset_metadata_command_service);
            drop(creative_command_service);
            drop(verification_service);
            drop(materialize_service);
            drop(service);
            drop(store);
            let _ = take_last_owner(storage).shutdown();
            let _ = opened.shutdown();
            return Err(primary);
        }
    };
    let listener = match UnixListener::from_std(std_listener) {
        Ok(listener) => listener,
        Err(_) => {
            let _ = endpoint.cleanup();
            drop(classifier);
            drop(query_service);
            drop(creative_query_service);
            drop(asset_metadata_command_service);
            drop(creative_command_service);
            drop(verification_service);
            drop(materialize_service);
            drop(service);
            drop(store);
            let _ = take_last_owner(storage).shutdown();
            let _ = opened.shutdown();
            return Err(ErrorCode::StorageIoError);
        }
    };

    let admission = Arc::new(Semaphore::new(config.max_pending));
    let sessions = Arc::new(Semaphore::new(config.max_sessions));
    let cancelling = Arc::new(AtomicBool::new(false));
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    let startup_control = Arc::new(StartupSqliteControl::new());
    let mut tasks = JoinSet::new();
    let classifier_control = Arc::clone(&startup_control);
    tasks.spawn(async move {
        let classification = classifier
            .classify(Arc::clone(&classifier_control) as Arc<dyn InterruptibleSqliteControl>);
        tokio::pin!(classification);
        let result = tokio::select! {
            result = &mut classification => result.map_err(StartupClassificationFailure::Store),
            () = tokio::time::sleep(STARTUP_LOCAL_CLASSIFICATION_TIMEOUT) => {
                let stop_result = classifier_control.stop(IngestStop::DeadlineReached);
                let joined = classification.await;
                match (stop_result, joined) {
                    (Ok(_), Err(AssetStoreError::DeadlineExceeded)) => {
                        Err(StartupClassificationFailure::Deadline)
                    }
                    (Ok(_), result) => result.map_err(StartupClassificationFailure::Store),
                    (Err(()), _) => Err(StartupClassificationFailure::Store(
                        AssetStoreError::Internal,
                    )),
                }
            }
        };
        DaemonTaskCompletion::StartupClassification(result)
    });
    let mut primary = None;
    let signal = shutdown_signal();
    tokio::pin!(signal);
    loop {
        tokio::select! {
            signal_result = &mut signal => {
                cancelling.store(true, Ordering::Release);
                let _ = shutdown_sender.send(true);
                let _ = startup_control.stop(IngestStop::Cancelled);
                if signal_result.is_err() {
                    primary = Some(ErrorCode::InternalError);
                }
                break;
            }
            accepted = listener.accept() => {
                match accepted {
                    Ok((stream, _)) => {
                        let Ok(permit) = admission.clone().try_acquire_owned() else {
                            drop(stream);
                            continue;
                        };
                        let limits = config.limits;
                        let operation_limits = config.operation_limits;
                        let max_operation_timeout = config.max_operation_timeout;
                        let max_verify_operation_timeout =
                            task_008.max_verify_operation_timeout();
                        let max_materialize_operation_timeout =
                            task_008.max_materialize_operation_timeout();
                        let max_metadata_operation_timeout =
                            task_009.max_metadata_operation_timeout();
                        let owner_uid = identity.owner_uid();
                        let sessions = Arc::clone(&sessions);
                        let service = Arc::clone(&service);
                        let query_service = Arc::clone(&query_service);
                        let verification_service = Arc::clone(&verification_service);
                        let materialize_service = Arc::clone(&materialize_service);
                        let creative_command_service = Arc::clone(&creative_command_service);
                        let asset_metadata_command_service =
                            Arc::clone(&asset_metadata_command_service);
                        let creative_query_service = Arc::clone(&creative_query_service);
                        let cancelling = Arc::clone(&cancelling);
                        let shutdown = shutdown_receiver.clone();
                        let health = Arc::clone(&health);
                        tasks.spawn(async move {
                            DaemonTaskCompletion::Session(serve_connection(
                                stream, owner_uid, limits, operation_limits,
                                max_operation_timeout, max_verify_operation_timeout,
                                max_materialize_operation_timeout, max_metadata_operation_timeout,
                                sessions, service,
                                query_service, verification_service, materialize_service,
                                asset_metadata_command_service, creative_command_service,
                                creative_query_service,
                                cancelling, shutdown, health, permit,
                            ).await)
                        });
                    }
                    Err(_) => {
                        primary = Some(ErrorCode::IpcTransportError);
                        break;
                    }
                }
            }
            completed = tasks.join_next(), if !tasks.is_empty() => {
                match completed {
                    Some(Ok(DaemonTaskCompletion::Session(Ok(())))) => {}
                    Some(Ok(DaemonTaskCompletion::Session(Err(code)))) => {
                        primary.get_or_insert(code);
                        cancelling.store(true, Ordering::Release);
                        let _ = shutdown_sender.send(true);
                        break;
                    }
                    Some(Ok(DaemonTaskCompletion::StartupClassification(Ok(result)))) => {
                        let update = health_state(
                            health_baseline,
                            AppReadinessBlockReason::None,
                            true,
                            result.recovery_required_command_count(),
                        )
                        .and_then(|next| publish_health(&health, next));
                        if let Err(code) = update {
                            primary.get_or_insert(code);
                            cancelling.store(true, Ordering::Release);
                            let _ = shutdown_sender.send(true);
                            break;
                        }
                    }
                    Some(Ok(DaemonTaskCompletion::StartupClassification(Err(error)))) => {
                        if startup_failure_is_status_only(&error) {
                            let update = health_state(
                                health_baseline,
                                AppReadinessBlockReason::LocalRecoveryOperatorAction,
                                false,
                                0,
                            )
                            .and_then(|next| publish_health(&health, next));
                            if let Err(code) = update {
                                primary.get_or_insert(code);
                                cancelling.store(true, Ordering::Release);
                                break;
                            }
                        } else {
                            primary.get_or_insert(startup_failure_code(error));
                            cancelling.store(true, Ordering::Release);
                            let _ = shutdown_sender.send(true);
                            break;
                        }
                    }
                    Some(Err(_)) => {
                        primary.get_or_insert(ErrorCode::InternalError);
                        cancelling.store(true, Ordering::Release);
                        let _ = shutdown_sender.send(true);
                        break;
                    }
                    None => {}
                }
            }
        }
    }
    drop(listener);

    cancelling.store(true, Ordering::Release);
    let _ = shutdown_sender.send(true);
    let _ = startup_control.stop(IngestStop::Cancelled);
    let join_deadline = tokio::time::Instant::now() + config.shutdown_timeout;
    while !tasks.is_empty() {
        match tokio::time::timeout_at(join_deadline, tasks.join_next()).await {
            Ok(Some(Ok(DaemonTaskCompletion::Session(Ok(()))))) => {}
            Ok(Some(Ok(DaemonTaskCompletion::Session(Err(code))))) => {
                primary.get_or_insert(code);
            }
            Ok(Some(Ok(DaemonTaskCompletion::StartupClassification(Ok(_))))) => {}
            Ok(Some(Ok(DaemonTaskCompletion::StartupClassification(Err(error))))) => {
                if !matches!(
                    error,
                    StartupClassificationFailure::Store(AssetStoreError::OperationCancelled)
                ) && !startup_failure_is_status_only(&error)
                {
                    primary.get_or_insert(startup_failure_code(error));
                }
            }
            Ok(Some(Err(_))) => {
                primary.get_or_insert(ErrorCode::InternalError);
            }
            Ok(None) => break,
            Err(_) => fatal_shutdown(),
        }
    }

    if let Err(error) = endpoint.cleanup() {
        primary.get_or_insert(authority_code(error));
    }
    drop(service);
    drop(query_service);
    drop(creative_query_service);
    drop(asset_metadata_command_service);
    drop(creative_command_service);
    drop(verification_service);
    drop(materialize_service);
    drop(store);
    if let Err(error) = take_last_owner(storage).shutdown() {
        primary.get_or_insert(error.code());
    }
    if let Err(error) = opened.shutdown() {
        primary.get_or_insert(error.code());
    }
    primary.map_or(Ok(()), Err)
}

fn health_state(
    baseline: HealthBaseline,
    readiness_block_reason: AppReadinessBlockReason,
    recovery_observation_available: bool,
    recovery_required_command_count: u64,
) -> Result<LibraryHealthState, ErrorCode> {
    LibraryHealthState::new(LibraryHealthInput {
        liveness: AppCoreLiveness::Live,
        readiness_block_reason,
        local_security_baseline: AppLocalSecurityBaseline::Verified,
        custody_observation: AppCustodyObservation::Unassessed,
        staging_orphan_count: baseline.staging_orphan_count,
        staging_orphan_bytes: baseline.staging_orphan_bytes,
        local_backend_matches: baseline.local_backend_matches,
        observability_degraded: false,
        recovery_observation_available,
        recovery_required_command_count,
        custody_degraded: false,
        dependency_degraded: false,
    })
}

fn startup_failure_is_status_only(error: &StartupClassificationFailure) -> bool {
    matches!(
        error,
        StartupClassificationFailure::Deadline
            | StartupClassificationFailure::Store(
                AssetStoreError::StorageConfiguration
                    | AssetStoreError::StorageBusy
                    | AssetStoreError::Backpressure
                    | AssetStoreError::IdGenerationUnavailable
            )
    )
}

fn startup_failure_code(error: StartupClassificationFailure) -> ErrorCode {
    match error {
        StartupClassificationFailure::Deadline => ErrorCode::DeadlineExceeded,
        StartupClassificationFailure::Store(error) => error.error_code(),
    }
}

fn read_health(health: &RwLock<LibraryHealthState>) -> Result<LibraryHealthState, ErrorCode> {
    health
        .read()
        .map(|state| *state)
        .map_err(|_| ErrorCode::InternalError)
}

fn publish_health(
    health: &RwLock<LibraryHealthState>,
    next: LibraryHealthState,
) -> Result<(), ErrorCode> {
    *health.write().map_err(|_| ErrorCode::InternalError)? = next;
    Ok(())
}

fn status_response(state: LibraryHealthState) -> CoreResponse {
    let liveness = match state.liveness() {
        AppCoreLiveness::Live => mengxia_core_proto::CoreLiveness::Live,
        AppCoreLiveness::Stopping => mengxia_core_proto::CoreLiveness::Stopping,
        AppCoreLiveness::Failed => mengxia_core_proto::CoreLiveness::Failed,
    };
    let readiness = match state.readiness() {
        AppCoreReadiness::NotReady => mengxia_core_proto::CoreReadiness::NotReady,
        AppCoreReadiness::Ready => mengxia_core_proto::CoreReadiness::Ready,
    };
    let availability = match state.availability() {
        AppCoreAvailability::Full => mengxia_core_proto::CoreAvailability::Full,
        AppCoreAvailability::ReadOnlyCustody => {
            mengxia_core_proto::CoreAvailability::ReadOnlyCustody
        }
        AppCoreAvailability::DegradedCustody => {
            mengxia_core_proto::CoreAvailability::DegradedCustody
        }
        AppCoreAvailability::DegradedDependency => {
            mengxia_core_proto::CoreAvailability::DegradedDependency
        }
    };
    let local_security_baseline = match state.local_security_baseline() {
        AppLocalSecurityBaseline::Verified => mengxia_core_proto::LocalSecurityBaseline::Verified,
        AppLocalSecurityBaseline::Unsafe => mengxia_core_proto::LocalSecurityBaseline::Unsafe,
        AppLocalSecurityBaseline::Unavailable => {
            mengxia_core_proto::LocalSecurityBaseline::Unavailable
        }
    };
    let custody_observation = match state.custody_observation() {
        AppCustodyObservation::Unassessed => mengxia_core_proto::CustodyObservation::Unassessed,
        AppCustodyObservation::NormalVerified => {
            mengxia_core_proto::CustodyObservation::NormalVerified
        }
        AppCustodyObservation::DeepVerified => mengxia_core_proto::CustodyObservation::DeepVerified,
    };
    let readiness_block_reason = match state.readiness_block_reason() {
        AppReadinessBlockReason::None => mengxia_core_proto::ReadinessBlockReason::None,
        AppReadinessBlockReason::LocalRecoveryPending => {
            mengxia_core_proto::ReadinessBlockReason::LocalRecoveryPending
        }
        AppReadinessBlockReason::LocalRecoveryOperatorAction => {
            mengxia_core_proto::ReadinessBlockReason::LocalRecoveryOperatorAction
        }
    };
    CoreResponse {
        response: Some(core_response::Response::GetLibraryStatus(
            mengxia_core_proto::GetLibraryStatusResult {
                liveness: liveness as i32,
                readiness: readiness as i32,
                availability: availability as i32,
                local_security_baseline: local_security_baseline as i32,
                can_read_metadata: state.can_read_metadata(),
                can_verify: state.can_verify(),
                can_ingest: state.can_ingest(),
                can_materialize: state.can_materialize(),
                staging_orphan_count: u32::from(state.staging_orphan_count()),
                staging_orphan_bytes: state.staging_orphan_bytes(),
                local_backend_matches: state.local_backend_matches(),
                observability_degraded: state.observability_degraded(),
                recovery_observation_available: state.recovery_observation_available(),
                recovery_required_command_count: state.recovery_required_command_count(),
                custody_observation: custody_observation as i32,
                readiness_block_reason: readiness_block_reason as i32,
            },
        )),
    }
}

fn take_last_owner<T>(owner: Arc<T>) -> T {
    match Arc::try_unwrap(owner) {
        Ok(owner) => owner,
        Err(_leaked_owner) => fatal_shutdown(),
    }
}

fn fatal_shutdown() -> ! {
    // The bounded shutdown contract forbids unwinding through storage/store
    // owners whose Drop implementations may wait for blocked workers.
    std::process::exit(1)
}

struct SessionControl {
    deadline: StdInstant,
    cancelling: Arc<AtomicBool>,
    peer_stopped: Arc<AtomicBool>,
}

struct StartupSqliteControl {
    state: Mutex<StartupSqliteControlState>,
}

struct StartupSqliteControlState {
    stop: Option<IngestStop>,
    interrupt: Option<Box<dyn SqliteInterrupt>>,
}

impl StartupSqliteControl {
    fn new() -> Self {
        Self {
            state: Mutex::new(StartupSqliteControlState {
                stop: None,
                interrupt: None,
            }),
        }
    }

    fn stop(&self, stop: IngestStop) -> Result<bool, ()> {
        let mut state = self.state.lock().map_err(|_| ())?;
        if state.stop.is_some() {
            return Ok(false);
        }
        state.stop = Some(stop);
        if let Some(interrupt) = state.interrupt.as_ref() {
            interrupt.interrupt();
        }
        Ok(true)
    }
}

impl IngestControl for StartupSqliteControl {
    fn checkpoint(&self) -> IngestDirective {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => panic!("startup SQLite control state poisoned"),
        };
        state
            .stop
            .map_or(IngestDirective::Continue, IngestDirective::Stop)
    }
}

impl InterruptibleSqliteControl for StartupSqliteControl {
    fn register_interrupt(
        &self,
        interrupt: Box<dyn SqliteInterrupt>,
    ) -> Result<IngestDirective, SqliteInterruptControlError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SqliteInterruptControlError::StateUnavailable)?;
        if state.interrupt.is_some() {
            return Err(SqliteInterruptControlError::RegistrationConflict);
        }
        if let Some(stop) = state.stop {
            return Ok(IngestDirective::Stop(stop));
        }
        state.interrupt = Some(interrupt);
        Ok(IngestDirective::Continue)
    }

    fn clear_interrupt(&self) -> Result<(), SqliteInterruptControlError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SqliteInterruptControlError::StateUnavailable)?;
        if state.interrupt.take().is_none() {
            return Err(SqliteInterruptControlError::RegistrationConflict);
        }
        Ok(())
    }
}

#[cfg(test)]
struct CrashCheckpointControl {
    session: SessionControl,
    selected: usize,
    observed: AtomicUsize,
    ready: PathBuf,
}

impl IngestControl for SessionControl {
    fn checkpoint(&self) -> IngestDirective {
        if self.cancelling.load(Ordering::Acquire) || self.peer_stopped.load(Ordering::Acquire) {
            IngestDirective::Stop(IngestStop::Cancelled)
        } else if StdInstant::now() >= self.deadline {
            IngestDirective::Stop(IngestStop::DeadlineReached)
        } else {
            IngestDirective::Continue
        }
    }
}

#[cfg(test)]
impl IngestControl for CrashCheckpointControl {
    fn checkpoint(&self) -> IngestDirective {
        let observed = self.observed.fetch_add(1, Ordering::AcqRel) + 1;
        if observed == self.selected {
            publish_crash_ready(&self.ready);
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        }
        self.session.checkpoint()
    }
}

fn session_control(
    deadline: StdInstant,
    cancelling: Arc<AtomicBool>,
    peer_stopped: Arc<AtomicBool>,
) -> Arc<dyn IngestControl> {
    let session = SessionControl {
        deadline,
        cancelling,
        peer_stopped,
    };
    #[cfg(test)]
    if let (Some(selected), Some(ready)) = (
        env::var_os("MENGXIA_TASK007_CRASH_CHECKPOINT"),
        env::var_os("MENGXIA_TASK007_CRASH_READY"),
    ) && let Ok(selected) = selected.to_string_lossy().parse::<usize>()
    {
        return Arc::new(CrashCheckpointControl {
            session,
            selected,
            observed: AtomicUsize::new(0),
            ready: PathBuf::from(ready),
        });
    }
    Arc::new(session)
}

#[cfg(test)]
fn publish_crash_ready(path: &std::path::Path) {
    use std::fs::File;

    let file = File::create(path).expect("create TASK-007 crash acknowledgement");
    file.sync_all()
        .expect("sync TASK-007 crash acknowledgement");
}

#[cfg(test)]
fn response_crash_boundary(boundary: &str) {
    if env::var_os("MENGXIA_TASK007_RESPONSE_CRASH_BOUNDARY").as_deref()
        != Some(OsStr::new(boundary))
    {
        return;
    }
    let ready = PathBuf::from(
        env::var_os("MENGXIA_TASK007_CRASH_READY").expect("TASK-007 response crash ready path"),
    );
    publish_crash_ready(&ready);
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

#[allow(clippy::too_many_arguments)]
async fn serve_connection(
    mut stream: tokio::net::UnixStream,
    owner_uid: u32,
    handshake_limits: HandshakeLimits,
    operation_limits: OperationLimits,
    max_operation_timeout: Duration,
    max_verify_operation_timeout: Duration,
    max_materialize_operation_timeout: Duration,
    max_metadata_operation_timeout: Duration,
    sessions: Arc<Semaphore>,
    service: Arc<IngestAssetCopyService<LocalBlobStorage>>,
    query_service: Arc<AssetQueryService<SqliteAssetStoreHandle>>,
    verification_service: Arc<VerificationService<SqliteAssetStoreHandle, LocalBlobStorage>>,
    materialize_service: Arc<MaterializeAssetService>,
    asset_metadata_command_service: Arc<AssetMetadataCommandService>,
    creative_command_service: Arc<CreativeCommandService>,
    creative_query_service: Arc<CreativeQueryService<SqliteAssetStoreHandle>>,
    cancelling: Arc<AtomicBool>,
    shutdown: watch::Receiver<bool>,
    health: Arc<RwLock<LibraryHealthState>>,
    handshake_permit: OwnedSemaphorePermit,
) -> Result<(), ErrorCode> {
    let negotiation = match serve_daemon_handshake(&mut stream, owner_uid, handshake_limits).await {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    let session = match negotiation {
        ServerNegotiation::HandshakeOnly(_) => return Ok(()),
        ServerNegotiation::SingleCommand(session) => session,
    };
    let session_permit = match sessions.try_acquire_owned() {
        Ok(permit) => permit,
        Err(_) => {
            drop(handshake_permit);
            let response = operation_error_response(
                ErrorCode::Backpressure,
                RetryAction::SameCommand,
                session.correlation_id(),
            )
            .map_err(|error| error.code())?;
            let _ = write_core_response(
                &mut stream,
                &response,
                operation_limits,
                tokio::time::Instant::now() + handshake_limits.timeout(),
            )
            .await;
            return Ok(());
        }
    };
    drop(handshake_permit);
    let request = match read_core_request(
        &mut stream,
        operation_limits,
        tokio::time::Instant::now() + handshake_limits.timeout(),
    )
    .await
    {
        Ok(request) => request,
        Err(error) => {
            let retry = match error.code() {
                ErrorCode::ValidationError => RetryAction::None,
                ErrorCode::DeadlineExceeded => RetryAction::SameCommand,
                _ => return Ok(()),
            };
            let response = operation_error_response(error.code(), retry, session.correlation_id())
                .map_err(|value| value.code())?;
            let _ = write_core_response(
                &mut stream,
                &response,
                operation_limits,
                tokio::time::Instant::now() + handshake_limits.timeout(),
            )
            .await;
            return Ok(());
        }
    };
    if let Err(error) = validate_core_request_for_minor(&request, session.protocol_minor()) {
        let response =
            operation_error_response(error.code(), RetryAction::None, session.correlation_id())
                .map_err(|value| value.code())?;
        let _ = write_core_response(
            &mut stream,
            &response,
            operation_limits,
            tokio::time::Instant::now() + handshake_limits.timeout(),
        )
        .await;
        return Ok(());
    }
    let health_snapshot = read_health(&health)?;
    if matches!(
        &request.operation,
        Some(core_request::Operation::GetLibraryStatus(_))
    ) {
        let _ = write_core_response(
            &mut stream,
            &status_response(health_snapshot),
            operation_limits,
            tokio::time::Instant::now() + handshake_limits.timeout(),
        )
        .await;
        return Ok(());
    }
    if health_snapshot.readiness() != AppCoreReadiness::Ready {
        let response = operation_error_response(
            ErrorCode::Backpressure,
            RetryAction::FreshCommand,
            session.correlation_id(),
        )
        .map_err(|value| value.code())?;
        let _ = write_core_response(
            &mut stream,
            &response,
            operation_limits,
            tokio::time::Instant::now() + handshake_limits.timeout(),
        )
        .await;
        return Ok(());
    }
    let capability_allowed = match request.operation.as_ref() {
        Some(core_request::Operation::IngestAssetCopy(_)) => health_snapshot.can_ingest(),
        Some(core_request::Operation::MaterializeAsset(_)) => health_snapshot.can_materialize(),
        Some(core_request::Operation::VerifyLibrary(_)) => health_snapshot.can_verify(),
        Some(
            core_request::Operation::ListAssets(_)
            | core_request::Operation::InspectAsset(_)
            | core_request::Operation::ListIntegrityIssues(_)
            | core_request::Operation::ListProjects(_)
            | core_request::Operation::ListSubjects(_)
            | core_request::Operation::ListWork(_)
            | core_request::Operation::ListTakes(_),
        ) => health_snapshot.can_read_metadata(),
        Some(
            core_request::Operation::CreateAssetRevision(_)
            | core_request::Operation::RetireAsset(_)
            | core_request::Operation::RestoreAsset(_)
            | core_request::Operation::CreateProject(_)
            | core_request::Operation::ReviseProjectSpec(_)
            | core_request::Operation::CreateSubject(_)
            | core_request::Operation::CreateWorkItem(_)
            | core_request::Operation::ReviseWork(_)
            | core_request::Operation::CreateTake(_)
            | core_request::Operation::TransitionTake(_)
            | core_request::Operation::ReopenTake(_),
        ) => health_snapshot.can_mutate_metadata(),
        Some(core_request::Operation::GetLibraryStatus(_)) | None => false,
    };
    if !capability_allowed {
        let response = operation_error_response(
            ErrorCode::StorageConfigurationError,
            RetryAction::OperatorOrRuntimeAction,
            session.correlation_id(),
        )
        .map_err(|value| value.code())?;
        let _ = write_core_response(
            &mut stream,
            &response,
            operation_limits,
            tokio::time::Instant::now() + handshake_limits.timeout(),
        )
        .await;
        return Ok(());
    }
    match request.operation.as_ref() {
        Some(core_request::Operation::ListAssets(query)) => {
            return serve_list_assets(
                &mut stream,
                query,
                session.correlation_id(),
                operation_limits,
                handshake_limits.timeout(),
                session.protocol_minor(),
                query_service,
                cancelling,
                shutdown,
            )
            .await;
        }
        Some(
            core_request::Operation::CreateAssetRevision(_)
            | core_request::Operation::RetireAsset(_)
            | core_request::Operation::RestoreAsset(_),
        ) => {
            return serve_asset_metadata_mutation(
                &mut stream,
                &request,
                session.correlation_id(),
                operation_limits,
                handshake_limits.timeout(),
                max_metadata_operation_timeout,
                asset_metadata_command_service,
                cancelling,
                shutdown,
            )
            .await;
        }
        Some(
            core_request::Operation::CreateProject(_)
            | core_request::Operation::ReviseProjectSpec(_)
            | core_request::Operation::CreateSubject(_)
            | core_request::Operation::CreateWorkItem(_)
            | core_request::Operation::ReviseWork(_)
            | core_request::Operation::CreateTake(_)
            | core_request::Operation::TransitionTake(_)
            | core_request::Operation::ReopenTake(_),
        ) => {
            return serve_creative_mutation(
                &mut stream,
                &request,
                session.correlation_id(),
                operation_limits,
                handshake_limits.timeout(),
                max_metadata_operation_timeout,
                creative_command_service,
                cancelling,
                shutdown,
            )
            .await;
        }
        Some(core_request::Operation::InspectAsset(query)) => {
            return serve_inspect_asset(
                &mut stream,
                query,
                session.correlation_id(),
                operation_limits,
                handshake_limits.timeout(),
                session.protocol_minor(),
                query_service,
                cancelling,
                shutdown,
            )
            .await;
        }
        Some(core_request::Operation::VerifyLibrary(request)) => {
            return serve_verify_library(
                &mut stream,
                request,
                session.correlation_id(),
                operation_limits,
                handshake_limits.timeout(),
                max_verify_operation_timeout,
                verification_service,
                cancelling,
                shutdown,
                health,
            )
            .await;
        }
        Some(core_request::Operation::ListIntegrityIssues(request)) => {
            return serve_list_integrity_issues(
                &mut stream,
                request,
                session.correlation_id(),
                operation_limits,
                handshake_limits.timeout(),
                verification_service,
            )
            .await;
        }
        Some(core_request::Operation::MaterializeAsset(request)) => {
            return serve_materialize_asset(
                &mut stream,
                request,
                session.correlation_id(),
                operation_limits,
                handshake_limits.timeout(),
                max_materialize_operation_timeout,
                materialize_service,
                cancelling,
            )
            .await;
        }
        Some(
            core_request::Operation::ListProjects(_)
            | core_request::Operation::ListSubjects(_)
            | core_request::Operation::ListWork(_)
            | core_request::Operation::ListTakes(_),
        ) => {
            return serve_creative_query(
                &mut stream,
                &request,
                session.correlation_id(),
                operation_limits,
                handshake_limits.timeout(),
                max_metadata_operation_timeout,
                creative_query_service,
                cancelling,
                shutdown,
            )
            .await;
        }
        _ => {}
    }
    let (request, requested_timeout) = match decode_ingest_request(request, max_operation_timeout) {
        Ok(value) => value,
        Err(code) => {
            let response =
                operation_error_response(code, RetryAction::None, session.correlation_id())
                    .map_err(|value| value.code())?;
            let _ = write_core_response(
                &mut stream,
                &response,
                operation_limits,
                tokio::time::Instant::now() + handshake_limits.timeout(),
            )
            .await;
            return Ok(());
        }
    };
    let semantic_deadline = StdInstant::now() + requested_timeout;
    let transport_deadline = tokio::time::Instant::now() + requested_timeout;
    let peer_stopped = Arc::new(AtomicBool::new(false));
    let control = session_control(semantic_deadline, cancelling, Arc::clone(&peer_stopped));
    let runtime = tokio::runtime::Handle::current();
    let worker =
        tokio::task::spawn_blocking(move || runtime.block_on(service.execute(request, control)));
    let result = await_ingest_with_watcher(
        &mut stream,
        worker,
        transport_deadline,
        Arc::clone(&peer_stopped),
    )
    .await?;
    let response = match result {
        Ok(result) => CoreResponse {
            response: Some(core_response::Response::IngestAssetCopy(
                mengxia_core_proto::IngestAssetCopyResult {
                    asset_id: result.asset_id().to_string(),
                    asset_revision_id: result.asset_revision_id().to_string(),
                    representation_id: result.representation_id().to_string(),
                    resource_id: result.resource_id().to_string(),
                    location_id: result.location_id().to_string(),
                    blob_sha256: result.blob_digest().to_bytes().to_vec(),
                },
            )),
        },
        Err(IngestAssetExecutionError::Respond(failure)) => operation_error_response(
            failure.code(),
            retry_action(failure.retry()),
            session.correlation_id(),
        )
        .map_err(|error| error.code())?,
        Err(IngestAssetExecutionError::RuntimeFailed) => return Err(ErrorCode::InternalError),
    };
    #[cfg(test)]
    let response_is_success = matches!(
        response.response,
        Some(core_response::Response::IngestAssetCopy(_))
    );
    #[cfg(test)]
    response_crash_boundary(if response_is_success {
        "KILL-007-012"
    } else {
        "KILL-007-015"
    });
    let write_result = write_core_response(
        &mut stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + handshake_limits.timeout(),
    )
    .await;
    #[cfg(test)]
    if write_result.is_ok() {
        response_crash_boundary(if response_is_success {
            "KILL-007-013"
        } else {
            "KILL-007-016"
        });
    }
    let _ = write_result;
    drop(session_permit);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn serve_list_assets(
    stream: &mut tokio::net::UnixStream,
    request: &mengxia_core_proto::ListAssetsRequest,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
    protocol_minor: u32,
    service: Arc<AssetQueryService<SqliteAssetStoreHandle>>,
    cancelling: Arc<AtomicBool>,
    shutdown: watch::Receiver<bool>,
) -> Result<(), ErrorCode> {
    let timeout = match decode_query_timeout(request.operation_timeout_ms) {
        Ok(timeout) => timeout,
        Err(code) => {
            return write_query_validation_error(
                stream,
                code,
                correlation_id,
                operation_limits,
                response_timeout,
            )
            .await;
        }
    };
    let control = Arc::new(StartupSqliteControl::new());
    let future = service.list_assets_controlled(
        request.page_size,
        (!request.cursor.is_empty()).then_some(request.cursor.as_slice()),
        Arc::clone(&control) as Arc<dyn InterruptibleSqliteControl>,
    );
    let result = await_sqlite_query(
        stream,
        future,
        tokio::time::Instant::now() + timeout,
        Arc::clone(&control),
        cancelling,
        shutdown,
    )
    .await;
    let (response, fatal) = match result {
        Ok(result) => (
            CoreResponse {
                response: Some(core_response::Response::ListAssets(
                    mengxia_core_proto::ListAssetsResult {
                        snapshot_commit_sequence: result.page().snapshot_sequence(),
                        assets: result
                            .page()
                            .assets()
                            .iter()
                            .map(|summary| asset_summary_response(summary, protocol_minor))
                            .collect(),
                        next_cursor: result.next_cursor().map(|cursor| cursor.to_vec()),
                    },
                )),
            },
            None,
        ),
        Err(error) => (
            query_error_response(error, correlation_id)?,
            query_fatal_code(error),
        ),
    };
    let _ = write_core_response(
        stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + response_timeout,
    )
    .await;
    fatal.map_or(Ok(()), Err)
}

#[allow(clippy::too_many_arguments)]
async fn serve_inspect_asset(
    stream: &mut tokio::net::UnixStream,
    request: &mengxia_core_proto::InspectAssetRequest,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
    protocol_minor: u32,
    service: Arc<AssetQueryService<SqliteAssetStoreHandle>>,
    cancelling: Arc<AtomicBool>,
    shutdown: watch::Receiver<bool>,
) -> Result<(), ErrorCode> {
    let decoded = (|| {
        Ok((
            decode_query_timeout(request.operation_timeout_ms)?,
            parse_canonical_id::<Asset>(&request.asset_id)?,
            request
                .asset_revision_id
                .as_deref()
                .map(parse_canonical_id::<AssetRevision>)
                .transpose()?,
        ))
    })();
    let (timeout, asset_id, selected_revision_id) = match decoded {
        Ok(decoded) => decoded,
        Err(code) => {
            return write_query_validation_error(
                stream,
                code,
                correlation_id,
                operation_limits,
                response_timeout,
            )
            .await;
        }
    };
    let control = Arc::new(StartupSqliteControl::new());
    let future = service.inspect_asset_controlled(
        asset_id,
        selected_revision_id,
        request.page_size,
        (!request.cursor.is_empty()).then_some(request.cursor.as_slice()),
        Arc::clone(&control) as Arc<dyn InterruptibleSqliteControl>,
    );
    let result = await_sqlite_query(
        stream,
        future,
        tokio::time::Instant::now() + timeout,
        Arc::clone(&control),
        cancelling,
        shutdown,
    )
    .await;
    let (response, fatal) = match result {
        Ok(result) => {
            let page = result.page();
            (
                CoreResponse {
                    response: Some(core_response::Response::InspectAsset(
                        mengxia_core_proto::InspectAssetResult {
                            asset: Some(asset_summary_response(page.asset(), protocol_minor)),
                            asset_revision_id: page.selected_revision_id().to_string(),
                            revision_sequence: page.revision_sequence(),
                            content_kind: page.content_kind().as_str().to_owned(),
                            custody: revision_custody_response(page.custody()) as i32,
                            parent_revision_ids: page
                                .parent_revision_ids()
                                .iter()
                                .map(ToString::to_string)
                                .collect(),
                            members: page.members().iter().map(asset_member_response).collect(),
                            next_cursor: result.next_cursor().map(|cursor| cursor.to_vec()),
                        },
                    )),
                },
                None,
            )
        }
        Err(error) => (
            query_error_response(error, correlation_id)?,
            query_fatal_code(error),
        ),
    };
    let _ = write_core_response(
        stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + response_timeout,
    )
    .await;
    fatal.map_or(Ok(()), Err)
}

#[allow(clippy::too_many_arguments)]
async fn serve_creative_query(
    stream: &mut tokio::net::UnixStream,
    request: &CoreRequest,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
    maximum_timeout: Duration,
    service: Arc<CreativeQueryService<SqliteAssetStoreHandle>>,
    cancelling: Arc<AtomicBool>,
    shutdown: watch::Receiver<bool>,
) -> Result<(), ErrorCode> {
    let control = Arc::new(StartupSqliteControl::new());
    let result = match request.operation.as_ref() {
        Some(core_request::Operation::ListProjects(request)) => {
            let timeout = match decode_bounded_operation_timeout(
                request.operation_timeout_ms,
                maximum_timeout,
            ) {
                Ok(timeout) => timeout,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.list_projects(
                    request.page_size.min(TASK_009_LARGE_ROW_PAGE_MAX),
                    (!request.cursor.is_empty()).then_some(request.cursor.as_slice()),
                    Arc::clone(&control) as Arc<dyn InterruptibleSqliteControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|page| CoreResponse {
                response: Some(core_response::Response::ListProjects(
                    mengxia_core_proto::ListProjectsResult {
                        snapshot_commit_sequence: page.page().snapshot_endpoint(),
                        projects: page.page().items().iter().map(project_response).collect(),
                        next_cursor: page.next_cursor().map(|cursor| cursor.to_vec()),
                    },
                )),
            })
        }
        Some(core_request::Operation::ListSubjects(request)) => {
            let timeout = match decode_bounded_operation_timeout(
                request.operation_timeout_ms,
                maximum_timeout,
            ) {
                Ok(timeout) => timeout,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.list_subjects(
                    request.page_size,
                    (!request.cursor.is_empty()).then_some(request.cursor.as_slice()),
                    Arc::clone(&control) as Arc<dyn InterruptibleSqliteControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|page| CoreResponse {
                response: Some(core_response::Response::ListSubjects(
                    mengxia_core_proto::ListSubjectsResult {
                        snapshot_commit_sequence: page.page().snapshot_endpoint(),
                        subjects: page.page().items().iter().map(subject_response).collect(),
                        next_cursor: page.next_cursor().map(|cursor| cursor.to_vec()),
                    },
                )),
            })
        }
        Some(core_request::Operation::ListWork(request)) => {
            let decoded = (|| {
                Ok((
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                    parse_canonical_id(&request.project_id)?,
                ))
            })();
            let (timeout, project_id) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.list_work(
                    project_id,
                    request.page_size.min(TASK_009_LARGE_ROW_PAGE_MAX),
                    (!request.cursor.is_empty()).then_some(request.cursor.as_slice()),
                    Arc::clone(&control) as Arc<dyn InterruptibleSqliteControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|page| CoreResponse {
                response: Some(core_response::Response::ListWork(
                    mengxia_core_proto::ListWorkResult {
                        snapshot_commit_sequence: page.page().snapshot_endpoint(),
                        work_items: page.page().items().iter().map(work_response).collect(),
                        next_cursor: page.next_cursor().map(|cursor| cursor.to_vec()),
                    },
                )),
            })
        }
        Some(core_request::Operation::ListTakes(request)) => {
            let decoded = (|| {
                Ok((
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                    parse_canonical_id(&request.project_id)?,
                    parse_canonical_id(&request.work_item_id)?,
                    parse_canonical_id(&request.work_revision_id)?,
                ))
            })();
            let (timeout, project_id, work_item_id, work_revision_id) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.list_takes(
                    project_id,
                    work_item_id,
                    work_revision_id,
                    request.page_size,
                    (!request.cursor.is_empty()).then_some(request.cursor.as_slice()),
                    Arc::clone(&control) as Arc<dyn InterruptibleSqliteControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .and_then(|page| {
                Ok(CoreResponse {
                    response: Some(core_response::Response::ListTakes(
                        mengxia_core_proto::ListTakesResult {
                            snapshot_ordinal: page.page().snapshot_endpoint(),
                            takes: page
                                .page()
                                .items()
                                .iter()
                                .map(take_response)
                                .collect::<Result<Vec<_>, _>>()?,
                            next_cursor: page.next_cursor().map(|cursor| cursor.to_vec()),
                        },
                    )),
                })
            })
        }
        _ => Err(AssetStoreError::Internal),
    };
    let (response, fatal) = match result {
        Ok(response) => (response, None),
        Err(error) => (
            query_error_response(error, correlation_id)?,
            query_fatal_code(error),
        ),
    };
    let response = bound_task_009_response(response, correlation_id)?;
    let _ = write_core_response(
        stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + response_timeout,
    )
    .await;
    fatal.map_or(Ok(()), Err)
}

fn decode_asset_revision_graph(
    input: &[mengxia_core_proto::AssetRevisionRepresentationInput],
) -> Result<Vec<AssetRevisionRepresentationInput>, ErrorCode> {
    if input.is_empty() || input.len() > 64 {
        return Err(ErrorCode::ValidationError);
    }
    input
        .iter()
        .map(|representation| {
            let purpose = RepresentationPurpose::new(representation.representation_purpose.clone())
                .map_err(|_| ErrorCode::ValidationError)?;
            let resources = representation
                .resources
                .iter()
                .map(|resource| {
                    let kind = ResourceKind::new(resource.resource_kind.clone())
                        .map_err(|_| ErrorCode::ValidationError)?;
                    let members = resource
                        .members
                        .iter()
                        .map(|member| {
                            let logical_name = LogicalName::new(member.logical_name.clone())
                                .map_err(|_| ErrorCode::ValidationError)?;
                            let digest: [u8; 32] = member
                                .blob_sha256
                                .as_slice()
                                .try_into()
                                .map_err(|_| ErrorCode::ValidationError)?;
                            Ok(AssetRevisionMemberInput::new(
                                logical_name,
                                Sha256Digest::from_bytes(digest),
                            ))
                        })
                        .collect::<Result<Vec<_>, ErrorCode>>()?;
                    AssetRevisionResourceInput::new(kind, members)
                        .map_err(|_| ErrorCode::ValidationError)
                })
                .collect::<Result<Vec<_>, ErrorCode>>()?;
            AssetRevisionRepresentationInput::new(purpose, resources)
                .map_err(|_| ErrorCode::ValidationError)
        })
        .collect()
}

fn asset_metadata_mutation_response(
    command_id: Id<PersistedCommand>,
    target: Option<AssetLifecycle>,
    outcome: mengxia_ports::MutationOutcome,
    correlation_id: &str,
) -> Result<CoreResponse, ErrorCode> {
    let (result, replayed) = match outcome {
        mengxia_ports::MutationOutcome::Applied(result) => (result, false),
        mengxia_ports::MutationOutcome::Replay(result) => (result, true),
        mengxia_ports::MutationOutcome::TerminalRejected { safe_error_code } => {
            return operation_error_response(
                safe_error_code,
                if safe_error_code == ErrorCode::Conflict {
                    RetryAction::FreshCommand
                } else {
                    RetryAction::None
                },
                correlation_id,
            )
            .map_err(|_| ErrorCode::InternalError);
        }
        mengxia_ports::MutationOutcome::RecoveryRequired { .. } => {
            return Err(ErrorCode::StorageCorruption);
        }
    };
    let response = match (target, result) {
        (None, CommandResult::AssetRevision(result)) => {
            let at = result.created_at();
            core_response::Response::CreateAssetRevision(
                mengxia_core_proto::CreateAssetRevisionResult {
                    command_id: command_id.to_string(),
                    asset_id: result.asset_id().to_string(),
                    asset_revision_id: result.asset_revision_id().to_string(),
                    resulting_revision: result.revision().get(),
                    created_at_seconds: at.unix_seconds(),
                    created_at_nanos: at.subsec_nanoseconds(),
                    replayed,
                },
            )
        }
        (Some(expected), CommandResult::Versioned(result)) => {
            let VersionedResultPayload::AssetLifecycle {
                revision,
                lifecycle,
            } = result.payload()
            else {
                return Err(ErrorCode::InternalError);
            };
            if lifecycle != expected {
                return Err(ErrorCode::InternalError);
            }
            let at = result.occurred_at();
            let value = mengxia_core_proto::AssetLifecycleMutationResult {
                command_id: command_id.to_string(),
                asset_id: Id::<Asset>::from_bytes(result.primary_id())
                    .map_err(|_| ErrorCode::InternalError)?
                    .to_string(),
                resulting_revision: revision,
                lifecycle: match lifecycle {
                    AssetLifecycle::Active => mengxia_core_proto::AssetLifecycleValue::Active,
                    AssetLifecycle::Retired => mengxia_core_proto::AssetLifecycleValue::Retired,
                } as i32,
                updated_at_seconds: at.unix_seconds(),
                updated_at_nanos: at.subsec_nanoseconds(),
                replayed,
            };
            if lifecycle == AssetLifecycle::Retired {
                core_response::Response::RetireAsset(value)
            } else {
                core_response::Response::RestoreAsset(value)
            }
        }
        _ => return Err(ErrorCode::InternalError),
    };
    Ok(CoreResponse {
        response: Some(response),
    })
}

#[allow(clippy::too_many_arguments)]
async fn serve_asset_metadata_mutation(
    stream: &mut tokio::net::UnixStream,
    request: &CoreRequest,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
    maximum_timeout: Duration,
    service: Arc<AssetMetadataCommandService>,
    cancelling: Arc<AtomicBool>,
    shutdown: watch::Receiver<bool>,
) -> Result<(), ErrorCode> {
    let control = Arc::new(StartupSqliteControl::new());
    let result = match request.operation.as_ref() {
        Some(core_request::Operation::CreateAssetRevision(request)) => {
            let decoded = (|| {
                let command_id = parse_canonical_id::<PersistedCommand>(&request.command_id)?;
                let asset_id = parse_canonical_id::<Asset>(&request.asset_id)?;
                let parents = request
                    .parent_revision_ids
                    .iter()
                    .map(|value| parse_canonical_id::<AssetRevision>(value))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((
                    command_id,
                    asset_id,
                    decode_revision(request.expected_revision)?,
                    parents,
                    ContentKind::new(request.content_kind.clone())
                        .map_err(|_| ErrorCode::ValidationError)?,
                    decode_asset_revision_graph(&request.representations)?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, asset_id, revision, parents, content, graph, timeout) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.create_revision(
                    command_id,
                    asset_id,
                    revision,
                    parents,
                    content,
                    graph,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, None, outcome))
        }
        Some(core_request::Operation::RetireAsset(request)) => {
            let target = AssetLifecycle::Retired;
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    parse_canonical_id::<Asset>(&request.asset_id)?,
                    decode_revision(request.expected_revision)?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, asset_id, revision, timeout) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.change_lifecycle(
                    command_id,
                    asset_id,
                    revision,
                    target,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, Some(target), outcome))
        }
        Some(core_request::Operation::RestoreAsset(request)) => {
            let target = AssetLifecycle::Active;
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    parse_canonical_id::<Asset>(&request.asset_id)?,
                    decode_revision(request.expected_revision)?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, asset_id, revision, timeout) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.change_lifecycle(
                    command_id,
                    asset_id,
                    revision,
                    target,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, Some(target), outcome))
        }
        _ => Err(AssetStoreError::Internal),
    };
    let (response, fatal) = match result {
        Ok((command_id, target, outcome)) => (
            asset_metadata_mutation_response(command_id, target, outcome, correlation_id)?,
            None,
        ),
        Err(error) => (
            query_error_response(error, correlation_id)?,
            query_fatal_code(error),
        ),
    };
    let response = bound_task_009_response(response, correlation_id)?;
    let _ = write_core_response(
        stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + response_timeout,
    )
    .await;
    fatal.map_or(Ok(()), Err)
}

#[allow(clippy::too_many_arguments)]
async fn serve_creative_mutation(
    stream: &mut tokio::net::UnixStream,
    request: &CoreRequest,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
    maximum_timeout: Duration,
    service: Arc<CreativeCommandService>,
    cancelling: Arc<AtomicBool>,
    shutdown: watch::Receiver<bool>,
) -> Result<(), ErrorCode> {
    let control = Arc::new(StartupSqliteControl::new());
    let result = match request.operation.as_ref() {
        Some(core_request::Operation::CreateProject(request)) => {
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    ProjectName::new(request.name.clone())
                        .map_err(|_| ErrorCode::ValidationError)?,
                    decode_project_specification(
                        request
                            .specification
                            .as_ref()
                            .ok_or(ErrorCode::ValidationError)?,
                    )?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, name, specification, timeout) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.create_project(
                    command_id,
                    name,
                    specification,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, CreativeMutationKind::CreateProject, outcome))
        }
        Some(core_request::Operation::ReviseProjectSpec(request)) => {
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    parse_canonical_id::<Project>(&request.project_id)?,
                    decode_revision(request.expected_revision)?,
                    decode_project_specification(
                        request
                            .specification
                            .as_ref()
                            .ok_or(ErrorCode::ValidationError)?,
                    )?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, project_id, revision, specification, timeout) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.revise_project_spec(
                    command_id,
                    project_id,
                    revision,
                    specification,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, CreativeMutationKind::ReviseProject, outcome))
        }
        Some(core_request::Operation::CreateSubject(request)) => {
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    SubjectKind::new(request.kind.clone())
                        .map_err(|_| ErrorCode::ValidationError)?,
                    SubjectName::new(request.canonical_name.clone())
                        .map_err(|_| ErrorCode::ValidationError)?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, kind, name, timeout) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.create_subject(
                    command_id,
                    kind,
                    name,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, CreativeMutationKind::CreateSubject, outcome))
        }
        Some(core_request::Operation::CreateWorkItem(request)) => {
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    parse_canonical_id::<Project>(&request.project_id)?,
                    decode_work_kind(request.kind)?,
                    WorkCode::new(request.code.clone()).map_err(|_| ErrorCode::ValidationError)?,
                    decode_work_specification(
                        &request.specification_json,
                        &request.subject_ids,
                        &request.asset_ids,
                    )?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, project_id, kind, code, specification, timeout) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.create_work(
                    command_id,
                    project_id,
                    kind,
                    code,
                    specification,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, CreativeMutationKind::CreateWork, outcome))
        }
        Some(core_request::Operation::ReviseWork(request)) => {
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    parse_canonical_id::<Project>(&request.project_id)?,
                    parse_canonical_id::<WorkItem>(&request.work_item_id)?,
                    decode_revision(request.expected_revision)?,
                    decode_work_specification(
                        &request.specification_json,
                        &request.subject_ids,
                        &request.asset_ids,
                    )?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, project_id, work_item_id, revision, specification, timeout) =
                match decoded {
                    Ok(value) => value,
                    Err(code) => {
                        return write_query_validation_error(
                            stream,
                            code,
                            correlation_id,
                            operation_limits,
                            response_timeout,
                        )
                        .await;
                    }
                };
            await_sqlite_query(
                stream,
                service.revise_work(
                    command_id,
                    project_id,
                    work_item_id,
                    revision,
                    specification,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, CreativeMutationKind::ReviseWork, outcome))
        }
        Some(core_request::Operation::CreateTake(request)) => {
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    parse_canonical_id::<Project>(&request.project_id)?,
                    parse_canonical_id::<WorkItem>(&request.work_item_id)?,
                    parse_canonical_id::<WorkRevision>(&request.work_revision_id)?,
                    parse_canonical_id::<Asset>(&request.primary_asset_id)?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (command_id, project_id, work_item_id, work_revision_id, asset_id, timeout) =
                match decoded {
                    Ok(value) => value,
                    Err(code) => {
                        return write_query_validation_error(
                            stream,
                            code,
                            correlation_id,
                            operation_limits,
                            response_timeout,
                        )
                        .await;
                    }
                };
            await_sqlite_query(
                stream,
                service.create_take(
                    command_id,
                    project_id,
                    work_item_id,
                    work_revision_id,
                    asset_id,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, CreativeMutationKind::CreateTake, outcome))
        }
        Some(core_request::Operation::TransitionTake(request)) => {
            let decoded = (|| {
                let related = match (
                    &request.related_take_id,
                    request.related_take_expected_revision,
                ) {
                    (Some(id), Some(revision)) => {
                        Some((parse_canonical_id::<Take>(id)?, decode_revision(revision)?))
                    }
                    (None, None) => None,
                    _ => return Err(ErrorCode::ValidationError),
                };
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    parse_canonical_id::<Project>(&request.project_id)?,
                    parse_canonical_id::<WorkItem>(&request.work_item_id)?,
                    parse_canonical_id::<WorkRevision>(&request.work_revision_id)?,
                    parse_canonical_id::<Take>(&request.take_id)?,
                    decode_revision(request.expected_revision)?,
                    decode_take_transition(request.transition)?,
                    request
                        .reason
                        .as_ref()
                        .map(|reason| {
                            TakeReason::new(reason.clone()).map_err(|_| ErrorCode::ValidationError)
                        })
                        .transpose()?,
                    related,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (
                command_id,
                project_id,
                work_item_id,
                work_revision_id,
                take_id,
                revision,
                transition,
                reason,
                related,
                timeout,
            ) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.transition_take(
                    command_id,
                    project_id,
                    work_item_id,
                    work_revision_id,
                    take_id,
                    revision,
                    transition,
                    reason,
                    related,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, CreativeMutationKind::TransitionTake, outcome))
        }
        Some(core_request::Operation::ReopenTake(request)) => {
            let decoded = (|| {
                Ok((
                    parse_canonical_id::<PersistedCommand>(&request.command_id)?,
                    parse_canonical_id::<Project>(&request.project_id)?,
                    parse_canonical_id::<WorkItem>(&request.work_item_id)?,
                    parse_canonical_id::<WorkRevision>(&request.work_revision_id)?,
                    parse_canonical_id::<Take>(&request.terminal_take_id)?,
                    decode_revision(request.terminal_take_expected_revision)?,
                    parse_canonical_id::<Asset>(&request.new_primary_asset_id)?,
                    decode_bounded_operation_timeout(
                        request.operation_timeout_ms,
                        maximum_timeout,
                    )?,
                ))
            })();
            let (
                command_id,
                project_id,
                work_item_id,
                work_revision_id,
                terminal_take_id,
                revision,
                asset_id,
                timeout,
            ) = match decoded {
                Ok(value) => value,
                Err(code) => {
                    return write_query_validation_error(
                        stream,
                        code,
                        correlation_id,
                        operation_limits,
                        response_timeout,
                    )
                    .await;
                }
            };
            await_sqlite_query(
                stream,
                service.reopen_take(
                    command_id,
                    project_id,
                    work_item_id,
                    work_revision_id,
                    terminal_take_id,
                    revision,
                    asset_id,
                    Arc::clone(&control) as Arc<dyn IngestControl>,
                ),
                tokio::time::Instant::now() + timeout,
                control,
                cancelling,
                shutdown,
            )
            .await
            .map(|outcome| (command_id, CreativeMutationKind::ReopenTake, outcome))
        }
        _ => Err(AssetStoreError::Internal),
    };
    let response = match result {
        Ok((command_id, kind, outcome)) => {
            creative_mutation_response(command_id, kind, outcome, correlation_id)?
        }
        Err(error) => query_error_response(error, correlation_id)?,
    };
    let response = bound_task_009_response(response, correlation_id)?;
    let _ = write_core_response(
        stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + response_timeout,
    )
    .await;
    Ok(())
}

fn decode_project_specification(
    input: &mengxia_core_proto::ProjectSpecInput,
) -> Result<ProjectSpecification, ErrorCode> {
    let resolution = optional_pair(
        input.resolution_width,
        input.resolution_height,
        Resolution::new,
    )?;
    let frame_rate = optional_pair(
        input.frame_rate_numerator,
        input.frame_rate_denominator,
        PositiveRatio::new,
    )?;
    let aspect_ratio = optional_pair(
        input.aspect_ratio_numerator,
        input.aspect_ratio_denominator,
        PositiveRatio::new,
    )?;
    mengxia_app::parse_project_specification(
        resolution,
        frame_rate,
        aspect_ratio,
        &input.color_policy_json,
        &input.audio_policy_json,
        &input.quality_policy_json,
        &input.privacy_policy_json,
    )
    .map_err(|_| ErrorCode::ValidationError)
}

fn optional_pair<T, E>(
    left: Option<u32>,
    right: Option<u32>,
    constructor: impl FnOnce(u32, u32) -> Result<T, E>,
) -> Result<Option<T>, ErrorCode> {
    match (left, right) {
        (None, None) => Ok(None),
        (Some(left), Some(right)) => constructor(left, right)
            .map(Some)
            .map_err(|_| ErrorCode::ValidationError),
        _ => Err(ErrorCode::ValidationError),
    }
}

fn decode_work_specification(
    json: &[u8],
    subject_ids: &[String],
    asset_ids: &[String],
) -> Result<WorkSpecification, ErrorCode> {
    let json =
        mengxia_app::parse_work_specification(json).map_err(|_| ErrorCode::ValidationError)?;
    let subjects = subject_ids
        .iter()
        .map(|value| parse_canonical_id::<Subject>(value))
        .collect::<Result<Vec<_>, _>>()?;
    let assets = asset_ids
        .iter()
        .map(|value| parse_canonical_id::<Asset>(value))
        .collect::<Result<Vec<_>, _>>()?;
    WorkSpecification::new(json, subjects, assets).map_err(|_| ErrorCode::ValidationError)
}

fn decode_work_kind(value: i32) -> Result<WorkKind, ErrorCode> {
    match mengxia_core_proto::WorkKindValue::try_from(value) {
        Ok(mengxia_core_proto::WorkKindValue::Scene) => Ok(WorkKind::Scene),
        Ok(mengxia_core_proto::WorkKindValue::Shot) => Ok(WorkKind::Shot),
        _ => Err(ErrorCode::ValidationError),
    }
}

fn decode_take_transition(value: i32) -> Result<TakeTransition, ErrorCode> {
    match mengxia_core_proto::TakeTransitionValue::try_from(value) {
        Ok(mengxia_core_proto::TakeTransitionValue::Shortlist) => Ok(TakeTransition::Shortlist),
        Ok(mengxia_core_proto::TakeTransitionValue::Select) => Ok(TakeTransition::Select),
        Ok(mengxia_core_proto::TakeTransitionValue::Approve) => Ok(TakeTransition::Approve),
        Ok(mengxia_core_proto::TakeTransitionValue::Reject) => Ok(TakeTransition::Reject),
        Ok(mengxia_core_proto::TakeTransitionValue::Supersede) => Ok(TakeTransition::Supersede),
        _ => Err(ErrorCode::ValidationError),
    }
}

#[derive(Clone, Copy)]
enum CreativeMutationKind {
    CreateProject,
    ReviseProject,
    CreateSubject,
    CreateWork,
    ReviseWork,
    CreateTake,
    TransitionTake,
    ReopenTake,
}

fn creative_mutation_response(
    command_id: Id<PersistedCommand>,
    kind: CreativeMutationKind,
    outcome: mengxia_ports::MutationOutcome,
    correlation_id: &str,
) -> Result<CoreResponse, ErrorCode> {
    let (result, replayed) = match outcome {
        mengxia_ports::MutationOutcome::Applied(result) => (result, false),
        mengxia_ports::MutationOutcome::Replay(result) => (result, true),
        mengxia_ports::MutationOutcome::TerminalRejected { safe_error_code } => {
            let retry = if safe_error_code == ErrorCode::Conflict {
                RetryAction::FreshCommand
            } else {
                RetryAction::None
            };
            return operation_error_response(safe_error_code, retry, correlation_id)
                .map_err(|_| ErrorCode::InternalError);
        }
        mengxia_ports::MutationOutcome::RecoveryRequired { .. } => {
            return Err(ErrorCode::StorageCorruption);
        }
    };
    let CommandResult::Versioned(result) = result else {
        return Err(ErrorCode::InternalError);
    };
    let at = result.occurred_at();
    let response = match (kind, result.payload()) {
        (
            CreativeMutationKind::CreateProject,
            VersionedResultPayload::Project {
                spec_revision_id,
                revision,
                sequence,
            },
        ) => core_response::Response::CreateProject(mengxia_core_proto::ProjectMutationResult {
            command_id: command_id.to_string(),
            project_id: Id::<Project>::from_bytes(result.primary_id())
                .map_err(|_| ErrorCode::InternalError)?
                .to_string(),
            project_spec_revision_id: Id::<mengxia_domain::ProjectSpecRevision>::from_bytes(
                spec_revision_id,
            )
            .map_err(|_| ErrorCode::InternalError)?
            .to_string(),
            project_revision: revision,
            specification_sequence: sequence,
            updated_at_seconds: at.unix_seconds(),
            updated_at_nanos: at.subsec_nanoseconds(),
            replayed,
        }),
        (
            CreativeMutationKind::ReviseProject,
            VersionedResultPayload::ProjectSpecRevision {
                project_id,
                revision,
                sequence,
            },
        ) => {
            core_response::Response::ReviseProjectSpec(mengxia_core_proto::ProjectMutationResult {
                command_id: command_id.to_string(),
                project_id: Id::<Project>::from_bytes(project_id)
                    .map_err(|_| ErrorCode::InternalError)?
                    .to_string(),
                project_spec_revision_id: Id::<mengxia_domain::ProjectSpecRevision>::from_bytes(
                    result.primary_id(),
                )
                .map_err(|_| ErrorCode::InternalError)?
                .to_string(),
                project_revision: revision,
                specification_sequence: sequence,
                updated_at_seconds: at.unix_seconds(),
                updated_at_nanos: at.subsec_nanoseconds(),
                replayed,
            })
        }
        (CreativeMutationKind::CreateSubject, VersionedResultPayload::Subject { revision }) => {
            core_response::Response::CreateSubject(mengxia_core_proto::SubjectMutationResult {
                command_id: command_id.to_string(),
                subject_id: Id::<Subject>::from_bytes(result.primary_id())
                    .map_err(|_| ErrorCode::InternalError)?
                    .to_string(),
                subject_revision: revision,
                created_at_seconds: at.unix_seconds(),
                created_at_nanos: at.subsec_nanoseconds(),
                replayed,
            })
        }
        (
            CreativeMutationKind::CreateWork,
            VersionedResultPayload::WorkItem {
                work_revision_id,
                revision,
                sequence,
            },
        ) => core_response::Response::CreateWorkItem(mengxia_core_proto::WorkMutationResult {
            command_id: command_id.to_string(),
            work_item_id: Id::<WorkItem>::from_bytes(result.primary_id())
                .map_err(|_| ErrorCode::InternalError)?
                .to_string(),
            work_revision_id: Id::<WorkRevision>::from_bytes(work_revision_id)
                .map_err(|_| ErrorCode::InternalError)?
                .to_string(),
            work_item_revision: revision,
            work_revision_sequence: sequence,
            updated_at_seconds: at.unix_seconds(),
            updated_at_nanos: at.subsec_nanoseconds(),
            replayed,
        }),
        (
            CreativeMutationKind::ReviseWork,
            VersionedResultPayload::WorkRevision {
                work_item_id,
                revision,
                sequence,
            },
        ) => core_response::Response::ReviseWork(mengxia_core_proto::WorkMutationResult {
            command_id: command_id.to_string(),
            work_item_id: Id::<WorkItem>::from_bytes(work_item_id)
                .map_err(|_| ErrorCode::InternalError)?
                .to_string(),
            work_revision_id: Id::<WorkRevision>::from_bytes(result.primary_id())
                .map_err(|_| ErrorCode::InternalError)?
                .to_string(),
            work_item_revision: revision,
            work_revision_sequence: sequence,
            updated_at_seconds: at.unix_seconds(),
            updated_at_nanos: at.subsec_nanoseconds(),
            replayed,
        }),
        (
            kind @ (CreativeMutationKind::CreateTake
            | CreativeMutationKind::TransitionTake
            | CreativeMutationKind::ReopenTake),
            VersionedResultPayload::Take {
                revision,
                ordinal,
                state,
                primary_asset_id,
                related_take_id,
            },
        ) => {
            let result = mengxia_core_proto::TakeMutationResult {
                command_id: command_id.to_string(),
                take_id: Id::<Take>::from_bytes(result.primary_id())
                    .map_err(|_| ErrorCode::InternalError)?
                    .to_string(),
                ordinal,
                state: take_state_response(state) as i32,
                primary_asset_id: Id::<Asset>::from_bytes(primary_asset_id)
                    .map_err(|_| ErrorCode::InternalError)?
                    .to_string(),
                take_revision: revision,
                related_take_id: related_take_id
                    .map(|id| Id::<Take>::from_bytes(id).map(|value| value.to_string()))
                    .transpose()
                    .map_err(|_| ErrorCode::InternalError)?,
                updated_at_seconds: at.unix_seconds(),
                updated_at_nanos: at.subsec_nanoseconds(),
                replayed,
            };
            match kind {
                CreativeMutationKind::CreateTake => core_response::Response::CreateTake(result),
                CreativeMutationKind::TransitionTake => {
                    core_response::Response::TransitionTake(result)
                }
                CreativeMutationKind::ReopenTake => core_response::Response::ReopenTake(result),
                _ => return Err(ErrorCode::InternalError),
            }
        }
        _ => return Err(ErrorCode::InternalError),
    };
    Ok(CoreResponse {
        response: Some(response),
    })
}

fn project_response(view: &mengxia_ports::ProjectView) -> mengxia_core_proto::ProjectView {
    let specification = view.specification();
    let resolution = specification.resolution();
    let frame_rate = specification.frame_rate();
    let aspect_ratio = specification.aspect_ratio();
    mengxia_core_proto::ProjectView {
        project_id: view.project_id().to_string(),
        name: view.name().as_str().to_owned(),
        revision: view.revision().get(),
        created_at_seconds: view.created_at().unix_seconds(),
        created_at_nanos: view.created_at().subsec_nanoseconds(),
        updated_at_seconds: view.updated_at().unix_seconds(),
        updated_at_nanos: view.updated_at().subsec_nanoseconds(),
        creation_commit_sequence: view.creation_commit_sequence(),
        current_specification: Some(mengxia_core_proto::ProjectSpecView {
            project_spec_revision_id: view.spec_revision_id().to_string(),
            sequence: view.spec_sequence(),
            resolution_width: resolution.map(|value| u32::from(value.width())),
            resolution_height: resolution.map(|value| u32::from(value.height())),
            frame_rate_numerator: frame_rate.map(|value| value.numerator()),
            frame_rate_denominator: frame_rate.map(|value| value.denominator()),
            aspect_ratio_numerator: aspect_ratio.map(|value| value.numerator()),
            aspect_ratio_denominator: aspect_ratio.map(|value| value.denominator()),
            policy_schema_version: 1,
            color_policy_json: specification.color_policy().bytes().to_vec(),
            audio_policy_json: specification.audio_policy().bytes().to_vec(),
            quality_policy_json: specification.quality_policy().bytes().to_vec(),
            privacy_policy_json: specification.privacy_policy().bytes().to_vec(),
            policy_sha256: specification.policy_digest().to_bytes().to_vec(),
        }),
        effective_trust: mengxia_core_proto::ProjectTrustValue::Untrusted as i32,
    }
}

fn subject_response(view: &mengxia_ports::SubjectView) -> mengxia_core_proto::SubjectView {
    mengxia_core_proto::SubjectView {
        subject_id: view.subject_id().to_string(),
        kind: view.kind().as_str().to_owned(),
        canonical_name: view.name().as_str().to_owned(),
        revision: view.revision().get(),
        created_at_seconds: view.created_at().unix_seconds(),
        created_at_nanos: view.created_at().subsec_nanoseconds(),
        creation_commit_sequence: view.creation_commit_sequence(),
    }
}

fn work_response(view: &mengxia_ports::WorkView) -> mengxia_core_proto::WorkView {
    let specification = view.specification();
    mengxia_core_proto::WorkView {
        work_item_id: view.work_item_id().to_string(),
        project_id: view.project_id().to_string(),
        kind: match view.kind() {
            mengxia_domain::WorkKind::Scene => mengxia_core_proto::WorkKindValue::Scene as i32,
            mengxia_domain::WorkKind::Shot => mengxia_core_proto::WorkKindValue::Shot as i32,
        },
        code: view.code().as_str().to_owned(),
        revision: view.revision().get(),
        created_at_seconds: view.created_at().unix_seconds(),
        created_at_nanos: view.created_at().subsec_nanoseconds(),
        updated_at_seconds: view.updated_at().unix_seconds(),
        updated_at_nanos: view.updated_at().subsec_nanoseconds(),
        creation_commit_sequence: view.creation_commit_sequence(),
        current_work_revision_id: view.work_revision_id().to_string(),
        current_work_revision_sequence: view.work_revision_sequence(),
        specification_schema_version: 1,
        specification_json: specification.json().bytes().to_vec(),
        specification_sha256: specification.json().digest().to_bytes().to_vec(),
        subject_ids: specification
            .subject_ids()
            .iter()
            .map(ToString::to_string)
            .collect(),
        asset_ids: specification
            .asset_ids()
            .iter()
            .map(ToString::to_string)
            .collect(),
    }
}

fn take_response(
    view: &mengxia_ports::TakeView,
) -> Result<mengxia_core_proto::TakeView, AssetStoreError> {
    Ok(mengxia_core_proto::TakeView {
        take_id: view.take_id().to_string(),
        work_revision_id: view.work_revision_id().to_string(),
        ordinal: view.ordinal(),
        state: take_state_response(view.state()) as i32,
        primary_asset_id: view.primary_asset_id().to_string(),
        revision: view.revision().get(),
        created_at_seconds: view.created_at().unix_seconds(),
        created_at_nanos: view.created_at().subsec_nanoseconds(),
        updated_at_seconds: view.updated_at().unix_seconds(),
        updated_at_nanos: view.updated_at().subsec_nanoseconds(),
        outgoing_relationships: view
            .outgoing_relationships()
            .iter()
            .map(|relationship| {
                Ok(mengxia_core_proto::TakeRelationshipView {
                    relationship_id: relationship.relationship_id().to_string(),
                    kind: match relationship.kind() {
                        mengxia_domain::RelationshipKind::TakeReopens => {
                            mengxia_core_proto::TakeRelationshipKindValue::Reopens as i32
                        }
                        mengxia_domain::RelationshipKind::TakeSupersedes => {
                            mengxia_core_proto::TakeRelationshipKindValue::Supersedes as i32
                        }
                        _ => return Err(AssetStoreError::StorageCorruption),
                    },
                    target_take_id: relationship.target_take_id().to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    })
}

const fn take_state_response(
    state: mengxia_domain::TakeState,
) -> mengxia_core_proto::TakeStateValue {
    match state {
        mengxia_domain::TakeState::Candidate => mengxia_core_proto::TakeStateValue::Candidate,
        mengxia_domain::TakeState::Shortlisted => mengxia_core_proto::TakeStateValue::Shortlisted,
        mengxia_domain::TakeState::Selected => mengxia_core_proto::TakeStateValue::Selected,
        mengxia_domain::TakeState::Approved => mengxia_core_proto::TakeStateValue::Approved,
        mengxia_domain::TakeState::Rejected => mengxia_core_proto::TakeStateValue::Rejected,
        mengxia_domain::TakeState::Superseded => mengxia_core_proto::TakeStateValue::Superseded,
    }
}

#[allow(clippy::too_many_arguments)]
async fn serve_verify_library(
    stream: &mut tokio::net::UnixStream,
    request: &mengxia_core_proto::VerifyLibraryRequest,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
    max_verify_operation_timeout: Duration,
    service: Arc<VerificationService<SqliteAssetStoreHandle, LocalBlobStorage>>,
    cancelling: Arc<AtomicBool>,
    shutdown: watch::Receiver<bool>,
    health: Arc<RwLock<LibraryHealthState>>,
) -> Result<(), ErrorCode> {
    let decoded = (|| {
        let mode = match request.mode {
            value if value == mengxia_core_proto::VerificationMode::Normal as i32 => {
                VerificationMode::Normal
            }
            value if value == mengxia_core_proto::VerificationMode::Deep as i32 => {
                VerificationMode::Deep
            }
            _ => return Err(ErrorCode::ValidationError),
        };
        Ok((
            mode,
            decode_bounded_operation_timeout(
                request.operation_timeout_ms,
                max_verify_operation_timeout,
            )?,
        ))
    })();
    let (mode, timeout) = match decoded {
        Ok(decoded) => decoded,
        Err(code) => {
            return write_query_validation_error(
                stream,
                code,
                correlation_id,
                operation_limits,
                response_timeout,
            )
            .await;
        }
    };
    let control = Arc::new(StartupSqliteControl::new());
    if cancelling.load(Ordering::Acquire) || *shutdown.borrow() {
        control
            .stop(IngestStop::Cancelled)
            .map_err(|()| ErrorCode::InternalError)?;
    }
    let worker_service = Arc::clone(&service);
    let worker_control = Arc::clone(&control);
    let runtime = tokio::runtime::Handle::current();
    let worker = tokio::task::spawn_blocking(move || {
        runtime.block_on(
            worker_service
                .verify_controlled(mode, worker_control as Arc<dyn InterruptibleSqliteControl>),
        )
    });
    let operation_deadline = tokio::time::Instant::now() + timeout;
    let result = await_verification_with_watcher(
        stream,
        worker,
        operation_deadline,
        Arc::clone(&control),
        cancelling,
        shutdown,
    )
    .await?;
    let (response, fatal) = match result {
        Ok(summary) => {
            let fatal = summary
                .has_fatal_local_issue()
                .then_some(ErrorCode::StorageCorruption);
            if fatal.is_none() {
                publish_verification_health(&health, summary)?;
            }
            (
                CoreResponse {
                    response: Some(core_response::Response::VerifyLibrary(
                        mengxia_core_proto::VerifyLibraryResult {
                            verification_id: summary.verification_id().to_string(),
                            mode: verification_mode_response(summary.mode()) as i32,
                            snapshot_commit_sequence: summary.snapshot_commit_sequence(),
                            discovered_issue_count: summary.discovered_issue_count(),
                            stored_issue_count: summary.stored_issue_count(),
                            dropped_issue_count: summary.dropped_issue_count(),
                            has_fatal_local_issue: summary.has_fatal_local_issue(),
                            has_custody_degradation: summary.has_custody_degradation(),
                            canonical_extra_classification_deferred: summary
                                .canonical_extra_classification_deferred(),
                            first_fatal_issue: summary
                                .first_fatal_issue()
                                .map(integrity_issue_response),
                        },
                    )),
                },
                fatal,
            )
        }
        Err(error) => (
            query_error_response(error, correlation_id)?,
            query_fatal_code(error),
        ),
    };
    let _ = write_core_response(stream, &response, operation_limits, operation_deadline).await;
    fatal.map_or(Ok(()), Err)
}

async fn serve_list_integrity_issues(
    stream: &mut tokio::net::UnixStream,
    request: &mengxia_core_proto::ListIntegrityIssuesRequest,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
    service: Arc<VerificationService<SqliteAssetStoreHandle, LocalBlobStorage>>,
) -> Result<(), ErrorCode> {
    let decoded = (|| {
        Ok((
            parse_canonical_id::<VerificationReportIdentity>(&request.verification_id)?,
            decode_query_timeout(request.operation_timeout_ms)?,
        ))
    })();
    let (verification_id, timeout) = match decoded {
        Ok(decoded) => decoded,
        Err(code) => {
            return write_query_validation_error(
                stream,
                code,
                correlation_id,
                operation_limits,
                response_timeout,
            )
            .await;
        }
    };
    let response = match service.list_issues(
        verification_id,
        request.page_size,
        (!request.cursor.is_empty()).then_some(request.cursor.as_slice()),
    ) {
        Ok(result) => {
            let page = result.page();
            CoreResponse {
                response: Some(core_response::Response::ListIntegrityIssues(
                    mengxia_core_proto::ListIntegrityIssuesResult {
                        verification_id: page.verification_id().to_string(),
                        issues: page
                            .issues()
                            .iter()
                            .copied()
                            .map(integrity_issue_response)
                            .collect(),
                        next_cursor: result.next_cursor().map(|cursor| cursor.to_vec()),
                        discovered_issue_count: page.discovered_issue_count(),
                        stored_issue_count: page.stored_issue_count(),
                        dropped_issue_count: page.dropped_issue_count(),
                    },
                )),
            }
        }
        Err(error) => query_error_response(error, correlation_id)?,
    };
    let _ = write_core_response(
        stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + timeout,
    )
    .await;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn serve_materialize_asset(
    stream: &mut tokio::net::UnixStream,
    request: &mengxia_core_proto::MaterializeAssetRequest,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
    max_materialize_operation_timeout: Duration,
    service: Arc<MaterializeAssetService>,
    cancelling: Arc<AtomicBool>,
) -> Result<(), ErrorCode> {
    let (request, timeout) =
        match decode_materialize_request(request, max_materialize_operation_timeout) {
            Ok(decoded) => decoded,
            Err(code) => {
                return write_query_validation_error(
                    stream,
                    code,
                    correlation_id,
                    operation_limits,
                    response_timeout,
                )
                .await;
            }
        };
    let semantic_deadline = StdInstant::now() + timeout;
    let transport_deadline = tokio::time::Instant::now() + timeout;
    let peer_stopped = Arc::new(AtomicBool::new(false));
    let control = session_control(semantic_deadline, cancelling, Arc::clone(&peer_stopped));
    let runtime = tokio::runtime::Handle::current();
    let worker = tokio::task::spawn_blocking(move || {
        runtime.block_on(service.execute_controlled(request, control))
    });
    let result = await_ingest_with_watcher(
        stream,
        worker,
        transport_deadline,
        Arc::clone(&peer_stopped),
    )
    .await?;
    let response = match result {
        Ok(result) => CoreResponse {
            response: Some(core_response::Response::MaterializeAsset(
                mengxia_core_proto::MaterializeAssetResult {
                    command_id: result.command_id().to_string(),
                    asset_revision_id: result.asset_revision_id().to_string(),
                    representation_id: result.representation_id().to_string(),
                    resource_id: result.resource_id().to_string(),
                    member_ordinal: result.member_ordinal(),
                    blob_sha256: result.blob_digest().to_bytes().to_vec(),
                    byte_length: result.byte_length(),
                    replayed: result.replayed(),
                    cleanup_pending: result.cleanup_pending(),
                },
            )),
        },
        Err(failure) => materialize_error_response(failure, correlation_id)?,
    };
    let _ = write_core_response(
        stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + response_timeout,
    )
    .await;
    Ok(())
}

fn decode_materialize_request(
    request: &mengxia_core_proto::MaterializeAssetRequest,
    maximum_timeout: Duration,
) -> Result<(MaterializeAssetRequest, Duration), ErrorCode> {
    let destination = request.destination_path.as_slice();
    let final_component_length = destination
        .rsplit(|byte| *byte == b'/')
        .next()
        .map_or(0, <[u8]>::len);
    if !(1..=1023).contains(&destination.len())
        || destination.contains(&0)
        || !normalized_absolute_bytes(destination)
        || !(1..=255).contains(&final_component_length)
        || request.member_ordinal > 4095
    {
        return Err(ErrorCode::ValidationError);
    }
    let timeout = decode_bounded_operation_timeout(request.operation_timeout_ms, maximum_timeout)?;
    let command_id = parse_canonical_id::<PersistedCommand>(&request.command_id)?;
    let asset_id = parse_canonical_id::<Asset>(&request.asset_id)?;
    let asset_revision_id = parse_canonical_id::<AssetRevision>(&request.asset_revision_id)?;
    let representation_id = parse_canonical_id::<Representation>(&request.representation_id)?;
    let resource_id = parse_canonical_id::<Resource>(&request.resource_id)?;
    let request = MaterializeAssetRequest::new(
        command_id,
        asset_id,
        asset_revision_id,
        representation_id,
        resource_id,
        request.member_ordinal,
        destination.to_vec(),
    )?;
    Ok((request, timeout))
}

fn materialize_error_response(
    failure: MaterializeAssetFailure,
    correlation_id: &str,
) -> Result<CoreResponse, ErrorCode> {
    let retry = match failure.code() {
        ErrorCode::ValidationError | ErrorCode::NotFound | ErrorCode::Conflict => RetryAction::None,
        ErrorCode::StorageBusy | ErrorCode::CommandInProgress | ErrorCode::StorageIoError => {
            RetryAction::SameCommand
        }
        ErrorCode::Backpressure | ErrorCode::DeadlineExceeded | ErrorCode::OperationCancelled => {
            RetryAction::FreshCommand
        }
        ErrorCode::StorageCorruption
        | ErrorCode::StorageConfigurationError
        | ErrorCode::IdGenerationUnavailable => RetryAction::OperatorOrRuntimeAction,
        code => return Err(code),
    };
    operation_error_response(failure.code(), retry, correlation_id)
        .map_err(|_| ErrorCode::InternalError)
}

async fn await_verification_with_watcher<T>(
    stream: &mut tokio::net::UnixStream,
    mut worker: tokio::task::JoinHandle<Result<T, AssetStoreError>>,
    deadline: tokio::time::Instant,
    control: Arc<StartupSqliteControl>,
    cancelling: Arc<AtomicBool>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<Result<T, AssetStoreError>, ErrorCode>
where
    T: Send + 'static,
{
    if cancelling.load(Ordering::Acquire) || *shutdown.borrow() {
        control
            .stop(IngestStop::Cancelled)
            .map_err(|()| ErrorCode::InternalError)?;
    }
    let mut unexpected = [0_u8; 1];
    tokio::select! {
        joined = &mut worker => joined.map_err(|_| ErrorCode::InternalError),
        _ = tokio::time::sleep_until(deadline) => {
            control.stop(IngestStop::DeadlineReached).map_err(|()| ErrorCode::InternalError)?;
            worker.await.map_err(|_| ErrorCode::InternalError)
        }
        _ = stream.read(&mut unexpected) => {
            control.stop(IngestStop::Cancelled).map_err(|()| ErrorCode::InternalError)?;
            worker.await.map_err(|_| ErrorCode::InternalError)
        }
        _ = shutdown.changed() => {
            control.stop(IngestStop::Cancelled).map_err(|()| ErrorCode::InternalError)?;
            worker.await.map_err(|_| ErrorCode::InternalError)
        }
    }
}

fn publish_verification_health(
    health: &RwLock<LibraryHealthState>,
    summary: mengxia_ports::VerificationSummary,
) -> Result<(), ErrorCode> {
    let current = read_health(health)?;
    let custody_observation = match summary.mode() {
        VerificationMode::Normal => AppCustodyObservation::NormalVerified,
        VerificationMode::Deep => AppCustodyObservation::DeepVerified,
    };
    let next = LibraryHealthState::new(LibraryHealthInput {
        liveness: current.liveness(),
        readiness_block_reason: current.readiness_block_reason(),
        local_security_baseline: current.local_security_baseline(),
        custody_observation,
        staging_orphan_count: current.staging_orphan_count(),
        staging_orphan_bytes: current.staging_orphan_bytes(),
        local_backend_matches: current.local_backend_matches(),
        observability_degraded: current.observability_degraded(),
        recovery_observation_available: current.recovery_observation_available(),
        recovery_required_command_count: current.recovery_required_command_count(),
        custody_degraded: summary.has_custody_degradation()
            || current.availability() == AppCoreAvailability::DegradedCustody,
        dependency_degraded: current.availability() == AppCoreAvailability::DegradedDependency,
    })?;
    publish_health(health, next)
}

const fn verification_mode_response(
    mode: VerificationMode,
) -> mengxia_core_proto::VerificationMode {
    match mode {
        VerificationMode::Normal => mengxia_core_proto::VerificationMode::Normal,
        VerificationMode::Deep => mengxia_core_proto::VerificationMode::Deep,
    }
}

fn integrity_issue_response(issue: IntegrityIssue) -> mengxia_core_proto::IntegrityIssue {
    let object_id = issue.object_id().map(|id| match id {
        IntegrityObjectId::Uuid(bytes) => bytes.to_vec(),
        IntegrityObjectId::Digest(digest) => digest.to_bytes().to_vec(),
    });
    mengxia_core_proto::IntegrityIssue {
        ordinal: issue.ordinal(),
        kind: integrity_issue_kind_response(issue.kind()) as i32,
        severity: integrity_severity_response(issue.severity()) as i32,
        object_kind: integrity_object_kind_response(issue.object_kind()) as i32,
        object_id,
        remediation: integrity_remediation_response(issue.remediation()) as i32,
    }
}

const fn integrity_issue_kind_response(
    kind: IntegrityIssueKind,
) -> mengxia_core_proto::IntegrityIssueKind {
    use mengxia_core_proto::IntegrityIssueKind as Wire;
    match kind {
        IntegrityIssueKind::DatabaseIntegrityFailure => Wire::DatabaseIntegrityFailure,
        IntegrityIssueKind::SchemaOrMigrationMismatch => Wire::SchemaOrMigrationMismatch,
        IntegrityIssueKind::LibraryAuthorityMismatch => Wire::LibraryAuthorityMismatch,
        IntegrityIssueKind::CommandRecoveryRequired => Wire::CommandRecoveryRequired,
        IntegrityIssueKind::EventOrGraphInconsistent => Wire::EventOrGraphInconsistent,
        IntegrityIssueKind::LocalBackendMismatch => Wire::LocalBackendMismatch,
        IntegrityIssueKind::ManagedBlobMissing => Wire::ManagedBlobMissing,
        IntegrityIssueKind::ManagedBlobUnsafe => Wire::ManagedBlobUnsafe,
        IntegrityIssueKind::ManagedBlobLengthMismatch => Wire::ManagedBlobLengthMismatch,
        IntegrityIssueKind::ManagedBlobDigestMismatch => Wire::ManagedBlobDigestMismatch,
        IntegrityIssueKind::UnregisteredCanonicalBlob => Wire::UnregisteredCanonicalBlob,
        IntegrityIssueKind::StagingOrphan => Wire::StagingOrphan,
        IntegrityIssueKind::UnsafeCasNamespaceEntry => Wire::UnsafeCasNamespaceEntry,
        IntegrityIssueKind::MaterializationRecoveryRequired => {
            Wire::MaterializationRecoveryRequired
        }
    }
}

const fn integrity_severity_response(
    severity: IntegritySeverity,
) -> mengxia_core_proto::IntegritySeverity {
    use mengxia_core_proto::IntegritySeverity as Wire;
    match severity {
        IntegritySeverity::FatalLocal => Wire::FatalLocal,
        IntegritySeverity::ReadOnlyCustody => Wire::ReadOnlyCustody,
        IntegritySeverity::DegradedCustody => Wire::DegradedCustody,
        IntegritySeverity::OperatorAction => Wire::OperatorAction,
    }
}

const fn integrity_object_kind_response(
    kind: IntegrityObjectKind,
) -> mengxia_core_proto::IntegrityObjectKind {
    use mengxia_core_proto::IntegrityObjectKind as Wire;
    match kind {
        IntegrityObjectKind::Library => Wire::Library,
        IntegrityObjectKind::Command => Wire::Command,
        IntegrityObjectKind::Asset => Wire::Asset,
        IntegrityObjectKind::AssetRevision => Wire::AssetRevision,
        IntegrityObjectKind::Blob => Wire::Blob,
        IntegrityObjectKind::Location => Wire::Location,
        IntegrityObjectKind::Staging => Wire::Staging,
        IntegrityObjectKind::Materialization => Wire::Materialization,
    }
}

const fn integrity_remediation_response(
    remediation: IntegrityRemediation,
) -> mengxia_core_proto::IntegrityRemediation {
    use mengxia_core_proto::IntegrityRemediation as Wire;
    match remediation {
        IntegrityRemediation::None => Wire::None,
        IntegrityRemediation::RetryExactCommand => Wire::RetryExactCommand,
        IntegrityRemediation::RerunWhenIdle => Wire::RerunWhenIdle,
        IntegrityRemediation::OperatorConfiguration => Wire::OperatorConfiguration,
        IntegrityRemediation::FutureAdminAction => Wire::FutureAdminAction,
        IntegrityRemediation::OperatorOrRuntimeAction => Wire::OperatorOrRuntimeAction,
    }
}

async fn write_query_validation_error(
    stream: &mut tokio::net::UnixStream,
    code: ErrorCode,
    correlation_id: &str,
    operation_limits: OperationLimits,
    response_timeout: Duration,
) -> Result<(), ErrorCode> {
    let response = operation_error_response(code, RetryAction::None, correlation_id)
        .map_err(|_| ErrorCode::InternalError)?;
    let _ = write_core_response(
        stream,
        &response,
        operation_limits,
        tokio::time::Instant::now() + response_timeout,
    )
    .await;
    Ok(())
}

async fn await_sqlite_query<F, T>(
    stream: &mut tokio::net::UnixStream,
    future: F,
    deadline: tokio::time::Instant,
    control: Arc<StartupSqliteControl>,
    cancelling: Arc<AtomicBool>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<T, AssetStoreError>
where
    F: Future<Output = Result<T, AssetStoreError>>,
{
    if cancelling.load(Ordering::Acquire) || *shutdown.borrow() {
        control
            .stop(IngestStop::Cancelled)
            .map_err(|()| AssetStoreError::Internal)?;
    }
    tokio::pin!(future);
    let mut unexpected = [0_u8; 1];
    tokio::select! {
        result = &mut future => result,
        _ = tokio::time::sleep_until(deadline) => {
            control.stop(IngestStop::DeadlineReached).map_err(|()| AssetStoreError::Internal)?;
            future.await
        }
        _ = stream.read(&mut unexpected) => {
            control.stop(IngestStop::Cancelled).map_err(|()| AssetStoreError::Internal)?;
            future.await
        }
        _ = shutdown.changed() => {
            control.stop(IngestStop::Cancelled).map_err(|()| AssetStoreError::Internal)?;
            future.await
        }
    }
}

fn decode_query_timeout(milliseconds: u64) -> Result<Duration, ErrorCode> {
    decode_bounded_operation_timeout(milliseconds, MAX_QUERY_OPERATION_TIMEOUT)
}

fn decode_bounded_operation_timeout(
    milliseconds: u64,
    maximum: Duration,
) -> Result<Duration, ErrorCode> {
    let timeout = Duration::from_millis(milliseconds);
    if timeout < Duration::from_millis(100) || timeout > maximum {
        Err(ErrorCode::ValidationError)
    } else {
        Ok(timeout)
    }
}

fn parse_canonical_id<T>(value: &str) -> Result<Id<T>, ErrorCode> {
    let id = Id::<T>::from_str(value).map_err(|_| ErrorCode::ValidationError)?;
    if id.to_string() == value {
        Ok(id)
    } else {
        Err(ErrorCode::ValidationError)
    }
}

fn decode_revision(value: u64) -> Result<mengxia_types::RevisionNo, ErrorCode> {
    (value != 0)
        .then_some(mengxia_types::RevisionNo::new(value))
        .ok_or(ErrorCode::ValidationError)
}

fn query_error_response(
    error: AssetStoreError,
    correlation_id: &str,
) -> Result<CoreResponse, ErrorCode> {
    let retry = match error {
        AssetStoreError::Validation | AssetStoreError::NotFound | AssetStoreError::Conflict => {
            RetryAction::None
        }
        AssetStoreError::StorageBusy => RetryAction::SameCommand,
        AssetStoreError::Backpressure
        | AssetStoreError::OperationCancelled
        | AssetStoreError::DeadlineExceeded => RetryAction::FreshCommand,
        AssetStoreError::StorageIo => RetryAction::FreshCommand,
        AssetStoreError::StorageCorruption
        | AssetStoreError::StorageConfiguration
        | AssetStoreError::IdGenerationUnavailable => RetryAction::OperatorOrRuntimeAction,
        AssetStoreError::ShuttingDown | AssetStoreError::Internal => {
            return Err(ErrorCode::InternalError);
        }
        _ => return Err(ErrorCode::InternalError),
    };
    operation_error_response(error.error_code(), retry, correlation_id)
        .map_err(|_| ErrorCode::InternalError)
}

fn bound_task_009_response(
    response: CoreResponse,
    correlation_id: &str,
) -> Result<CoreResponse, ErrorCode> {
    if core_response_encoded_len(&response) <= TASK_009_MAX_RESPONSE_BYTES {
        Ok(response)
    } else {
        operation_error_response(
            ErrorCode::ValidationError,
            RetryAction::None,
            correlation_id,
        )
        .map_err(|_| ErrorCode::InternalError)
    }
}

const fn query_fatal_code(error: AssetStoreError) -> Option<ErrorCode> {
    match error {
        AssetStoreError::StorageIo
        | AssetStoreError::StorageCorruption
        | AssetStoreError::Internal => Some(error.error_code()),
        AssetStoreError::IdGenerationUnavailable
        | AssetStoreError::Validation
        | AssetStoreError::NotFound
        | AssetStoreError::Conflict
        | AssetStoreError::StorageBusy
        | AssetStoreError::StorageConfiguration
        | AssetStoreError::Backpressure
        | AssetStoreError::OperationCancelled
        | AssetStoreError::DeadlineExceeded
        | AssetStoreError::ShuttingDown => None,
        _ => Some(ErrorCode::InternalError),
    }
}

fn asset_summary_response(
    summary: &mengxia_ports::AssetSummaryView,
    protocol_minor: u32,
) -> mengxia_core_proto::AssetSummary {
    let lifecycle = match summary.lifecycle() {
        AssetLifecycle::Active => mengxia_core_proto::AssetLifecycleValue::Active,
        AssetLifecycle::Retired => mengxia_core_proto::AssetLifecycleValue::Retired,
    };
    mengxia_core_proto::AssetSummary {
        asset_id: summary.asset_id().to_string(),
        kind: summary.kind().as_str().to_owned(),
        lifecycle: lifecycle as i32,
        revision: summary.revision().get(),
        created_at_seconds: summary.created_at().unix_seconds(),
        created_at_nanos: summary.created_at().subsec_nanoseconds(),
        creation_commit_sequence: summary.creation_commit_sequence(),
        updated_at_seconds: (protocol_minor >= mengxia_core_proto::TASK_009_PROTOCOL_MINOR)
            .then_some(summary.updated_at().unix_seconds()),
        updated_at_nanos: (protocol_minor >= mengxia_core_proto::TASK_009_PROTOCOL_MINOR)
            .then_some(summary.updated_at().subsec_nanoseconds()),
    }
}

fn revision_custody_response(custody: RevisionCustody) -> mengxia_core_proto::RevisionCustodyValue {
    match custody {
        RevisionCustody::Managed => mengxia_core_proto::RevisionCustodyValue::Managed,
        RevisionCustody::Unmanaged => mengxia_core_proto::RevisionCustodyValue::Unmanaged,
    }
}

fn asset_member_response(
    member: &mengxia_ports::AssetMemberView,
) -> mengxia_core_proto::AssetMemberView {
    let (location_id, lifecycle, custody, durability) = match member.location() {
        Some(location) => (
            Some(location.location_id().to_string()),
            location_lifecycle_response(location.lifecycle()) as i32,
            location_custody_response(location.custody()) as i32,
            location_durability_response(location.durability()) as i32,
        ),
        None => (None, 0, 0, 0),
    };
    mengxia_core_proto::AssetMemberView {
        representation_id: member.representation_id().to_string(),
        representation_purpose: member.representation_purpose().as_str().to_owned(),
        resource_id: member.resource_id().to_string(),
        resource_kind: member.resource_kind().as_str().to_owned(),
        member_ordinal: member.member_ordinal(),
        logical_name: member.logical_name().as_str().to_owned(),
        blob_sha256: member.blob_digest().to_bytes().to_vec(),
        byte_length: member.byte_length(),
        media_type: member.media_type().map(|value| value.as_str().to_owned()),
        location_id,
        location_lifecycle: lifecycle,
        location_custody: custody,
        location_durability: durability,
    }
}

fn location_lifecycle_response(
    value: LocationLifecycle,
) -> mengxia_core_proto::LocationLifecycleValue {
    match value {
        LocationLifecycle::Available => mengxia_core_proto::LocationLifecycleValue::Available,
        LocationLifecycle::Corrupt => mengxia_core_proto::LocationLifecycleValue::Corrupt,
        LocationLifecycle::Missing => mengxia_core_proto::LocationLifecycleValue::Missing,
        LocationLifecycle::Removed => mengxia_core_proto::LocationLifecycleValue::Removed,
    }
}

fn location_custody_response(value: LocationCustody) -> mengxia_core_proto::LocationCustodyValue {
    match value {
        LocationCustody::Managed => mengxia_core_proto::LocationCustodyValue::Managed,
        LocationCustody::Unmanaged => mengxia_core_proto::LocationCustodyValue::Unmanaged,
    }
}

fn location_durability_response(
    value: LocationDurability,
) -> mengxia_core_proto::LocationDurabilityValue {
    match value {
        LocationDurability::Durable => mengxia_core_proto::LocationDurabilityValue::Durable,
        LocationDurability::Unknown => mengxia_core_proto::LocationDurabilityValue::Unknown,
    }
}

async fn await_ingest_with_watcher<T: Send + 'static>(
    stream: &mut tokio::net::UnixStream,
    mut worker: tokio::task::JoinHandle<T>,
    deadline: tokio::time::Instant,
    peer_stopped: Arc<AtomicBool>,
) -> Result<T, ErrorCode> {
    let mut unexpected = [0_u8; 1];
    tokio::select! {
        joined = &mut worker => joined.map_err(|_| ErrorCode::InternalError),
        _ = tokio::time::sleep_until(deadline) => {
            peer_stopped.store(true, Ordering::Release);
            worker.await.map_err(|_| ErrorCode::InternalError)
        }
        _ = stream.read(&mut unexpected) => {
            peer_stopped.store(true, Ordering::Release);
            worker.await.map_err(|_| ErrorCode::InternalError)
        }
    }
}

fn decode_ingest_request(
    request: CoreRequest,
    max_timeout: Duration,
) -> Result<(AppIngestRequest, Duration), ErrorCode> {
    let request = match request.operation {
        Some(core_request::Operation::IngestAssetCopy(request)) => request,
        Some(_) | None => return Err(ErrorCode::ValidationError),
    };
    if request.mode != IngestMode::Copy as i32
        || !(1..=1023).contains(&request.source_path.len())
        || request.source_path.contains(&0)
        || !normalized_absolute_bytes(&request.source_path)
    {
        return Err(ErrorCode::ValidationError);
    }
    let command_id = Id::<PersistedCommand>::from_str(&request.command_id)
        .map_err(|_| ErrorCode::ValidationError)?;
    if command_id.to_string() != request.command_id {
        return Err(ErrorCode::ValidationError);
    }
    let expected_digest = request
        .expected_sha256
        .map(|bytes| {
            <[u8; 32]>::try_from(bytes)
                .map(Sha256Digest::from_bytes)
                .map_err(|_| ErrorCode::ValidationError)
        })
        .transpose()?;
    let timeout = Duration::from_millis(request.operation_timeout_ms);
    if timeout < Duration::from_millis(100) || timeout > max_timeout {
        return Err(ErrorCode::ValidationError);
    }
    let app = AppIngestRequest::new(
        command_id,
        PathBuf::from(OsString::from_vec(request.source_path)),
        AssetKind::new(request.asset_kind).map_err(|_| ErrorCode::ValidationError)?,
        ContentKind::new(request.content_kind).map_err(|_| ErrorCode::ValidationError)?,
        RepresentationPurpose::new(request.representation_purpose)
            .map_err(|_| ErrorCode::ValidationError)?,
        ResourceKind::new(request.resource_kind).map_err(|_| ErrorCode::ValidationError)?,
        LogicalName::new(request.logical_name).map_err(|_| ErrorCode::ValidationError)?,
        expected_digest,
    );
    Ok((app, timeout))
}

fn normalized_absolute_bytes(path: &[u8]) -> bool {
    path.first() == Some(&b'/')
        && path.len() > 1
        && !path.ends_with(b"/")
        && path[1..]
            .split(|byte| *byte == b'/')
            .all(|component| !component.is_empty() && component != b"." && component != b"..")
}

const fn retry_action(retry: IngestRetry) -> RetryAction {
    match retry {
        IngestRetry::No => RetryAction::None,
        IngestRetry::SameCommandAfterBoundedDelay => RetryAction::SameCommand,
        IngestRetry::FreshCommandAfterBoundedDelay => RetryAction::FreshCommand,
        IngestRetry::AfterSourceStabilizesWithSameCommand => RetryAction::SourceStableSameCommand,
        IngestRetry::AfterSourceStabilizesWithFreshCommand => RetryAction::SourceStableFreshCommand,
        IngestRetry::AfterOperatorOrRuntimeAction => RetryAction::OperatorOrRuntimeAction,
    }
}

async fn shutdown_signal() -> Result<(), ()> {
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
        .map_err(|_| ())?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| ())?;
    tokio::select! {
        _ = interrupt.recv() => Ok(()),
        _ = terminate.recv() => Ok(()),
    }
}

#[derive(Default)]
struct ServeCli {
    library_config: Option<OsString>,
    library_root: Option<OsString>,
    blob_root: Option<OsString>,
    endpoint: Option<OsString>,
    frame: Option<OsString>,
    depth: Option<OsString>,
    timeout: Option<OsString>,
    pending: Option<OsString>,
    max_sessions: Option<OsString>,
    max_operation_timeout: Option<OsString>,
    max_verify_operation_timeout: Option<OsString>,
    max_materialize_operation_timeout: Option<OsString>,
    max_metadata_operation_timeout: Option<OsString>,
    log_level: Option<OsString>,
    shutdown_timeout: Option<OsString>,
    storage_io: Option<OsString>,
    hash: Option<OsString>,
    max_ingests: Option<OsString>,
    stream_buffer: Option<OsString>,
    max_ingest_bytes: Option<OsString>,
    max_staging_bytes: Option<OsString>,
    min_free_bytes: Option<OsString>,
    min_free_percent: Option<OsString>,
    db_write_queue: Option<OsString>,
    db_read_connections: Option<OsString>,
    db_busy_timeout: Option<OsString>,
}

enum Command {
    Help,
    Serve(Box<ServeCli>),
}

fn parse_command(args: Vec<OsString>) -> Result<Command, ErrorCode> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(Command::Help);
    }
    if args.first().is_none_or(|arg| arg != "serve") {
        return Err(ErrorCode::ValidationError);
    }
    let mut cli = ServeCli::default();
    let mut index = 1;
    while index < args.len() {
        let option = args[index].to_str().ok_or(ErrorCode::ValidationError)?;
        let value = args.get(index + 1).ok_or(ErrorCode::ValidationError)?;
        let slot = match option {
            "--library-root" => &mut cli.library_root,
            "--library-config" => &mut cli.library_config,
            "--blob-root" => &mut cli.blob_root,
            "--client-endpoint" => &mut cli.endpoint,
            "--max-frame-bytes" => &mut cli.frame,
            "--max-decode-depth" => &mut cli.depth,
            "--client-handshake-timeout-ms" => &mut cli.timeout,
            "--max-pending-handshakes" => &mut cli.pending,
            "--max-client-sessions" => &mut cli.max_sessions,
            "--max-ingest-operation-timeout-ms" => &mut cli.max_operation_timeout,
            "--max-verify-operation-timeout-ms" => &mut cli.max_verify_operation_timeout,
            "--max-materialize-operation-timeout-ms" => &mut cli.max_materialize_operation_timeout,
            "--max-metadata-operation-timeout-ms" => &mut cli.max_metadata_operation_timeout,
            "--log-level" => &mut cli.log_level,
            "--ingest-shutdown-timeout-ms" => &mut cli.shutdown_timeout,
            "--storage-io-concurrency" => &mut cli.storage_io,
            "--hash-concurrency" => &mut cli.hash,
            "--max-concurrent-ingests" => &mut cli.max_ingests,
            "--stream-buffer-bytes" => &mut cli.stream_buffer,
            "--max-ingest-bytes" => &mut cli.max_ingest_bytes,
            "--max-staging-bytes" => &mut cli.max_staging_bytes,
            "--min-free-bytes" => &mut cli.min_free_bytes,
            "--min-free-percent" => &mut cli.min_free_percent,
            "--db-write-queue" => &mut cli.db_write_queue,
            "--db-read-connections" => &mut cli.db_read_connections,
            "--db-busy-timeout-ms" => &mut cli.db_busy_timeout,
            _ => return Err(ErrorCode::ValidationError),
        };
        if slot.is_some() {
            return Err(ErrorCode::ValidationError);
        }
        *slot = Some(value.clone());
        index += 2;
    }
    Ok(Command::Serve(Box::new(cli)))
}

struct DaemonConfig {
    store: StoreConfig,
    blob: BlobStorageConfig,
    endpoint: PathBuf,
    limits: HandshakeLimits,
    max_pending: usize,
    max_sessions: usize,
    operation_limits: OperationLimits,
    max_operation_timeout: Duration,
    task_008: Task008RuntimeConfig,
    task_009: Task009RuntimeConfig,
    shutdown_timeout: Duration,
}

fn resolve(cli: ServeCli) -> Result<DaemonConfig, ErrorCode> {
    let mut cli = cli;
    let environment = DaemonEnvironment::capture();
    let selector = cli
        .library_config
        .take()
        .or_else(|| env::var_os("MENGXIA_LIBRARY_CONFIG"));
    let library = match selector {
        Some(path) => {
            let bytes = read_library_config(&PathBuf::from(path)).map_err(authority_code)?;
            let document = LibraryConfigDocument::parse(&bytes)
                .map_err(|_| ErrorCode::StorageConfigurationError)?;
            DaemonLibraryConfig::from_document(&document)?
        }
        None => DaemonLibraryConfig::default(),
    };
    resolve_from_layers(cli, environment, library)
}

#[derive(Default)]
struct DaemonLibraryConfig {
    library_root: Option<PathBuf>,
    endpoint: Option<PathBuf>,
    frame_bytes: Option<OsString>,
    decode_depth: Option<OsString>,
    handshake_timeout_ms: Option<OsString>,
    max_pending_handshakes: Option<OsString>,
    write_queue: Option<OsString>,
    read_connections: Option<OsString>,
    busy_timeout_ms: Option<OsString>,
    blob_root: Option<PathBuf>,
    storage_io: Option<OsString>,
    hash: Option<OsString>,
    max_ingests: Option<OsString>,
    stream_buffer: Option<OsString>,
    max_ingest_bytes: Option<OsString>,
    max_staging_bytes: Option<OsString>,
    min_free_bytes: Option<OsString>,
    min_free_percent: Option<OsString>,
    max_sessions: Option<OsString>,
    max_operation_timeout: Option<OsString>,
    max_verify_operation_timeout: Option<OsString>,
    max_materialize_operation_timeout: Option<OsString>,
    max_metadata_operation_timeout: Option<OsString>,
    log_level: Option<OsString>,
    shutdown_timeout: Option<OsString>,
}

impl DaemonLibraryConfig {
    fn from_document(document: &LibraryConfigDocument) -> Result<Self, ErrorCode> {
        Ok(Self {
            library_root: library_path(document, LibraryConfigKey::LibraryRoot),
            endpoint: library_path(document, LibraryConfigKey::ClientEndpoint),
            frame_bytes: library_raw(document, LibraryConfigKey::MaxFrameBytes),
            decode_depth: library_raw(document, LibraryConfigKey::MaxDecodeDepth),
            handshake_timeout_ms: library_raw(document, LibraryConfigKey::ClientHandshakeTimeoutMs),
            max_pending_handshakes: library_raw(document, LibraryConfigKey::MaxPendingHandshakes),
            write_queue: library_raw(document, LibraryConfigKey::DbWriteQueue),
            read_connections: library_raw(document, LibraryConfigKey::DbReadConnections),
            busy_timeout_ms: library_raw(document, LibraryConfigKey::DbBusyTimeoutMs),
            blob_root: library_path(document, LibraryConfigKey::BlobRoot),
            storage_io: library_raw(document, LibraryConfigKey::StorageIoConcurrency),
            hash: library_raw(document, LibraryConfigKey::HashConcurrency),
            max_ingests: library_raw(document, LibraryConfigKey::MaxConcurrentIngests),
            stream_buffer: library_raw(document, LibraryConfigKey::StreamBufferBytes),
            max_ingest_bytes: library_raw(document, LibraryConfigKey::MaxIngestBytes),
            max_staging_bytes: library_raw(document, LibraryConfigKey::MaxStagingBytes),
            min_free_bytes: library_raw(document, LibraryConfigKey::MinFreeBytes),
            min_free_percent: library_raw(document, LibraryConfigKey::MinFreePercent),
            max_sessions: library_raw(document, LibraryConfigKey::MaxClientSessions),
            max_operation_timeout: library_raw(
                document,
                LibraryConfigKey::MaxIngestOperationTimeoutMs,
            ),
            max_verify_operation_timeout: library_raw(
                document,
                LibraryConfigKey::MaxVerifyOperationTimeoutMs,
            ),
            max_materialize_operation_timeout: library_raw(
                document,
                LibraryConfigKey::MaxMaterializeOperationTimeoutMs,
            ),
            max_metadata_operation_timeout: library_raw(
                document,
                LibraryConfigKey::MaxMetadataOperationTimeoutMs,
            ),
            log_level: library_raw(document, LibraryConfigKey::LogLevel),
            shutdown_timeout: library_raw(document, LibraryConfigKey::IngestShutdownTimeoutMs),
        })
    }
}

fn library_path(document: &LibraryConfigDocument, key: LibraryConfigKey) -> Option<PathBuf> {
    document
        .value(key)
        .map(|value| PathBuf::from(OsString::from_vec(value.to_vec())))
}

fn library_raw(document: &LibraryConfigDocument, key: LibraryConfigKey) -> Option<OsString> {
    document
        .value(key)
        .map(|value| OsString::from_vec(value.to_vec()))
}

#[derive(Default)]
struct DaemonEnvironment {
    library_root: Option<OsString>,
    endpoint: Option<OsString>,
    frame_bytes: Option<OsString>,
    decode_depth: Option<OsString>,
    handshake_timeout_ms: Option<OsString>,
    max_pending_handshakes: Option<OsString>,
    write_queue: Option<OsString>,
    read_connections: Option<OsString>,
    busy_timeout_ms: Option<OsString>,
    blob_root: Option<OsString>,
    storage_io: Option<OsString>,
    hash: Option<OsString>,
    max_ingests: Option<OsString>,
    stream_buffer: Option<OsString>,
    max_ingest_bytes: Option<OsString>,
    max_staging_bytes: Option<OsString>,
    min_free_bytes: Option<OsString>,
    min_free_percent: Option<OsString>,
    max_sessions: Option<OsString>,
    max_operation_timeout: Option<OsString>,
    max_verify_operation_timeout: Option<OsString>,
    max_materialize_operation_timeout: Option<OsString>,
    max_metadata_operation_timeout: Option<OsString>,
    log_level: Option<OsString>,
    shutdown_timeout: Option<OsString>,
    platform_temp_root: PathBuf,
}

impl DaemonEnvironment {
    fn capture() -> Self {
        Self {
            library_root: env::var_os("MENGXIA_LIBRARY_ROOT"),
            endpoint: env::var_os("MENGXIA_CLIENT_ENDPOINT"),
            frame_bytes: env::var_os("MENGXIA_MAX_FRAME_BYTES"),
            decode_depth: env::var_os("MENGXIA_MAX_DECODE_DEPTH"),
            handshake_timeout_ms: env::var_os("MENGXIA_CLIENT_HANDSHAKE_TIMEOUT_MS"),
            max_pending_handshakes: env::var_os("MENGXIA_MAX_PENDING_HANDSHAKES"),
            write_queue: env::var_os("MENGXIA_DB_WRITE_QUEUE"),
            read_connections: env::var_os("MENGXIA_DB_READ_CONNECTIONS"),
            busy_timeout_ms: env::var_os("MENGXIA_DB_BUSY_TIMEOUT_MS"),
            blob_root: env::var_os("MENGXIA_BLOB_ROOT"),
            storage_io: env::var_os("MENGXIA_STORAGE_IO_CONCURRENCY"),
            hash: env::var_os("MENGXIA_HASH_CONCURRENCY"),
            max_ingests: env::var_os("MENGXIA_MAX_CONCURRENT_INGESTS"),
            stream_buffer: env::var_os("MENGXIA_STREAM_BUFFER_BYTES"),
            max_ingest_bytes: env::var_os("MENGXIA_MAX_INGEST_BYTES"),
            max_staging_bytes: env::var_os("MENGXIA_MAX_STAGING_BYTES"),
            min_free_bytes: env::var_os("MENGXIA_MIN_FREE_BYTES"),
            min_free_percent: env::var_os("MENGXIA_MIN_FREE_PERCENT"),
            max_sessions: env::var_os("MENGXIA_MAX_CLIENT_SESSIONS"),
            max_operation_timeout: env::var_os("MENGXIA_MAX_INGEST_OPERATION_TIMEOUT_MS"),
            max_verify_operation_timeout: env::var_os("MENGXIA_MAX_VERIFY_OPERATION_TIMEOUT_MS"),
            max_materialize_operation_timeout: env::var_os(
                "MENGXIA_MAX_MATERIALIZE_OPERATION_TIMEOUT_MS",
            ),
            max_metadata_operation_timeout: env::var_os(
                "MENGXIA_MAX_METADATA_OPERATION_TIMEOUT_MS",
            ),
            log_level: env::var_os("MENGXIA_LOG_LEVEL"),
            shutdown_timeout: env::var_os("MENGXIA_INGEST_SHUTDOWN_TIMEOUT_MS"),
            platform_temp_root: env::temp_dir(),
        }
    }
}

fn resolve_from_layers(
    cli: ServeCli,
    environment: DaemonEnvironment,
    library: DaemonLibraryConfig,
) -> Result<DaemonConfig, ErrorCode> {
    let (library_raw, library_source, blob_library_source) = selected_required(
        cli.library_root,
        environment.library_root,
        library.library_root.clone(),
    )?;
    let library_root = PathBuf::from(library_raw);
    let blob_library_root = library_root.clone();
    let endpoint = cli
        .endpoint
        .map(PathBuf::from)
        .or_else(|| environment.endpoint.map(PathBuf::from))
        .or(library.endpoint)
        .map_or_else(
            || {
                std::fs::canonicalize(environment.platform_temp_root)
                    .map(|root| root.join("mengxia-runtime-v1/client.sock"))
            },
            Ok,
        )
        .map_err(|_| ErrorCode::ValidationError)?;
    validate_runtime_endpoint_path(&endpoint).map_err(|_| ErrorCode::ValidationError)?;

    let (frame, _) = select_u64(
        cli.frame,
        environment.frame_bytes,
        library.frame_bytes,
        4 * 1024 * 1024,
    )?;
    let frame = u32::try_from(frame)
        .ok()
        .and_then(|value| FrameLimit::new(value).ok())
        .ok_or(ErrorCode::ValidationError)?;
    let (depth, _) = select_u64(
        cli.depth,
        environment.decode_depth,
        library.decode_depth,
        64,
    )?;
    let depth = u8::try_from(depth)
        .ok()
        .and_then(|value| DecodeDepth::new(value).ok())
        .ok_or(ErrorCode::ValidationError)?;
    if depth.get() < TASK_009_MIN_OPERATION_DECODE_DEPTH {
        return Err(ErrorCode::ValidationError);
    }
    let (timeout_ms, _) = select_u64(
        cli.timeout,
        environment.handshake_timeout_ms,
        library.handshake_timeout_ms,
        5_000,
    )?;
    let limits = HandshakeLimits::new(frame, depth, Duration::from_millis(timeout_ms))
        .map_err(|error| error.code())?;
    let operation_limits = OperationLimits::new(frame, depth).map_err(|error| error.code())?;
    let (max_pending, _) = select_u64(
        cli.pending,
        environment.max_pending_handshakes,
        library.max_pending_handshakes,
        32,
    )?;
    let max_pending = usize::try_from(max_pending).map_err(|_| ErrorCode::ValidationError)?;
    if !(1..=256).contains(&max_pending) {
        return Err(ErrorCode::ValidationError);
    }
    let (max_sessions, _) = select_u64(
        cli.max_sessions,
        environment.max_sessions,
        library.max_sessions,
        32,
    )?;
    let max_sessions = usize::try_from(max_sessions).map_err(|_| ErrorCode::ValidationError)?;
    if !(1..=256).contains(&max_sessions) {
        return Err(ErrorCode::ValidationError);
    }
    let (max_operation_timeout_ms, _) = select_u64(
        cli.max_operation_timeout,
        environment.max_operation_timeout,
        library.max_operation_timeout,
        86_400_000,
    )?;
    if !(100..=86_400_000).contains(&max_operation_timeout_ms) {
        return Err(ErrorCode::ValidationError);
    }
    let log_level = select_raw(cli.log_level, environment.log_level, library.log_level);
    let max_verify_operation_timeout = select_raw(
        cli.max_verify_operation_timeout,
        environment.max_verify_operation_timeout,
        library.max_verify_operation_timeout,
    );
    let max_materialize_operation_timeout = select_raw(
        cli.max_materialize_operation_timeout,
        environment.max_materialize_operation_timeout,
        library.max_materialize_operation_timeout,
    );
    let task_008 = Task008RuntimeConfig::from_selected(
        log_level.as_deref().map(OsStr::as_bytes),
        max_verify_operation_timeout.as_deref().map(OsStr::as_bytes),
        max_materialize_operation_timeout
            .as_deref()
            .map(OsStr::as_bytes),
    )?;
    let max_metadata_operation_timeout = select_raw(
        cli.max_metadata_operation_timeout,
        environment.max_metadata_operation_timeout,
        library.max_metadata_operation_timeout,
    );
    let task_009 = Task009RuntimeConfig::from_selected(
        max_metadata_operation_timeout
            .as_deref()
            .map(OsStr::as_bytes),
    )?;
    let (shutdown_timeout_ms, _) = select_u64(
        cli.shutdown_timeout,
        environment.shutdown_timeout,
        library.shutdown_timeout,
        5_000,
    )?;
    if !(100..=30_000).contains(&shutdown_timeout_ms) {
        return Err(ErrorCode::ValidationError);
    }

    let (write_queue, write_queue_source) = select_u64(
        cli.db_write_queue,
        environment.write_queue,
        library.write_queue,
        256,
    )?;
    let (readers, readers_source) = select_u64(
        cli.db_read_connections,
        environment.read_connections,
        library.read_connections,
        4,
    )?;
    let (busy, busy_source) = select_u64(
        cli.db_busy_timeout,
        environment.busy_timeout_ms,
        library.busy_timeout_ms,
        5_000,
    )?;
    let resolved_store = ResolvedStoreConfig::from_selected(
        Some(library_root),
        library_source,
        usize::try_from(write_queue).map_err(|_| ErrorCode::ValidationError)?,
        write_queue_source,
        usize::try_from(readers).map_err(|_| ErrorCode::ValidationError)?,
        readers_source,
        busy,
        busy_source,
    );
    let (blob_root, blob_root_source) = if let Some(value) = cli.blob_root {
        (PathBuf::from(value), BlobConfigSource::Cli)
    } else if let Some(value) = environment.blob_root {
        (PathBuf::from(value), BlobConfigSource::Environment)
    } else if let Some(value) = library.blob_root {
        (value, BlobConfigSource::Library)
    } else {
        (
            blob_library_root.join("storage"),
            BlobConfigSource::CompiledDefault,
        )
    };
    let (storage_io, storage_io_source) = select_blob_value(
        cli.storage_io,
        environment.storage_io,
        library.storage_io,
        2,
    )?;
    let (hash, hash_source) = select_blob_value(cli.hash, environment.hash, library.hash, 2)?;
    let (max_ingests, max_ingests_source) = select_blob_value(
        cli.max_ingests,
        environment.max_ingests,
        library.max_ingests,
        2,
    )?;
    let (stream_buffer, stream_buffer_source) = select_blob_value(
        cli.stream_buffer,
        environment.stream_buffer,
        library.stream_buffer,
        8 * 1024 * 1024,
    )?;
    let (max_ingest_bytes, max_ingest_source) = select_blob_value(
        cli.max_ingest_bytes,
        environment.max_ingest_bytes,
        library.max_ingest_bytes,
        1024 * 1024 * 1024 * 1024,
    )?;
    let (max_staging_bytes, max_staging_source) = select_blob_value(
        cli.max_staging_bytes,
        environment.max_staging_bytes,
        library.max_staging_bytes,
        2 * 1024 * 1024 * 1024 * 1024,
    )?;
    let (min_free_bytes, min_free_source) = select_blob_value(
        cli.min_free_bytes,
        environment.min_free_bytes,
        library.min_free_bytes,
        10 * 1024 * 1024 * 1024,
    )?;
    let (min_free_percent, min_free_percent_source) = select_blob_value(
        cli.min_free_percent,
        environment.min_free_percent,
        library.min_free_percent,
        5,
    )?;
    let blob = ResolvedBlobStorageConfig::from_selected(
        Some(blob_library_root),
        blob_library_source,
        Some(blob_root),
        blob_root_source,
        Some(storage_io),
        storage_io_source,
        Some(hash),
        hash_source,
        Some(max_ingests),
        max_ingests_source,
        Some(stream_buffer),
        stream_buffer_source,
        Some(max_ingest_bytes),
        max_ingest_source,
        Some(max_staging_bytes),
        max_staging_source,
        Some(min_free_bytes),
        min_free_source,
        Some(min_free_percent),
        min_free_percent_source,
    )
    .validate()
    .map_err(|_| ErrorCode::ValidationError)?;
    let store = resolved_store
        .with_migration_reserve(blob.min_free_bytes(), blob.min_free_percent())
        .validate()
        .map_err(|_| ErrorCode::ValidationError)?;
    Ok(DaemonConfig {
        store,
        blob,
        endpoint,
        limits,
        max_pending,
        max_sessions,
        operation_limits,
        max_operation_timeout: Duration::from_millis(max_operation_timeout_ms),
        task_008,
        task_009,
        shutdown_timeout: Duration::from_millis(shutdown_timeout_ms),
    })
}

fn select_raw(
    cli: Option<OsString>,
    environment: Option<OsString>,
    library: Option<OsString>,
) -> Option<OsString> {
    cli.or(environment).or(library)
}

fn select_blob_value(
    cli: Option<OsString>,
    environment: Option<OsString>,
    library: Option<OsString>,
    default: u64,
) -> Result<(String, BlobConfigSource), ErrorCode> {
    if let Some(value) = cli {
        Ok((
            value
                .into_string()
                .map_err(|_| ErrorCode::ValidationError)?,
            BlobConfigSource::Cli,
        ))
    } else if let Some(value) = environment {
        Ok((
            value
                .into_string()
                .map_err(|_| ErrorCode::ValidationError)?,
            BlobConfigSource::Environment,
        ))
    } else if let Some(value) = library {
        Ok((
            value
                .into_string()
                .map_err(|_| ErrorCode::ValidationError)?,
            BlobConfigSource::Library,
        ))
    } else {
        Ok((default.to_string(), BlobConfigSource::CompiledDefault))
    }
}

fn selected_required(
    cli: Option<OsString>,
    environment: Option<OsString>,
    library: Option<PathBuf>,
) -> Result<(OsString, ConfigSource, BlobConfigSource), ErrorCode> {
    if let Some(value) = cli {
        Ok((value, ConfigSource::Cli, BlobConfigSource::Cli))
    } else if let Some(value) = environment {
        Ok((
            value,
            ConfigSource::Environment,
            BlobConfigSource::Environment,
        ))
    } else if let Some(value) = library {
        Ok((
            value.into_os_string(),
            ConfigSource::Library,
            BlobConfigSource::Library,
        ))
    } else {
        Err(ErrorCode::ValidationError)
    }
}

fn select_u64(
    cli: Option<OsString>,
    environment: Option<OsString>,
    library: Option<OsString>,
    default: u64,
) -> Result<(u64, ConfigSource), ErrorCode> {
    if let Some(value) = cli {
        Ok((parse_ascii_u64(&value)?, ConfigSource::Cli))
    } else if let Some(value) = environment {
        Ok((parse_ascii_u64(&value)?, ConfigSource::Environment))
    } else if let Some(value) = library {
        Ok((parse_ascii_u64(&value)?, ConfigSource::Library))
    } else {
        Ok((default, ConfigSource::CompiledDefault))
    }
}

fn parse_ascii_u64(value: &OsStr) -> Result<u64, ErrorCode> {
    let text = value.to_str().ok_or(ErrorCode::ValidationError)?;
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ErrorCode::ValidationError);
    }
    text.parse().map_err(|_| ErrorCode::ValidationError)
}

fn authority_code(error: AuthorityError) -> ErrorCode {
    match error {
        AuthorityError::UnsafeConfiguration | AuthorityError::Contended => {
            ErrorCode::StorageConfigurationError
        }
        AuthorityError::Io => ErrorCode::StorageIoError,
        AuthorityError::ConflictingData => ErrorCode::StorageCorruption,
        _ => ErrorCode::InternalError,
    }
}

fn fail(code: ErrorCode, status: u8) -> ExitCode {
    eprintln!("MENGXIA_ERROR code={}", code.as_str());
    ExitCode::from(status)
}

#[cfg(test)]
#[test]
#[ignore = "requires the reviewed formal second-UID macOS runner"]
fn task_003_real_second_uid_peer_is_rejected_before_frame() {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::os::unix::net::{UnixListener as StdUnixListener, UnixStream as StdUnixStream};
    use std::process::Command as ProcessCommand;
    use std::time::Instant as StdInstant;

    const ROLE: &str = "MENGXIA_TASK003_TEST_ROLE";
    const ENDPOINT: &str = "MENGXIA_TASK003_TEST_ENDPOINT";
    const ACCOUNT: &str = "mengxia-task003-ci";

    if env::var_os(ROLE).as_deref() == Some(OsStr::new("second_uid_client")) {
        let endpoint = PathBuf::from(env::var_os(ENDPOINT).expect("formal endpoint is present"));
        let production_case = endpoint
            .parent()
            .and_then(|path| path.file_name())
            .is_some_and(|name| name == "mengxia-runtime-v1");
        match StdUnixStream::connect(&endpoint) {
            Err(_) if production_case => return,
            Ok(mut stream) if !production_case => {
                stream
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                stream.write_all(b"MENGXIA-TASK003-CANARY").unwrap();
                return;
            }
            _ => panic!("second-UID reachability did not match the expected branch"),
        }
    }

    let executable = env::current_exe().unwrap();
    let mut preflight = ProcessCommand::new("/usr/bin/sudo")
        .args([
            "-n",
            "-u",
            ACCOUNT,
            "--",
            "/usr/bin/env",
            "-i",
            "/bin/test",
            "-x",
        ])
        .arg(&executable)
        .spawn()
        .unwrap();
    wait_formal_child(&mut preflight, Duration::from_secs(5));

    let owner_home = fs::canonicalize(PathBuf::from(env::var_os("HOME").unwrap())).unwrap();
    let owner_root = owner_home.join(format!(".mengxia-task003-owner-{}", std::process::id()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&owner_root)
        .unwrap();
    let production_endpoint = owner_root.join("mengxia-runtime-v1/client.sock");
    let mut library_id = [0x5a; 16];
    library_id[6] = 0x7a;
    library_id[8] = 0x9a;
    let published = bind_runtime_endpoint(
        &production_endpoint,
        library_id,
        mengxia_platform_fs::effective_user_id(),
    )
    .unwrap();
    run_formal_child(&executable, ACCOUNT, &production_endpoint);
    published.cleanup().unwrap();
    fs::remove_dir_all(&owner_root).unwrap();

    let fixture_root = PathBuf::from(format!(
        "/private/tmp/mengxia-task003-peer-{}",
        std::process::id()
    ));
    fs::DirBuilder::new()
        .mode(0o777)
        .create(&fixture_root)
        .unwrap();
    fs::set_permissions(&fixture_root, fs::Permissions::from_mode(0o777)).unwrap();
    let fixture_endpoint = fixture_root.join("client.sock");
    let listener = StdUnixListener::bind(&fixture_endpoint).unwrap();
    fs::set_permissions(&fixture_endpoint, fs::Permissions::from_mode(0o666)).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut child = spawn_formal_child(&executable, ACCOUNT, &fixture_endpoint);
    let deadline = StdInstant::now() + Duration::from_secs(5);
    let accepted = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if let Some(status) = child.try_wait().unwrap() {
                    panic!("formal client exited before accept: {status}");
                }
                if StdInstant::now() >= deadline {
                    child.kill().unwrap();
                    let _ = child.wait();
                    panic!("formal accept exceeded its deadline");
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => panic!("formal listener failed"),
        }
    };
    accepted.set_nonblocking(true).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let limits = HandshakeLimits::new(
        FrameLimit::default(),
        DecodeDepth::new(3).unwrap(),
        Duration::from_millis(500),
    )
    .unwrap();
    let owner_uid = mengxia_platform_fs::effective_user_id();
    assert_eq!(
        runtime
            .block_on(async move {
                let mut accepted = tokio::net::UnixStream::from_std(accepted).unwrap();
                serve_handshake(&mut accepted, owner_uid, limits).await
            })
            .map_err(|error| error.code()),
        Err(ErrorCode::AuthenticationError)
    );
    wait_formal_child(&mut child, Duration::from_secs(5));
    drop(listener);
    fs::remove_file(&fixture_endpoint).unwrap();
    fs::remove_dir(&fixture_root).unwrap();
}

#[cfg(test)]
fn spawn_formal_child(
    executable: &std::path::Path,
    account: &str,
    endpoint: &std::path::Path,
) -> std::process::Child {
    let role = "MENGXIA_TASK003_TEST_ROLE=second_uid_client".to_owned();
    let endpoint = format!(
        "MENGXIA_TASK003_TEST_ENDPOINT={}",
        endpoint.to_str().unwrap()
    );
    std::process::Command::new("/usr/bin/sudo")
        .args(["-n", "-u", account, "--", "/usr/bin/env", "-i"])
        .arg(role)
        .arg(endpoint)
        .arg(executable)
        .args([
            "task_003_real_second_uid_peer_is_rejected_before_frame",
            "--exact",
            "--ignored",
            "--nocapture",
        ])
        .spawn()
        .unwrap()
}

#[cfg(test)]
fn run_formal_child(executable: &std::path::Path, account: &str, endpoint: &std::path::Path) {
    let mut child = spawn_formal_child(executable, account, endpoint);
    wait_formal_child(&mut child, std::time::Duration::from_secs(5));
}

#[cfg(test)]
fn wait_formal_child(child: &mut std::process::Child, timeout: std::time::Duration) {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        match child.try_wait().unwrap() {
            Some(status) => {
                assert!(status.success(), "formal second-UID child failed");
                return;
            }
            None if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            None => {
                child.kill().unwrap();
                let _ = child.wait();
                panic!("formal second-UID child exceeded its deadline");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    use std::path::PathBuf;
    use std::process::{Command as ProcessCommand, Stdio};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use mengxia_app::CoreLogLevel;
    use mengxia_core_proto::{
        CoreRequest, CoreResponse, DecodeDepth, HandshakeLimits, OperationLimits, RetryAction,
        core_request, core_response, request_single_command, request_task_008_command,
        request_task_009_command,
    };
    use mengxia_framing::FrameLimit;
    use mengxia_storage_local::BlobConfigSource;
    use mengxia_store_sqlite::ConfigSource;
    use mengxia_types::ErrorCode;

    use super::{
        AppReadinessBlockReason, Command, DaemonEnvironment, DaemonLibraryConfig, HealthBaseline,
        IngestMode, ServeCli, StartupSqliteControl, await_ingest_with_watcher,
        decode_ingest_request, fatal_shutdown, health_state, parse_ascii_u64, parse_command,
        resolve_from_layers, selected_required, serve, status_response, take_last_owner,
    };

    const FATAL_CHILD_ROLE: &str = "MENGXIA_TASK007_FATAL_CHILD_ROLE";
    const CRASH_CHILD_ROLE: &str = "MENGXIA_TASK007_CRASH_CHILD_ROLE";
    const CRASH_LIBRARY: &str = "MENGXIA_TASK007_CRASH_LIBRARY";
    const CRASH_ENDPOINT: &str = "MENGXIA_TASK007_CRASH_ENDPOINT";

    struct BlockingDrop;

    impl Drop for BlockingDrop {
        fn drop(&mut self) {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        }
    }

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn exact_daemon_grammar_accepts_only_help_or_serve() {
        assert!(matches!(
            parse_command(args(&["--help"])),
            Ok(Command::Help)
        ));
        assert!(matches!(
            parse_command(args(&[
                "serve",
                "--library-root",
                "/private/tmp/Library",
                "--max-pending-handshakes",
                "32",
            ])),
            Ok(Command::Serve(_))
        ));
        for invalid in [
            args(&[]),
            args(&["serve", "--help"]),
            args(&["serve", "--library-root=/tmp/x"]),
            args(&["serve", "--library-root"]),
            args(&[
                "serve",
                "--library-root",
                "/tmp/a",
                "--library-root",
                "/tmp/b",
            ]),
            args(&["unknown"]),
        ] {
            assert_eq!(
                parse_command(invalid).err(),
                Some(ErrorCode::ValidationError)
            );
        }
    }

    struct CountingInterrupt(Arc<AtomicUsize>);

    impl mengxia_ports::SqliteInterrupt for CountingInterrupt {
        fn interrupt(&self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn startup_sqlite_control_interrupts_once_and_preserves_the_first_cause() {
        use mengxia_ports::{
            IngestControl as _, IngestDirective, IngestStop, InterruptibleSqliteControl as _,
        };

        let calls = Arc::new(AtomicUsize::new(0));
        let control = StartupSqliteControl::new();
        assert_eq!(
            control.register_interrupt(Box::new(CountingInterrupt(Arc::clone(&calls)))),
            Ok(IngestDirective::Continue)
        );
        assert_eq!(control.stop(IngestStop::DeadlineReached), Ok(true));
        assert_eq!(control.stop(IngestStop::Cancelled), Ok(false));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(
            control.checkpoint(),
            IngestDirective::Stop(IngestStop::DeadlineReached)
        );
        assert_eq!(control.clear_interrupt(), Ok(()));
        assert_eq!(
            control.register_interrupt(Box::new(CountingInterrupt(Arc::clone(&calls)))),
            Ok(IngestDirective::Stop(IngestStop::DeadlineReached))
        );
        assert_eq!(
            control.clear_interrupt(),
            Err(mengxia_ports::SqliteInterruptControlError::RegistrationConflict)
        );
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[tokio::test]
    async fn sqlite_query_deadline_interrupts_and_joins_the_owned_future() {
        use mengxia_ports::{
            AssetStoreError, IngestControl as _, IngestDirective, IngestStop,
            InterruptibleSqliteControl as _,
        };

        let calls = Arc::new(AtomicUsize::new(0));
        let control = Arc::new(StartupSqliteControl::new());
        assert_eq!(
            control.register_interrupt(Box::new(CountingInterrupt(Arc::clone(&calls)))),
            Ok(IngestDirective::Continue)
        );
        let future_control = Arc::clone(&control);
        let future = async move {
            loop {
                match future_control.checkpoint() {
                    IngestDirective::Continue => tokio::task::yield_now().await,
                    IngestDirective::Stop(IngestStop::DeadlineReached) => {
                        future_control.clear_interrupt().unwrap();
                        return Err::<(), _>(AssetStoreError::DeadlineExceeded);
                    }
                    IngestDirective::Stop(IngestStop::Cancelled) => {
                        return Err::<(), _>(AssetStoreError::OperationCancelled);
                    }
                }
            }
        };
        let (mut server, _client) = tokio::net::UnixStream::pair().unwrap();
        let (_shutdown_sender, shutdown) = tokio::sync::watch::channel(false);
        let result = super::await_sqlite_query(
            &mut server,
            future,
            tokio::time::Instant::now() + Duration::from_millis(10),
            Arc::clone(&control),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            shutdown,
        )
        .await;
        assert_eq!(result, Err(AssetStoreError::DeadlineExceeded));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn startup_health_is_status_only_until_classification_publishes_ready() {
        let baseline = HealthBaseline {
            staging_orphan_count: 2,
            staging_orphan_bytes: 17,
            local_backend_matches: true,
        };
        let pending = health_state(
            baseline,
            AppReadinessBlockReason::LocalRecoveryPending,
            false,
            0,
        )
        .unwrap();
        let ready = health_state(baseline, AppReadinessBlockReason::None, true, 3).unwrap();

        let pending = match status_response(pending).response.unwrap() {
            core_response::Response::GetLibraryStatus(status) => status,
            _ => panic!("status response variant"),
        };
        assert_eq!(
            pending.readiness,
            mengxia_core_proto::CoreReadiness::NotReady as i32
        );
        assert_eq!(
            pending.readiness_block_reason,
            mengxia_core_proto::ReadinessBlockReason::LocalRecoveryPending as i32
        );
        assert!(!pending.can_ingest);
        assert!(!pending.recovery_observation_available);

        let ready = match status_response(ready).response.unwrap() {
            core_response::Response::GetLibraryStatus(status) => status,
            _ => panic!("status response variant"),
        };
        assert_eq!(
            ready.readiness,
            mengxia_core_proto::CoreReadiness::Ready as i32
        );
        assert_eq!(ready.recovery_required_command_count, 3);
        assert!(ready.can_ingest);
        assert!(ready.recovery_observation_available);
    }

    #[test]
    fn task_008_list_and_inspect_are_served_through_protocol_1_2_after_readiness() {
        let home = fs::canonicalize(PathBuf::from(std::env::var_os("HOME").unwrap())).unwrap();
        let base = home.join(format!(".mengxia-task008-query-e2e-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        let library = base.join("Library");
        let endpoint = base.join("runtime/mengxia-runtime-v1/client.sock");
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(endpoint.parent().unwrap().parent().unwrap())
            .unwrap();
        let ready = base.join("unused.ready");
        let mut daemon = spawn_crash_daemon(&library, &endpoint, &ready, None, None);

        let listed = task_008_request_after_start(
            &endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::ListAssets(
                    mengxia_core_proto::ListAssetsRequest {
                        page_size: 64,
                        cursor: Vec::new(),
                        operation_timeout_ms: 1_000,
                    },
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        let Some(core_response::Response::ListAssets(listed)) = listed.response else {
            panic!("ListAssets result");
        };
        assert_eq!(listed.snapshot_commit_sequence, 0);
        assert!(listed.assets.is_empty());
        assert!(listed.next_cursor.is_none());

        let inspected = task_008_request_after_start(
            &endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::InspectAsset(
                    mengxia_core_proto::InspectAssetRequest {
                        asset_id: "018d442f-c000-7a11-8022-334455667700".to_owned(),
                        asset_revision_id: None,
                        page_size: 64,
                        cursor: Vec::new(),
                        operation_timeout_ms: 1_000,
                    },
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        let Some(core_response::Response::Error(inspected)) = inspected.response else {
            panic!("InspectAsset not-found response");
        };
        assert_eq!(inspected.code, ErrorCode::NotFound.as_str());
        assert!(!inspected.retryable);
        assert_eq!(inspected.retry_action, Some(RetryAction::None as i32));

        let invalid = task_008_request_after_start(
            &endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::InspectAsset(
                    mengxia_core_proto::InspectAssetRequest {
                        asset_id: "not-an-id".to_owned(),
                        asset_revision_id: None,
                        page_size: 64,
                        cursor: Vec::new(),
                        operation_timeout_ms: 1_000,
                    },
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        let Some(core_response::Response::Error(invalid)) = invalid.response else {
            panic!("InspectAsset validation response");
        };
        assert_eq!(invalid.code, ErrorCode::ValidationError.as_str());

        let after_invalid = task_008_request_after_start(
            &endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::ListAssets(
                    mengxia_core_proto::ListAssetsRequest {
                        page_size: 1,
                        cursor: Vec::new(),
                        operation_timeout_ms: 1_000,
                    },
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        assert!(matches!(
            after_invalid.response,
            Some(core_response::Response::ListAssets(_))
        ));

        let verified = task_008_request_after_start(
            &endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::VerifyLibrary(
                    mengxia_core_proto::VerifyLibraryRequest {
                        mode: mengxia_core_proto::VerificationMode::Normal as i32,
                        operation_timeout_ms: 5_000,
                    },
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        let Some(core_response::Response::VerifyLibrary(verified)) = verified.response else {
            panic!("VerifyLibrary result");
        };
        assert_eq!(verified.discovered_issue_count, 0);
        assert_eq!(verified.stored_issue_count, 0);
        assert!(!verified.has_fatal_local_issue);

        let issues = task_008_request_after_start(
            &endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::ListIntegrityIssues(
                    mengxia_core_proto::ListIntegrityIssuesRequest {
                        verification_id: verified.verification_id.clone(),
                        page_size: 64,
                        cursor: Vec::new(),
                        operation_timeout_ms: 1_000,
                    },
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        let Some(core_response::Response::ListIntegrityIssues(issues)) = issues.response else {
            panic!("ListIntegrityIssues result");
        };
        assert_eq!(issues.verification_id, verified.verification_id);
        assert!(issues.issues.is_empty());
        assert!(issues.next_cursor.is_none());

        let status = task_008_request_after_start(
            &endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::GetLibraryStatus(
                    mengxia_core_proto::GetLibraryStatusRequest {},
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        let Some(core_response::Response::GetLibraryStatus(status)) = status.response else {
            panic!("GetLibraryStatus result");
        };
        assert_eq!(
            status.custody_observation,
            mengxia_core_proto::CustodyObservation::NormalVerified as i32
        );

        stop_daemon(&mut daemon);
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn task_008_materialize_routes_one_no_clobber_effect_and_exact_replay() {
        let home = fs::canonicalize(PathBuf::from(std::env::var_os("HOME").unwrap())).unwrap();
        let base = home.join(format!(
            ".mengxia-task008-materialize-e2e-{}",
            std::process::id()
        ));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        let source = base.join("source.bin");
        let content = b"TASK-008 daemon materialization route";
        fs::write(&source, content).unwrap();
        let library = base.join("Library");
        let endpoint = base.join("runtime/mengxia-runtime-v1/client.sock");
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(endpoint.parent().unwrap().parent().unwrap())
            .unwrap();
        let ready = base.join("unused.ready");
        let mut daemon = spawn_crash_daemon(&library, &endpoint, &ready, None, None);

        let ingested = request_after_start(
            &endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::IngestAssetCopy(
                    mengxia_core_proto::IngestAssetCopyRequest {
                        command_id: "018d442f-c000-7a11-8022-334455667741".to_owned(),
                        source_path: source.as_os_str().as_bytes().to_vec(),
                        mode: IngestMode::Copy as i32,
                        asset_kind: "file".to_owned(),
                        content_kind: "binary".to_owned(),
                        representation_purpose: "original".to_owned(),
                        resource_kind: "blob".to_owned(),
                        logical_name: "source.bin".to_owned(),
                        expected_sha256: None,
                        operation_timeout_ms: 5_000,
                    },
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        let Some(core_response::Response::IngestAssetCopy(ingested)) = ingested.response else {
            panic!("IngestAssetCopy result");
        };
        let destination = base.join("materialized.bin");
        let request = CoreRequest {
            operation: Some(core_request::Operation::MaterializeAsset(
                mengxia_core_proto::MaterializeAssetRequest {
                    command_id: "018d442f-c000-7a11-8022-334455667742".to_owned(),
                    asset_id: ingested.asset_id,
                    asset_revision_id: ingested.asset_revision_id,
                    representation_id: ingested.representation_id,
                    resource_id: ingested.resource_id,
                    member_ordinal: 0,
                    destination_path: destination.as_os_str().as_bytes().to_vec(),
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let materialized =
            task_008_request_after_start(&endpoint, &request, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::MaterializeAsset(materialized)) = materialized.response
        else {
            panic!("MaterializeAsset result");
        };
        assert!(!materialized.replayed);
        assert!(!materialized.cleanup_pending);
        assert_eq!(materialized.byte_length, content.len() as u64);
        assert_eq!(fs::read(&destination).unwrap(), content);

        let replayed =
            task_008_request_after_start(&endpoint, &request, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::MaterializeAsset(replayed)) = replayed.response else {
            panic!("MaterializeAsset replay result");
        };
        assert!(replayed.replayed);
        assert!(!replayed.cleanup_pending);
        assert_eq!(replayed.command_id, materialized.command_id);
        assert_eq!(fs::read(&destination).unwrap(), content);

        let conflicting_destination = base.join("must-not-exist.bin");
        let mut conflicting_request = request.clone();
        let Some(core_request::Operation::MaterializeAsset(conflicting)) =
            conflicting_request.operation.as_mut()
        else {
            unreachable!();
        };
        conflicting.destination_path = conflicting_destination.as_os_str().as_bytes().to_vec();
        let conflict =
            task_008_request_after_start(&endpoint, &conflicting_request, Duration::from_secs(15))
                .unwrap();
        let Some(core_response::Response::Error(conflict)) = conflict.response else {
            panic!("MaterializeAsset conflict result");
        };
        assert_eq!(conflict.code, ErrorCode::Conflict.as_str());
        assert!(!conflicting_destination.exists());

        stop_daemon(&mut daemon);
        fs::remove_dir_all(&base).unwrap();
    }

    fn ingest_for_task_009(
        endpoint: &std::path::Path,
        source: &std::path::Path,
        command_id: &str,
        logical_name: &str,
    ) -> mengxia_core_proto::IngestAssetCopyResult {
        let response = request_after_start(
            endpoint,
            &CoreRequest {
                operation: Some(core_request::Operation::IngestAssetCopy(
                    mengxia_core_proto::IngestAssetCopyRequest {
                        command_id: command_id.to_owned(),
                        source_path: source.as_os_str().as_bytes().to_vec(),
                        mode: IngestMode::Copy as i32,
                        asset_kind: "image".to_owned(),
                        content_kind: "raster".to_owned(),
                        representation_purpose: "original".to_owned(),
                        resource_kind: "file".to_owned(),
                        logical_name: logical_name.to_owned(),
                        expected_sha256: None,
                        operation_timeout_ms: 5_000,
                    },
                )),
            },
            Duration::from_secs(15),
        )
        .unwrap();
        let Some(core_response::Response::IngestAssetCopy(result)) = response.response else {
            panic!("TASK-009 fixture ingest result");
        };
        result
    }

    #[test]
    fn task_009_every_operation_family_is_reachable_replayable_and_restart_durable() {
        use mengxia_core_proto::{
            AssetLifecycleRequest, AssetRevisionMemberInput, AssetRevisionRepresentationInput,
            AssetRevisionResourceInput, CreateAssetRevisionRequest, CreateProjectRequest,
            CreateSubjectRequest, CreateTakeRequest, CreateWorkItemRequest, ListProjectsRequest,
            ListSubjectsRequest, ListTakesRequest, ListWorkRequest, ProjectSpecInput,
            ReopenTakeRequest, ReviseProjectSpecRequest, ReviseWorkRequest, TakeStateValue,
            TakeTransitionValue, TransitionTakeRequest, WorkKindValue,
        };

        let home = fs::canonicalize(PathBuf::from(std::env::var_os("HOME").unwrap())).unwrap();
        let base = home.join(format!(".mengxia-task009-e2e-{}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        let source_one = base.join("one.bin");
        let source_two = base.join("two.bin");
        fs::write(&source_one, b"TASK-009 primary one").unwrap();
        fs::write(&source_two, b"TASK-009 primary two").unwrap();
        let library = base.join("Library");
        let endpoint = base.join("runtime/mengxia-runtime-v1/client.sock");
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(endpoint.parent().unwrap().parent().unwrap())
            .unwrap();
        let ready = base.join("unused.ready");
        let mut daemon = spawn_crash_daemon(&library, &endpoint, &ready, None, None);

        let asset_one = ingest_for_task_009(
            &endpoint,
            &source_one,
            "018d442f-c000-7a11-8022-334455667801",
            "one.bin",
        );
        let asset_two = ingest_for_task_009(
            &endpoint,
            &source_two,
            "018d442f-c000-7a11-8022-334455667802",
            "two.bin",
        );

        let create_revision = CoreRequest {
            operation: Some(core_request::Operation::CreateAssetRevision(
                CreateAssetRevisionRequest {
                    command_id: "018d442f-c000-7a11-8022-334455667803".to_owned(),
                    asset_id: asset_one.asset_id.clone(),
                    expected_revision: 1,
                    parent_revision_ids: vec![asset_one.asset_revision_id.clone()],
                    content_kind: "raster".to_owned(),
                    representations: vec![AssetRevisionRepresentationInput {
                        representation_purpose: "derived".to_owned(),
                        resources: vec![AssetRevisionResourceInput {
                            resource_kind: "file".to_owned(),
                            members: vec![AssetRevisionMemberInput {
                                logical_name: "one.bin".to_owned(),
                                blob_sha256: asset_one.blob_sha256.clone(),
                            }],
                        }],
                    }],
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let response =
            task_009_request_after_start(&endpoint, &create_revision, Duration::from_secs(15))
                .unwrap();
        let Some(core_response::Response::CreateAssetRevision(revision)) = response.response else {
            panic!("CreateAssetRevision result");
        };
        assert_eq!(revision.resulting_revision, 2);
        assert!(!revision.replayed);

        let retire = CoreRequest {
            operation: Some(core_request::Operation::RetireAsset(
                AssetLifecycleRequest {
                    command_id: "018d442f-c000-7a11-8022-334455667804".to_owned(),
                    asset_id: asset_one.asset_id.clone(),
                    expected_revision: 2,
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let response =
            task_009_request_after_start(&endpoint, &retire, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::RetireAsset(retired)) = response.response else {
            panic!("RetireAsset result");
        };
        assert_eq!(retired.resulting_revision, 3);
        assert_eq!(
            retired.lifecycle,
            mengxia_core_proto::AssetLifecycleValue::Retired as i32
        );

        let restore = CoreRequest {
            operation: Some(core_request::Operation::RestoreAsset(
                AssetLifecycleRequest {
                    command_id: "018d442f-c000-7a11-8022-334455667805".to_owned(),
                    asset_id: asset_one.asset_id.clone(),
                    expected_revision: 3,
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let response =
            task_009_request_after_start(&endpoint, &restore, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::RestoreAsset(restored)) = response.response else {
            panic!("RestoreAsset result");
        };
        assert_eq!(restored.resulting_revision, 4);

        let specification = ProjectSpecInput {
            resolution_width: Some(1920),
            resolution_height: Some(1080),
            frame_rate_numerator: Some(24),
            frame_rate_denominator: Some(1),
            aspect_ratio_numerator: Some(16),
            aspect_ratio_denominator: Some(9),
            color_policy_json: br#"{"space":"srgb"}"#.to_vec(),
            audio_policy_json: br#"{"channels":2}"#.to_vec(),
            quality_policy_json: br#"{"tier":"review"}"#.to_vec(),
            privacy_policy_json: br#"{"sharing":false}"#.to_vec(),
        };
        let create_project = CoreRequest {
            operation: Some(core_request::Operation::CreateProject(
                CreateProjectRequest {
                    command_id: "018d442f-c000-7a11-8022-334455667806".to_owned(),
                    name: "TASK-009 Project".to_owned(),
                    specification: Some(specification.clone()),
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let response =
            task_009_request_after_start(&endpoint, &create_project, Duration::from_secs(15))
                .unwrap();
        let Some(core_response::Response::CreateProject(project)) = response.response else {
            panic!("CreateProject result");
        };
        assert_eq!(project.project_revision, 1);
        assert!(!project.replayed);
        let project_id = project.project_id.clone();

        let replay =
            task_009_request_after_start(&endpoint, &create_project, Duration::from_secs(15))
                .unwrap();
        let Some(core_response::Response::CreateProject(replay)) = replay.response else {
            panic!("CreateProject replay");
        };
        assert!(replay.replayed);
        assert_eq!(replay.project_id, project_id);

        let revise_project = CoreRequest {
            operation: Some(core_request::Operation::ReviseProjectSpec(
                ReviseProjectSpecRequest {
                    command_id: "018d442f-c000-7a11-8022-334455667807".to_owned(),
                    project_id: project_id.clone(),
                    expected_revision: 1,
                    specification: Some(ProjectSpecInput {
                        quality_policy_json: br#"{"tier":"final"}"#.to_vec(),
                        ..specification.clone()
                    }),
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let response =
            task_009_request_after_start(&endpoint, &revise_project, Duration::from_secs(15))
                .unwrap();
        let Some(core_response::Response::ReviseProjectSpec(project_revision)) = response.response
        else {
            panic!("ReviseProjectSpec result");
        };
        assert_eq!(project_revision.project_revision, 2);

        let create_subject = CoreRequest {
            operation: Some(core_request::Operation::CreateSubject(
                CreateSubjectRequest {
                    command_id: "018d442f-c000-7a11-8022-334455667808".to_owned(),
                    kind: "person".to_owned(),
                    canonical_name: "Lead".to_owned(),
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let response =
            task_009_request_after_start(&endpoint, &create_subject, Duration::from_secs(15))
                .unwrap();
        let Some(core_response::Response::CreateSubject(subject)) = response.response else {
            panic!("CreateSubject result");
        };
        let subject_id = subject.subject_id.clone();

        let create_work = CoreRequest {
            operation: Some(core_request::Operation::CreateWorkItem(
                CreateWorkItemRequest {
                    command_id: "018d442f-c000-7a11-8022-334455667809".to_owned(),
                    project_id: project_id.clone(),
                    kind: WorkKindValue::Shot as i32,
                    code: "SHOT-001".to_owned(),
                    specification_json: br#"{"brief":"first"}"#.to_vec(),
                    subject_ids: vec![subject_id.clone()],
                    asset_ids: vec![asset_one.asset_id.clone()],
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let response =
            task_009_request_after_start(&endpoint, &create_work, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::CreateWorkItem(work)) = response.response else {
            panic!("CreateWorkItem result");
        };
        let work_item_id = work.work_item_id.clone();

        let revise_work = CoreRequest {
            operation: Some(core_request::Operation::ReviseWork(ReviseWorkRequest {
                command_id: "018d442f-c000-7a11-8022-33445566780a".to_owned(),
                project_id: project_id.clone(),
                work_item_id: work_item_id.clone(),
                expected_revision: 1,
                specification_json: br#"{"brief":"second"}"#.to_vec(),
                subject_ids: vec![subject_id.clone()],
                asset_ids: vec![asset_one.asset_id.clone(), asset_two.asset_id.clone()],
                operation_timeout_ms: 5_000,
            })),
        };
        let response =
            task_009_request_after_start(&endpoint, &revise_work, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::ReviseWork(work)) = response.response else {
            panic!("ReviseWork result");
        };
        assert_eq!(work.work_item_revision, 2);
        let work_revision_id = work.work_revision_id.clone();

        let create_take = CoreRequest {
            operation: Some(core_request::Operation::CreateTake(CreateTakeRequest {
                command_id: "018d442f-c000-7a11-8022-33445566780b".to_owned(),
                project_id: project_id.clone(),
                work_item_id: work_item_id.clone(),
                work_revision_id: work_revision_id.clone(),
                primary_asset_id: asset_one.asset_id.clone(),
                operation_timeout_ms: 5_000,
            })),
        };
        let response =
            task_009_request_after_start(&endpoint, &create_take, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::CreateTake(take)) = response.response else {
            panic!("CreateTake result");
        };
        assert_eq!(take.state, TakeStateValue::Candidate as i32);
        let take_id = take.take_id.clone();

        let reject_take = CoreRequest {
            operation: Some(core_request::Operation::TransitionTake(
                TransitionTakeRequest {
                    command_id: "018d442f-c000-7a11-8022-33445566780c".to_owned(),
                    project_id: project_id.clone(),
                    work_item_id: work_item_id.clone(),
                    work_revision_id: work_revision_id.clone(),
                    take_id: take_id.clone(),
                    expected_revision: 1,
                    transition: TakeTransitionValue::Reject as i32,
                    reason: Some("not selected".to_owned()),
                    related_take_id: None,
                    related_take_expected_revision: None,
                    operation_timeout_ms: 5_000,
                },
            )),
        };
        let response =
            task_009_request_after_start(&endpoint, &reject_take, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::TransitionTake(rejected)) = response.response else {
            panic!("TransitionTake result");
        };
        assert_eq!(rejected.state, TakeStateValue::Rejected as i32);
        assert_eq!(rejected.take_revision, 2);

        let reopen_take = CoreRequest {
            operation: Some(core_request::Operation::ReopenTake(ReopenTakeRequest {
                command_id: "018d442f-c000-7a11-8022-33445566780d".to_owned(),
                project_id: project_id.clone(),
                work_item_id: work_item_id.clone(),
                work_revision_id: work_revision_id.clone(),
                terminal_take_id: take_id.clone(),
                terminal_take_expected_revision: 2,
                new_primary_asset_id: asset_two.asset_id.clone(),
                operation_timeout_ms: 5_000,
            })),
        };
        let response =
            task_009_request_after_start(&endpoint, &reopen_take, Duration::from_secs(15)).unwrap();
        let Some(core_response::Response::ReopenTake(reopened)) = response.response else {
            panic!("ReopenTake result");
        };
        assert_eq!(reopened.state, TakeStateValue::Candidate as i32);
        assert_eq!(reopened.related_take_id.as_deref(), Some(take_id.as_str()));

        for request in [
            CoreRequest {
                operation: Some(core_request::Operation::ListProjects(ListProjectsRequest {
                    page_size: 64,
                    cursor: Vec::new(),
                    operation_timeout_ms: 5_000,
                })),
            },
            CoreRequest {
                operation: Some(core_request::Operation::ListSubjects(ListSubjectsRequest {
                    page_size: 64,
                    cursor: Vec::new(),
                    operation_timeout_ms: 5_000,
                })),
            },
            CoreRequest {
                operation: Some(core_request::Operation::ListWork(ListWorkRequest {
                    project_id: project_id.clone(),
                    page_size: 64,
                    cursor: Vec::new(),
                    operation_timeout_ms: 5_000,
                })),
            },
            CoreRequest {
                operation: Some(core_request::Operation::ListTakes(ListTakesRequest {
                    project_id: project_id.clone(),
                    work_item_id: work_item_id.clone(),
                    work_revision_id: work_revision_id.clone(),
                    page_size: 64,
                    cursor: Vec::new(),
                    operation_timeout_ms: 5_000,
                })),
            },
        ] {
            let response =
                task_009_request_after_start(&endpoint, &request, Duration::from_secs(15)).unwrap();
            assert!(matches!(
                response.response,
                Some(
                    core_response::Response::ListProjects(_)
                        | core_response::Response::ListSubjects(_)
                        | core_response::Response::ListWork(_)
                        | core_response::Response::ListTakes(_)
                )
            ));
        }

        stop_daemon(&mut daemon);
        let mut daemon = spawn_crash_daemon(&library, &endpoint, &ready, None, None);
        let replay =
            task_009_request_after_start(&endpoint, &create_project, Duration::from_secs(15))
                .unwrap();
        let Some(core_response::Response::CreateProject(replay)) = replay.response else {
            panic!("durable CreateProject replay");
        };
        assert!(replay.replayed);
        assert_eq!(replay.project_id, project_id);

        stop_daemon(&mut daemon);
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn numeric_values_are_unsigned_ascii_decimal_only() {
        assert_eq!(parse_ascii_u64(&OsString::from("5000")), Ok(5000));
        for invalid in ["", " 1", "+1", "-1", "01", "1_0", "18446744073709551616"] {
            assert_eq!(
                parse_ascii_u64(&OsString::from(invalid)),
                Err(ErrorCode::ValidationError)
            );
        }
    }

    fn wire_ingest() -> mengxia_core_proto::CoreRequest {
        mengxia_core_proto::CoreRequest {
            operation: Some(
                mengxia_core_proto::core_request::Operation::IngestAssetCopy(
                    mengxia_core_proto::IngestAssetCopyRequest {
                        command_id: "018d442f-c000-7a11-8022-334455667788".to_owned(),
                        source_path: b"/private/tmp/source.bin".to_vec(),
                        mode: IngestMode::Copy as i32,
                        asset_kind: "file".to_owned(),
                        content_kind: "binary".to_owned(),
                        representation_purpose: "original".to_owned(),
                        resource_kind: "blob".to_owned(),
                        logical_name: "source.bin".to_owned(),
                        expected_sha256: None,
                        operation_timeout_ms: 100,
                    },
                ),
            ),
        }
    }

    #[test]
    fn ingest_semantics_reject_unknown_modes_paths_digests_and_timeouts() {
        assert!(
            decode_ingest_request(wire_ingest(), std::time::Duration::from_millis(100)).is_ok()
        );
        for mutate in [
            |request: &mut mengxia_core_proto::IngestAssetCopyRequest| request.mode = 0,
            |request: &mut mengxia_core_proto::IngestAssetCopyRequest| request.mode = 99,
            |request: &mut mengxia_core_proto::IngestAssetCopyRequest| {
                request.source_path = b"relative/source".to_vec();
            },
            |request: &mut mengxia_core_proto::IngestAssetCopyRequest| {
                request.expected_sha256 = Some(vec![0; 31]);
            },
            |request: &mut mengxia_core_proto::IngestAssetCopyRequest| {
                request.operation_timeout_ms = 99;
            },
        ] {
            let mut request = wire_ingest();
            let Some(mengxia_core_proto::core_request::Operation::IngestAssetCopy(ingest)) =
                request.operation.as_mut()
            else {
                unreachable!();
            };
            mutate(ingest);
            assert_eq!(
                decode_ingest_request(request, std::time::Duration::from_millis(100)).err(),
                Some(ErrorCode::ValidationError)
            );
        }
    }

    #[test]
    fn disconnect_extra_input_and_deadline_signal_and_join_owned_work() {
        for trigger in [0_u8, 1, 2] {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                use std::sync::Arc;
                use std::sync::atomic::{AtomicBool, Ordering};

                use tokio::io::AsyncWriteExt as _;

                let (mut server, mut client) = tokio::net::UnixStream::pair().unwrap();
                let stopped = Arc::new(AtomicBool::new(false));
                let worker_stopped = Arc::clone(&stopped);
                let worker = tokio::task::spawn_blocking(move || {
                    while !worker_stopped.load(Ordering::Acquire) {
                        std::thread::yield_now();
                    }
                    7_u8
                });
                let deadline = if trigger == 2 {
                    tokio::time::Instant::now() + std::time::Duration::from_millis(10)
                } else {
                    tokio::time::Instant::now() + std::time::Duration::from_secs(1)
                };
                if trigger == 0 {
                    drop(client);
                } else if trigger == 1 {
                    client.write_all(b"x").await.unwrap();
                }
                assert_eq!(
                    await_ingest_with_watcher(&mut server, worker, deadline, Arc::clone(&stopped))
                        .await,
                    Ok(7)
                );
                assert!(stopped.load(Ordering::Acquire));
            });
        }
    }

    #[test]
    fn task_007_fatal_shutdown_child_entrypoint() {
        let Some(role) = std::env::var_os(FATAL_CHILD_ROLE) else {
            return;
        };
        let blocking = std::sync::Arc::new(BlockingDrop);
        match role.to_str() {
            Some("leaked-owner") => {
                let _leaked = std::sync::Arc::clone(&blocking);
                let _never_returns = take_last_owner(blocking);
            }
            Some("shutdown-timeout") => fatal_shutdown(),
            _ => panic!("unknown fatal child role"),
        }
    }

    #[test]
    fn leaked_owner_and_shutdown_timeout_exit_without_blocking_drop_unwind() {
        for role in ["leaked-owner", "shutdown-timeout"] {
            let mut child = ProcessCommand::new(std::env::current_exe().unwrap())
                .env(FATAL_CHILD_ROLE, role)
                .args([
                    "tests::task_007_fatal_shutdown_child_entrypoint",
                    "--exact",
                    "--nocapture",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if std::time::Instant::now() >= deadline {
                    child.kill().unwrap();
                    let _ = child.wait();
                    panic!("fatal shutdown child blocked in Drop for role {role}");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert_eq!(status.code(), Some(1), "fatal branch for role {role}");
        }
    }

    #[test]
    fn task_007_orchestration_sigkill_child_entrypoint() {
        if std::env::var_os(CRASH_CHILD_ROLE).is_none() {
            return;
        }
        let library = std::env::var_os(CRASH_LIBRARY).expect("crash Library path");
        let endpoint = std::env::var_os(CRASH_ENDPOINT).expect("crash endpoint path");
        let config = resolve_from_layers(
            ServeCli {
                library_root: Some(library),
                endpoint: Some(endpoint),
                ..ServeCli::default()
            },
            DaemonEnvironment {
                platform_temp_root: PathBuf::from("/private/tmp"),
                ..DaemonEnvironment::default()
            },
            DaemonLibraryConfig::default(),
        )
        .expect("resolve crash-child daemon configuration");
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build crash-child runtime");
        assert_eq!(runtime.block_on(serve(config)), Ok(()));
    }

    #[test]
    fn orchestration_sigkill_boundaries_reopen_through_production_startup() {
        struct CrashCase {
            id: &'static str,
            checkpoint: Option<usize>,
            terminal: bool,
            restart_error: Option<ErrorCode>,
        }

        let cases = [
            CrashCase {
                id: "KILL-007-001",
                checkpoint: Some(1),
                terminal: false,
                restart_error: None,
            },
            CrashCase {
                id: "KILL-007-002",
                checkpoint: Some(2),
                terminal: false,
                restart_error: None,
            },
            CrashCase {
                id: "KILL-007-003",
                checkpoint: Some(4),
                terminal: false,
                restart_error: Some(ErrorCode::StorageConfigurationError),
            },
            CrashCase {
                id: "KILL-007-012",
                checkpoint: None,
                terminal: false,
                restart_error: None,
            },
            CrashCase {
                id: "KILL-007-013",
                checkpoint: None,
                terminal: false,
                restart_error: None,
            },
            CrashCase {
                id: "KILL-007-015",
                checkpoint: None,
                terminal: true,
                restart_error: Some(ErrorCode::StorageCorruption),
            },
            CrashCase {
                id: "KILL-007-016",
                checkpoint: None,
                terminal: true,
                restart_error: Some(ErrorCode::StorageCorruption),
            },
        ];

        for (index, case) in cases.into_iter().enumerate() {
            let home = fs::canonicalize(PathBuf::from(std::env::var_os("HOME").unwrap())).unwrap();
            let base = home.join(format!(
                ".mengxia-task007-orchestration-{}-{index}",
                std::process::id()
            ));
            fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
            fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
            let library = base.join("Library");
            let endpoint = base.join("runtime/mengxia-runtime-v1/client.sock");
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(endpoint.parent().unwrap().parent().unwrap())
                .unwrap();
            let source = base.join("source.bin");
            fs::write(&source, b"TASK-007 orchestration crash fixture").unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
            let ready = base.join("crash.ready");
            let command_id = format!("018d442f-c000-7a11-8022-3344556678{index:02x}");
            let request = crash_ingest_request(&source, &command_id, case.terminal);

            let mut child =
                spawn_crash_daemon(&library, &endpoint, &ready, case.checkpoint, Some(case.id));
            let client_endpoint = endpoint.clone();
            let client_request = request.clone();
            let client = thread::spawn(move || {
                request_after_start(&client_endpoint, &client_request, Duration::from_secs(15))
            });
            wait_for_crash_ready(&mut child, &ready, case.id);
            child.kill().expect("SIGKILL TASK-007 crash child");
            let status = child.wait().expect("reap TASK-007 crash child");
            assert!(!status.success(), "{} child must be killed", case.id);
            let _ = client.join().expect("join crash-boundary client");

            fs::remove_file(&ready).unwrap();
            let mut reopened = spawn_crash_daemon(&library, &endpoint, &ready, None, None);
            let response = request_after_start(&endpoint, &request, Duration::from_secs(15))
                .unwrap_or_else(|error| panic!("{} restart request failed: {error:?}", case.id));
            assert_restart_response(case.id, response, case.restart_error);
            stop_daemon(&mut reopened);
            fs::remove_dir_all(&base).unwrap();
        }
    }

    fn crash_ingest_request(
        source: &std::path::Path,
        command_id: &str,
        terminal: bool,
    ) -> CoreRequest {
        CoreRequest {
            operation: Some(core_request::Operation::IngestAssetCopy(
                mengxia_core_proto::IngestAssetCopyRequest {
                    command_id: command_id.to_owned(),
                    source_path: source.as_os_str().as_bytes().to_vec(),
                    mode: IngestMode::Copy as i32,
                    asset_kind: "file".to_owned(),
                    content_kind: "binary".to_owned(),
                    representation_purpose: "original".to_owned(),
                    resource_kind: "blob".to_owned(),
                    logical_name: "source.bin".to_owned(),
                    expected_sha256: terminal.then(|| vec![0; 32]),
                    operation_timeout_ms: 10_000,
                },
            )),
        }
    }

    fn spawn_crash_daemon(
        library: &std::path::Path,
        endpoint: &std::path::Path,
        ready: &std::path::Path,
        checkpoint: Option<usize>,
        response_boundary: Option<&str>,
    ) -> std::process::Child {
        let mut command = ProcessCommand::new(std::env::current_exe().unwrap());
        command
            .env(CRASH_CHILD_ROLE, "daemon")
            .env(CRASH_LIBRARY, library)
            .env(CRASH_ENDPOINT, endpoint)
            .env("MENGXIA_TASK007_CRASH_READY", ready)
            .env_remove("MENGXIA_TASK007_CRASH_CHECKPOINT")
            .env_remove("MENGXIA_TASK007_RESPONSE_CRASH_BOUNDARY")
            .args([
                "tests::task_007_orchestration_sigkill_child_entrypoint",
                "--exact",
                "--nocapture",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(checkpoint) = checkpoint {
            command.env("MENGXIA_TASK007_CRASH_CHECKPOINT", checkpoint.to_string());
        } else if let Some(boundary) = response_boundary {
            command.env("MENGXIA_TASK007_RESPONSE_CRASH_BOUNDARY", boundary);
        }
        command.spawn().expect("spawn TASK-007 crash daemon")
    }

    fn request_after_start(
        endpoint: &std::path::Path,
        request: &CoreRequest,
        timeout: Duration,
    ) -> Result<CoreResponse, ErrorCode> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let deadline = tokio::time::Instant::now() + timeout;
            loop {
                let mut stream = loop {
                    match tokio::net::UnixStream::connect(endpoint).await {
                        Ok(stream) => break stream,
                        Err(_) if tokio::time::Instant::now() < deadline => {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Err(_) => return Err(ErrorCode::IpcTransportError),
                    }
                };
                let handshake = HandshakeLimits::new(
                    FrameLimit::default(),
                    DecodeDepth::new(64).unwrap(),
                    Duration::from_secs(5),
                )
                .unwrap();
                let operation =
                    OperationLimits::new(FrameLimit::default(), DecodeDepth::new(64).unwrap())
                        .unwrap();
                let remaining = deadline
                    .checked_duration_since(tokio::time::Instant::now())
                    .ok_or(ErrorCode::DeadlineExceeded)?;
                let response = request_single_command(
                    &mut stream,
                    "018d442f-c000-7a11-8022-3344556677ff",
                    request,
                    handshake,
                    operation,
                    remaining,
                )
                .await
                .map(|(_, response)| response)
                .map_err(|error| error.code())?;
                let startup_pending = matches!(
                    response.response.as_ref(),
                    Some(core_response::Response::Error(error))
                        if error.code == ErrorCode::Backpressure.as_str()
                            && error.retry_action == Some(RetryAction::FreshCommand as i32)
                );
                if !startup_pending {
                    return Ok(response);
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err(ErrorCode::DeadlineExceeded);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
    }

    fn task_008_request_after_start(
        endpoint: &std::path::Path,
        request: &CoreRequest,
        timeout: Duration,
    ) -> Result<CoreResponse, ErrorCode> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let deadline = tokio::time::Instant::now() + timeout;
            loop {
                let mut stream = loop {
                    match tokio::net::UnixStream::connect(endpoint).await {
                        Ok(stream) => break stream,
                        Err(_) if tokio::time::Instant::now() < deadline => {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Err(_) => return Err(ErrorCode::IpcTransportError),
                    }
                };
                let handshake = HandshakeLimits::new(
                    FrameLimit::default(),
                    DecodeDepth::new(64).unwrap(),
                    Duration::from_secs(5),
                )
                .unwrap();
                let operation =
                    OperationLimits::new(FrameLimit::default(), DecodeDepth::new(64).unwrap())
                        .unwrap();
                let remaining = deadline
                    .checked_duration_since(tokio::time::Instant::now())
                    .ok_or(ErrorCode::DeadlineExceeded)?;
                let response = request_task_008_command(
                    &mut stream,
                    "018d442f-c000-7a11-8022-3344556677ee",
                    request,
                    handshake,
                    operation,
                    remaining,
                )
                .await
                .map(|(_, response)| response)
                .map_err(|error| error.code())?;
                let startup_pending = matches!(
                    response.response.as_ref(),
                    Some(core_response::Response::Error(error))
                        if error.code == ErrorCode::Backpressure.as_str()
                            && error.retry_action == Some(RetryAction::FreshCommand as i32)
                );
                if !startup_pending {
                    return Ok(response);
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err(ErrorCode::DeadlineExceeded);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
    }

    fn task_009_request_after_start(
        endpoint: &std::path::Path,
        request: &CoreRequest,
        timeout: Duration,
    ) -> Result<CoreResponse, ErrorCode> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let deadline = tokio::time::Instant::now() + timeout;
            loop {
                let mut stream = loop {
                    match tokio::net::UnixStream::connect(endpoint).await {
                        Ok(stream) => break stream,
                        Err(_) if tokio::time::Instant::now() < deadline => {
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                        Err(_) => return Err(ErrorCode::IpcTransportError),
                    }
                };
                let handshake = HandshakeLimits::new(
                    FrameLimit::default(),
                    DecodeDepth::new(64).unwrap(),
                    Duration::from_secs(5),
                )
                .unwrap();
                let operation =
                    OperationLimits::new(FrameLimit::default(), DecodeDepth::new(64).unwrap())
                        .unwrap();
                let remaining = deadline
                    .checked_duration_since(tokio::time::Instant::now())
                    .ok_or(ErrorCode::DeadlineExceeded)?;
                let response = request_task_009_command(
                    &mut stream,
                    "018d442f-c000-7a11-8022-3344556677dd",
                    request,
                    handshake,
                    operation,
                    remaining,
                )
                .await
                .map(|(_, response)| response)
                .map_err(|error| error.code())?;
                let startup_pending = matches!(
                    response.response.as_ref(),
                    Some(core_response::Response::Error(error))
                        if error.code == ErrorCode::Backpressure.as_str()
                            && error.retry_action == Some(RetryAction::FreshCommand as i32)
                );
                if !startup_pending {
                    return Ok(response);
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err(ErrorCode::DeadlineExceeded);
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
    }

    fn wait_for_crash_ready(child: &mut std::process::Child, ready: &std::path::Path, id: &str) {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !ready.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "{id} crash acknowledgement exceeded deadline"
            );
            assert!(
                child.try_wait().unwrap().is_none(),
                "{id} child exited early"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn assert_restart_response(id: &str, response: CoreResponse, expected: Option<ErrorCode>) {
        match (response.response, expected) {
            (Some(core_response::Response::IngestAssetCopy(_)), None) => {}
            (Some(core_response::Response::Error(error)), Some(expected)) => {
                assert_eq!(error.code, expected.as_str(), "{id} safe error");
            }
            _ => panic!("{id} restart response did not match its durable state"),
        }
    }

    fn stop_daemon(child: &mut std::process::Child) {
        let status = ProcessCommand::new("/bin/kill")
            .arg("-INT")
            .arg(child.id().to_string())
            .status()
            .expect("signal reopened TASK-007 daemon");
        assert!(status.success());
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        loop {
            match child.try_wait().unwrap() {
                Some(status) => {
                    assert!(status.success(), "reopened TASK-007 daemon failed");
                    return;
                }
                None if std::time::Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(10));
                }
                None => {
                    child.kill().unwrap();
                    let _ = child.wait();
                    panic!("reopened TASK-007 daemon shutdown exceeded deadline");
                }
            }
        }
    }

    #[test]
    fn typed_layers_obey_cli_environment_library_default_precedence() {
        let endpoint = PathBuf::from("/private/tmp/task003-resolver/client.sock");
        let config = resolve_from_layers(
            ServeCli {
                library_root: Some(OsString::from("/private/tmp/Task003Library")),
                endpoint: Some(endpoint.clone().into_os_string()),
                frame: Some(OsString::from("65536")),
                depth: Some(OsString::from("5")),
                timeout: Some(OsString::from("100")),
                pending: Some(OsString::from("1")),
                storage_io: Some(OsString::from("3")),
                log_level: Some(OsString::from("debug")),
                max_verify_operation_timeout: Some(OsString::from("1000")),
                ..ServeCli::default()
            },
            DaemonEnvironment {
                library_root: Some(OsString::from("invalid-relative-library")),
                endpoint: Some(OsString::from("invalid-relative-endpoint")),
                frame_bytes: Some(OsString::from("invalid")),
                decode_depth: Some(OsString::from("invalid")),
                handshake_timeout_ms: Some(OsString::from("invalid")),
                max_pending_handshakes: Some(OsString::from("invalid")),
                write_queue: Some(OsString::from("32")),
                hash: Some(OsString::from("4")),
                log_level: Some(OsString::from("invalid-lower-log-level")),
                max_verify_operation_timeout: Some(OsString::from("invalid-lower-timeout")),
                max_materialize_operation_timeout: Some(OsString::from("2000")),
                read_connections: None,
                busy_timeout_ms: None,
                platform_temp_root: PathBuf::from("/private/tmp"),
                ..DaemonEnvironment::default()
            },
            DaemonLibraryConfig {
                endpoint: Some(PathBuf::from("/private/tmp/lower/client.sock")),
                frame_bytes: Some(OsString::from("invalid-lower-frame")),
                decode_depth: Some(OsString::from("4")),
                handshake_timeout_ms: Some(OsString::from("200")),
                max_pending_handshakes: Some(OsString::from("2")),
                write_queue: Some(OsString::from("64")),
                read_connections: Some(OsString::from("2")),
                busy_timeout_ms: None,
                max_ingests: Some(OsString::from("5")),
                log_level: Some(OsString::from("info")),
                max_materialize_operation_timeout: Some(OsString::from("invalid-lower-timeout")),
                ..DaemonLibraryConfig::default()
            },
        )
        .unwrap();

        assert_eq!(config.endpoint, endpoint);
        assert_eq!(
            config.limits.timeout(),
            std::time::Duration::from_millis(100)
        );
        assert_eq!(config.max_pending, 1);
        assert_eq!(config.task_008.log_level(), CoreLogLevel::Debug);
        assert_eq!(
            config.task_008.max_verify_operation_timeout(),
            Duration::from_millis(1000)
        );
        assert_eq!(
            config.task_008.max_materialize_operation_timeout(),
            Duration::from_millis(2000)
        );
        assert_eq!(config.store.library_root_source(), ConfigSource::Cli);
        assert_eq!(config.store.write_queue_capacity(), 32);
        assert_eq!(config.store.write_queue_source(), ConfigSource::Environment);
        assert_eq!(config.store.read_connection_count(), 2);
        assert_eq!(config.store.read_connection_source(), ConfigSource::Library);
        assert_eq!(
            config.store.busy_timeout_source(),
            ConfigSource::CompiledDefault
        );
        assert_eq!(config.blob.storage_io_concurrency(), 3);
        assert_eq!(
            config.blob.storage_io_concurrency_source(),
            BlobConfigSource::Cli
        );
        assert_eq!(config.blob.hash_concurrency(), 4);
        assert_eq!(
            config.blob.hash_concurrency_source(),
            BlobConfigSource::Environment
        );
        assert_eq!(config.blob.max_concurrent_ingests(), 5);
        assert_eq!(
            config.blob.max_concurrent_ingests_source(),
            BlobConfigSource::Library
        );
        assert_eq!(
            config.blob.stream_buffer_bytes_source(),
            BlobConfigSource::CompiledDefault
        );

        let (_, store_source, blob_source) = selected_required(
            None,
            None,
            Some(PathBuf::from("/private/tmp/LibraryFromDocument")),
        )
        .unwrap();
        assert_eq!(store_source, ConfigSource::Library);
        assert_eq!(blob_source, BlobConfigSource::Library);

        let invalid_higher_layer = resolve_from_layers(
            ServeCli {
                library_root: Some(OsString::from("/private/tmp/Task003Library")),
                endpoint: Some(OsString::from("/private/tmp/task003-resolver/client.sock")),
                ..ServeCli::default()
            },
            DaemonEnvironment {
                library_root: None,
                endpoint: None,
                frame_bytes: Some(OsString::from("invalid")),
                decode_depth: None,
                handshake_timeout_ms: None,
                max_pending_handshakes: None,
                write_queue: None,
                read_connections: None,
                busy_timeout_ms: None,
                platform_temp_root: PathBuf::from("/private/tmp"),
                ..DaemonEnvironment::default()
            },
            DaemonLibraryConfig {
                frame_bytes: Some(OsString::from("65536")),
                ..DaemonLibraryConfig::default()
            },
        );
        assert!(matches!(
            invalid_higher_layer,
            Err(ErrorCode::ValidationError)
        ));

        let invalid_task_008_higher_layer = resolve_from_layers(
            ServeCli {
                library_root: Some(OsString::from("/private/tmp/Task008Library")),
                endpoint: Some(OsString::from("/private/tmp/task008-resolver/client.sock")),
                log_level: Some(OsString::from_vec(vec![0xff])),
                ..ServeCli::default()
            },
            DaemonEnvironment {
                log_level: Some(OsString::from("info")),
                platform_temp_root: PathBuf::from("/private/tmp"),
                ..DaemonEnvironment::default()
            },
            DaemonLibraryConfig::default(),
        );
        assert!(matches!(
            invalid_task_008_higher_layer,
            Err(ErrorCode::StorageConfigurationError)
        ));
    }
}
