//! MengXia daemon composition root.

#![forbid(unsafe_code)]

use std::env;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStringExt as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant as StdInstant};

use mengxia_app::{
    IngestAdmissionLimits, IngestAssetCopyRequest as AppIngestRequest, IngestAssetCopyService,
    IngestAssetExecutionError, IngestRetry, LibraryConfigDocument, LibraryConfigKey,
};
#[cfg(test)]
use mengxia_core_proto::serve_handshake;
use mengxia_core_proto::{
    CoreRequest, CoreResponse, DecodeDepth, HandshakeLimits, IngestMode, OperationLimits,
    RetryAction, ServerNegotiation, core_request, core_response, operation_error_response,
    read_core_request, serve_daemon_handshake, write_core_response,
};
use mengxia_domain::{AssetKind, ContentKind, LogicalName, RepresentationPurpose, ResourceKind};
use mengxia_framing::FrameLimit;
use mengxia_platform_fs::{
    AuthorityError, bind_runtime_endpoint, read_library_config, validate_runtime_endpoint_path,
};
use mengxia_ports::{Command as PersistedCommand, IngestControl, IngestDirective, IngestStop};
use mengxia_storage_local::{
    BlobConfigSource, BlobIngestState, BlobStorageConfig, LocalBlobStorage,
    ResolvedBlobStorageConfig,
};
use mengxia_store_sqlite::{
    ConfigSource, OpenedLibrary, ResolvedStoreConfig, StoreConfig, StoreError,
};
use mengxia_types::{ErrorCode, Id, Sha256Digest};
use tokio::io::AsyncReadExt as _;
use tokio::net::UnixListener;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const HELP: &str = "mengxiad serve [--library-root PATH] [--blob-root PATH] [--client-endpoint PATH]\n  [--max-frame-bytes ASCII_U64] [--max-decode-depth ASCII_U32]\n  [--client-handshake-timeout-ms ASCII_U64] [--max-pending-handshakes ASCII_U32]\n  [--max-client-sessions ASCII_U32] [--max-ingest-operation-timeout-ms ASCII_U64]\n  [--ingest-shutdown-timeout-ms ASCII_U64]\n";

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
    let store = opened.asset_store_handle();
    if let Err(error) = store
        .validate_local_managed_backend(startup.backend_id())
        .await
    {
        let _ = storage.shutdown();
        let _ = opened.shutdown();
        return Err(error.error_code());
    }
    let storage = Arc::new(storage);
    let service = Arc::new(IngestAssetCopyService::new(
        Arc::new(store),
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
            drop(service);
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
            drop(service);
            let _ = take_last_owner(storage).shutdown();
            let _ = opened.shutdown();
            return Err(primary);
        }
    };
    let listener = match UnixListener::from_std(std_listener) {
        Ok(listener) => listener,
        Err(_) => {
            let _ = endpoint.cleanup();
            drop(service);
            let _ = take_last_owner(storage).shutdown();
            let _ = opened.shutdown();
            return Err(ErrorCode::StorageIoError);
        }
    };

    let admission = Arc::new(Semaphore::new(config.max_pending));
    let sessions = Arc::new(Semaphore::new(config.max_sessions));
    let cancelling = Arc::new(AtomicBool::new(false));
    let mut tasks = JoinSet::new();
    let mut primary = None;
    let signal = shutdown_signal();
    tokio::pin!(signal);
    loop {
        tokio::select! {
            signal_result = &mut signal => {
                cancelling.store(true, Ordering::Release);
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
                        let owner_uid = identity.owner_uid();
                        let sessions = Arc::clone(&sessions);
                        let service = Arc::clone(&service);
                        let cancelling = Arc::clone(&cancelling);
                        tasks.spawn(async move {
                            serve_connection(
                                stream, owner_uid, limits, operation_limits,
                                max_operation_timeout, sessions, service, cancelling, permit,
                            ).await
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
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(code))) => {
                        primary.get_or_insert(code);
                        cancelling.store(true, Ordering::Release);
                        break;
                    }
                    Some(Err(_)) => {
                        primary.get_or_insert(ErrorCode::InternalError);
                        cancelling.store(true, Ordering::Release);
                        break;
                    }
                    None => {}
                }
            }
        }
    }
    drop(listener);

    cancelling.store(true, Ordering::Release);
    let join_deadline = tokio::time::Instant::now() + config.shutdown_timeout;
    while !tasks.is_empty() {
        match tokio::time::timeout_at(join_deadline, tasks.join_next()).await {
            Ok(Some(Ok(Ok(())))) => {}
            Ok(Some(Ok(Err(code)))) => {
                primary.get_or_insert(code);
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
    if let Err(error) = take_last_owner(storage).shutdown() {
        primary.get_or_insert(error.code());
    }
    if let Err(error) = opened.shutdown() {
        primary.get_or_insert(error.code());
    }
    primary.map_or(Ok(()), Err)
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
    sessions: Arc<Semaphore>,
    service: Arc<IngestAssetCopyService<LocalBlobStorage>>,
    cancelling: Arc<AtomicBool>,
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
        None => return Err(ErrorCode::ValidationError),
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
    let store = ResolvedStoreConfig::from_selected(
        Some(library_root),
        library_source,
        usize::try_from(write_queue).map_err(|_| ErrorCode::ValidationError)?,
        write_queue_source,
        usize::try_from(readers).map_err(|_| ErrorCode::ValidationError)?,
        readers_source,
        busy,
        busy_source,
    )
    .validate()
    .map_err(|_| ErrorCode::ValidationError)?;
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
    Ok(DaemonConfig {
        store,
        blob,
        endpoint,
        limits,
        max_pending,
        max_sessions,
        operation_limits,
        max_operation_timeout: Duration::from_millis(max_operation_timeout_ms),
        shutdown_timeout: Duration::from_millis(shutdown_timeout_ms),
    })
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
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
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
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
    use std::path::PathBuf;
    use std::process::{Command as ProcessCommand, Stdio};
    use std::thread;
    use std::time::Duration;

    use mengxia_core_proto::{
        CoreRequest, CoreResponse, DecodeDepth, HandshakeLimits, OperationLimits, core_request,
        core_response, request_single_command,
    };
    use mengxia_framing::FrameLimit;
    use mengxia_storage_local::BlobConfigSource;
    use mengxia_store_sqlite::ConfigSource;
    use mengxia_types::ErrorCode;

    use super::{
        Command, DaemonEnvironment, DaemonLibraryConfig, IngestMode, ServeCli,
        await_ingest_with_watcher, decode_ingest_request, fatal_shutdown, parse_ascii_u64,
        parse_command, resolve_from_layers, selected_required, serve, take_last_owner,
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

    #[test]
    fn numeric_values_are_unsigned_ascii_decimal_only() {
        assert_eq!(parse_ascii_u64(&OsString::from("5000")), Ok(5000));
        for invalid in ["", " 1", "+1", "-1", "1_0", "18446744073709551616"] {
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
                OperationLimits::new(FrameLimit::default(), DecodeDepth::new(64).unwrap()).unwrap();
            request_single_command(
                &mut stream,
                "018d442f-c000-7a11-8022-3344556677ff",
                request,
                handshake,
                operation,
                timeout,
            )
            .await
            .map(|(_, response)| response)
            .map_err(|error| error.code())
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
                depth: Some(OsString::from("3")),
                timeout: Some(OsString::from("100")),
                pending: Some(OsString::from("1")),
                storage_io: Some(OsString::from("3")),
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
    }
}
