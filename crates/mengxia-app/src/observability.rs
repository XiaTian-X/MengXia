use std::fmt;
use std::io::Write;
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use mengxia_ports::{Command, IntegrityIssueKind};
use mengxia_types::{ErrorCode, Id, Timestamp};

use crate::CoreLogLevel;

const OBSERVABILITY_QUEUE_MAX: usize = 256;
const CORE_LOG_LINE_BYTES_MAX: usize = 1024;
const BUILD_VERSION_BYTES_MAX: usize = 64;

pub enum CoreRequestIdentity {}
pub enum CoreCorrelationIdentity {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreLogSeverity {
    Alert,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl CoreLogSeverity {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Alert => "ALERT",
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
        }
    }
}

impl CoreLogLevel {
    #[must_use]
    pub const fn allows(self, severity: CoreLogSeverity) -> bool {
        match severity {
            CoreLogSeverity::Alert | CoreLogSeverity::Error => true,
            CoreLogSeverity::Warn => !matches!(self, Self::Error),
            CoreLogSeverity::Info => matches!(self, Self::Info | Self::Debug | Self::Trace),
            CoreLogSeverity::Debug => matches!(self, Self::Debug | Self::Trace),
            CoreLogSeverity::Trace => matches!(self, Self::Trace),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreLogEventKind {
    ServiceStarting,
    ServiceReady,
    ServiceStopping,
    OperationStarted,
    OperationCompleted,
    OperationFailed,
    DbObservation,
    StorageObservation,
    IntegrityIssueObserved,
    HealthTransition,
    RecoveryClassification,
}

impl CoreLogEventKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ServiceStarting => "SERVICE_STARTING",
            Self::ServiceReady => "SERVICE_READY",
            Self::ServiceStopping => "SERVICE_STOPPING",
            Self::OperationStarted => "OPERATION_STARTED",
            Self::OperationCompleted => "OPERATION_COMPLETED",
            Self::OperationFailed => "OPERATION_FAILED",
            Self::DbObservation => "DB_OBSERVATION",
            Self::StorageObservation => "STORAGE_OBSERVATION",
            Self::IntegrityIssueObserved => "INTEGRITY_ISSUE_OBSERVED",
            Self::HealthTransition => "HEALTH_TRANSITION",
            Self::RecoveryClassification => "RECOVERY_CLASSIFICATION",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreOperationKind {
    None,
    HandshakeV1,
    AssetIngestCopyV1,
    LibraryStatusV1,
    LibraryVerifyV1,
    LibraryIntegrityIssuesListV1,
    AssetInspectV1,
    AssetListV1,
    AssetMaterializeV1,
    AssetRevisionCreateV1,
    AssetRetireV1,
    AssetRestoreV1,
    ProjectCreateV1,
    ProjectSpecReviseV1,
    ProjectListV1,
    SubjectCreateV1,
    SubjectListV1,
    WorkCreateV1,
    WorkReviseV1,
    WorkListV1,
    TakeCreateV1,
    TakeTransitionV1,
    TakeReopenV1,
    TakeListV1,
    StartupLocalClassificationV1,
}

impl CoreOperationKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::HandshakeV1 => "HANDSHAKE_V1",
            Self::AssetIngestCopyV1 => "ASSET_INGEST_COPY_V1",
            Self::LibraryStatusV1 => "LIBRARY_STATUS_V1",
            Self::LibraryVerifyV1 => "LIBRARY_VERIFY_V1",
            Self::LibraryIntegrityIssuesListV1 => "LIBRARY_INTEGRITY_ISSUES_LIST_V1",
            Self::AssetInspectV1 => "ASSET_INSPECT_V1",
            Self::AssetListV1 => "ASSET_LIST_V1",
            Self::AssetMaterializeV1 => "ASSET_MATERIALIZE_V1",
            Self::AssetRevisionCreateV1 => "ASSET_REVISION_CREATE_V1",
            Self::AssetRetireV1 => "ASSET_RETIRE_V1",
            Self::AssetRestoreV1 => "ASSET_RESTORE_V1",
            Self::ProjectCreateV1 => "PROJECT_CREATE_V1",
            Self::ProjectSpecReviseV1 => "PROJECT_SPEC_REVISE_V1",
            Self::ProjectListV1 => "PROJECT_LIST_V1",
            Self::SubjectCreateV1 => "SUBJECT_CREATE_V1",
            Self::SubjectListV1 => "SUBJECT_LIST_V1",
            Self::WorkCreateV1 => "WORK_CREATE_V1",
            Self::WorkReviseV1 => "WORK_REVISE_V1",
            Self::WorkListV1 => "WORK_LIST_V1",
            Self::TakeCreateV1 => "TAKE_CREATE_V1",
            Self::TakeTransitionV1 => "TAKE_TRANSITION_V1",
            Self::TakeReopenV1 => "TAKE_REOPEN_V1",
            Self::TakeListV1 => "TAKE_LIST_V1",
            Self::StartupLocalClassificationV1 => "STARTUP_LOCAL_CLASSIFICATION_V1",
        }
    }

    const fn requires_command(self) -> bool {
        matches!(
            self,
            Self::AssetIngestCopyV1
                | Self::AssetMaterializeV1
                | Self::AssetRevisionCreateV1
                | Self::AssetRetireV1
                | Self::AssetRestoreV1
                | Self::ProjectCreateV1
                | Self::ProjectSpecReviseV1
                | Self::SubjectCreateV1
                | Self::WorkCreateV1
                | Self::WorkReviseV1
                | Self::TakeCreateV1
                | Self::TakeTransitionV1
                | Self::TakeReopenV1
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreOutcomeKind {
    None,
    Started,
    Succeeded,
    Replayed,
    TerminalRejected,
    RecoveryRequired,
    Cancelled,
    DeadlineExceeded,
    Backpressure,
    Failed,
}

impl CoreOutcomeKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Started => "STARTED",
            Self::Succeeded => "SUCCEEDED",
            Self::Replayed => "REPLAYED",
            Self::TerminalRejected => "TERMINAL_REJECTED",
            Self::RecoveryRequired => "RECOVERY_REQUIRED",
            Self::Cancelled => "CANCELLED",
            Self::DeadlineExceeded => "DEADLINE_EXCEEDED",
            Self::Backpressure => "BACKPRESSURE",
            Self::Failed => "FAILED",
        }
    }

    const fn is_success(self) -> bool {
        matches!(self, Self::Succeeded | Self::Replayed)
    }
}

#[derive(Clone, Copy)]
pub struct CoreLogContext {
    request_id: Id<CoreRequestIdentity>,
    correlation_id: Id<CoreCorrelationIdentity>,
    command_id: Option<Id<Command>>,
}

impl CoreLogContext {
    #[must_use]
    pub const fn new(
        request_id: Id<CoreRequestIdentity>,
        correlation_id: Id<CoreCorrelationIdentity>,
        command_id: Option<Id<Command>>,
    ) -> Self {
        Self {
            request_id,
            correlation_id,
            command_id,
        }
    }
}

pub struct CoreLogEventDraft {
    pub timestamp: Option<Timestamp>,
    pub severity: CoreLogSeverity,
    pub event_kind: CoreLogEventKind,
    pub operation_kind: CoreOperationKind,
    pub outcome_kind: CoreOutcomeKind,
    pub context: Option<CoreLogContext>,
    pub error_code: Option<ErrorCode>,
    pub retryable: Option<bool>,
    pub duration_ms: Option<u64>,
    pub issue_kind: Option<IntegrityIssueKind>,
}

pub struct CoreLogEvent {
    draft: CoreLogEventDraft,
}

impl CoreLogEvent {
    pub fn new(draft: CoreLogEventDraft) -> Result<Self, ErrorCode> {
        validate_log_event(&draft)?;
        Ok(Self { draft })
    }

    #[must_use]
    pub const fn severity(&self) -> CoreLogSeverity {
        self.draft.severity
    }
}

fn validate_log_event(event: &CoreLogEventDraft) -> Result<(), ErrorCode> {
    let context_command_matches = event.context.is_none_or(|context| {
        context.command_id.is_some() == event.operation_kind.requires_command()
    });
    let valid = match event.event_kind {
        CoreLogEventKind::ServiceStarting
        | CoreLogEventKind::ServiceReady
        | CoreLogEventKind::ServiceStopping
        | CoreLogEventKind::HealthTransition => {
            event.operation_kind == CoreOperationKind::None
                && event.outcome_kind == CoreOutcomeKind::None
                && event.context.is_none()
                && event.error_code.is_none()
                && event.retryable.is_none()
                && event.duration_ms.is_none()
                && event.issue_kind.is_none()
        }
        CoreLogEventKind::OperationStarted => {
            event.operation_kind != CoreOperationKind::None
                && event.operation_kind != CoreOperationKind::StartupLocalClassificationV1
                && event.outcome_kind == CoreOutcomeKind::Started
                && event.context.is_some()
                && context_command_matches
                && event.error_code.is_none()
                && event.retryable.is_none()
                && event.duration_ms.is_none()
                && event.issue_kind.is_none()
        }
        CoreLogEventKind::OperationCompleted => {
            event.operation_kind != CoreOperationKind::None
                && event.operation_kind != CoreOperationKind::StartupLocalClassificationV1
                && event.outcome_kind.is_success()
                && event.context.is_some()
                && context_command_matches
                && event.error_code.is_none()
                && event.retryable.is_none()
                && event.duration_ms.is_some()
                && event.issue_kind.is_none()
        }
        CoreLogEventKind::OperationFailed => {
            event.operation_kind != CoreOperationKind::None
                && event.operation_kind != CoreOperationKind::StartupLocalClassificationV1
                && !matches!(
                    event.outcome_kind,
                    CoreOutcomeKind::None
                        | CoreOutcomeKind::Started
                        | CoreOutcomeKind::Succeeded
                        | CoreOutcomeKind::Replayed
                )
                && event.context.is_some()
                && context_command_matches
                && event.error_code.is_some()
                && event.retryable.is_some()
                && event.duration_ms.is_some()
                && event.issue_kind.is_none()
        }
        CoreLogEventKind::DbObservation | CoreLogEventKind::StorageObservation => {
            event.operation_kind != CoreOperationKind::None
                && event.outcome_kind != CoreOutcomeKind::None
                && event.outcome_kind != CoreOutcomeKind::Started
                && event.context.is_none_or(|_| context_command_matches)
                && (event.outcome_kind.is_success()
                    == (event.error_code.is_none() && event.retryable.is_none()))
                && event.duration_ms.is_some()
                && event.issue_kind.is_none()
        }
        CoreLogEventKind::IntegrityIssueObserved => {
            event.operation_kind == CoreOperationKind::LibraryVerifyV1
                && event.outcome_kind == CoreOutcomeKind::Succeeded
                && event.context.is_some()
                && event
                    .context
                    .is_some_and(|context| context.command_id.is_none())
                && event.error_code.is_none()
                && event.retryable.is_none()
                && event.duration_ms.is_none()
                && event.issue_kind.is_some()
        }
        CoreLogEventKind::RecoveryClassification => {
            event.operation_kind == CoreOperationKind::StartupLocalClassificationV1
                && !matches!(
                    event.outcome_kind,
                    CoreOutcomeKind::None | CoreOutcomeKind::Started
                )
                && event.context.is_none()
                && (event.outcome_kind.is_success()
                    == (event.error_code.is_none() && event.retryable.is_none()))
                && event.duration_ms.is_some()
                && event.issue_kind.is_none()
        }
    };
    if valid {
        Ok(())
    } else {
        Err(ErrorCode::InternalError)
    }
}

struct BuildVersion(String);

impl BuildVersion {
    fn new(value: &str) -> Result<Self, ErrorCode> {
        if value.is_empty()
            || value.len() > BUILD_VERSION_BYTES_MAX
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._+-".contains(&byte))
        {
            return Err(ErrorCode::StorageConfigurationError);
        }
        Ok(Self(value.to_owned()))
    }
}

pub struct CoreLogEncoder {
    build_version: BuildVersion,
}

impl CoreLogEncoder {
    pub fn new(build_version: &str) -> Result<Self, ErrorCode> {
        Ok(Self {
            build_version: BuildVersion::new(build_version)?,
        })
    }

    pub fn encode(&self, event: &CoreLogEvent) -> Result<Vec<u8>, ErrorCode> {
        let draft = &event.draft;
        let timestamp = draft
            .timestamp
            .map_or_else(|| "UNAVAILABLE".to_owned(), |value| value.to_string());
        let (request_id, correlation_id, command_id) = match draft.context {
            Some(context) => (
                context.request_id.to_string(),
                context.correlation_id.to_string(),
                context
                    .command_id
                    .map_or_else(|| "-".to_owned(), |value| value.to_string()),
            ),
            None => ("-".to_owned(), "-".to_owned(), "-".to_owned()),
        };
        let error_code = draft.error_code.map_or("-", ErrorCode::as_str);
        let retryable = draft
            .retryable
            .map_or("-", |value| if value { "true" } else { "false" });
        let duration = draft
            .duration_ms
            .map_or_else(|| "-".to_owned(), |value| value.to_string());
        let issue = draft.issue_kind.map_or("-", issue_kind_str);
        let line = format!(
            "schema_version=1 timestamp={timestamp} severity={} service=MENGXIAD build_version={} event_kind={} operation_kind={} outcome_kind={} request_id={request_id} correlation_id={correlation_id} command_id={command_id} causation_id=- error_code={error_code} retryable={retryable} duration_ms={duration} issue_kind={issue}",
            draft.severity.as_str(),
            self.build_version.0,
            draft.event_kind.as_str(),
            draft.operation_kind.as_str(),
            draft.outcome_kind.as_str(),
        );
        if line.len() > CORE_LOG_LINE_BYTES_MAX || !line.is_ascii() {
            return Err(ErrorCode::InternalError);
        }
        Ok(line.into_bytes())
    }
}

fn issue_kind_str(kind: IntegrityIssueKind) -> &'static str {
    match kind {
        IntegrityIssueKind::DatabaseIntegrityFailure => "DATABASE_INTEGRITY_FAILURE",
        IntegrityIssueKind::SchemaOrMigrationMismatch => "SCHEMA_OR_MIGRATION_MISMATCH",
        IntegrityIssueKind::LibraryAuthorityMismatch => "LIBRARY_AUTHORITY_MISMATCH",
        IntegrityIssueKind::CommandRecoveryRequired => "COMMAND_RECOVERY_REQUIRED",
        IntegrityIssueKind::EventOrGraphInconsistent => "EVENT_OR_GRAPH_INCONSISTENT",
        IntegrityIssueKind::LocalBackendMismatch => "LOCAL_BACKEND_MISMATCH",
        IntegrityIssueKind::ManagedBlobMissing => "MANAGED_BLOB_MISSING",
        IntegrityIssueKind::ManagedBlobUnsafe => "MANAGED_BLOB_UNSAFE",
        IntegrityIssueKind::ManagedBlobLengthMismatch => "MANAGED_BLOB_LENGTH_MISMATCH",
        IntegrityIssueKind::ManagedBlobDigestMismatch => "MANAGED_BLOB_DIGEST_MISMATCH",
        IntegrityIssueKind::UnregisteredCanonicalBlob => "UNREGISTERED_CANONICAL_BLOB",
        IntegrityIssueKind::StagingOrphan => "STAGING_ORPHAN",
        IntegrityIssueKind::UnsafeCasNamespaceEntry => "UNSAFE_CAS_NAMESPACE_ENTRY",
        IntegrityIssueKind::MaterializationRecoveryRequired => "MATERIALIZATION_RECOVERY_REQUIRED",
    }
}

enum LogCommand {
    Line(Vec<u8>),
    Shutdown,
}

#[derive(Default)]
struct LogSinkState {
    queue_drops: u64,
    encode_drops: u64,
    writer_failed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreLogSinkSnapshot {
    pub queue_drops: u64,
    pub encode_drops: u64,
    pub writer_failed: bool,
}

pub struct CoreLogSink {
    level: CoreLogLevel,
    encoder: CoreLogEncoder,
    sender: SyncSender<LogCommand>,
    state: Arc<Mutex<LogSinkState>>,
    worker: Option<JoinHandle<()>>,
}

impl CoreLogSink {
    pub fn start(
        level: CoreLogLevel,
        build_version: &str,
        writer: Box<dyn Write + Send>,
    ) -> Result<Self, ErrorCode> {
        let encoder = CoreLogEncoder::new(build_version)?;
        let (sender, receiver) = mpsc::sync_channel(OBSERVABILITY_QUEUE_MAX);
        let state = Arc::new(Mutex::new(LogSinkState::default()));
        let worker_state = Arc::clone(&state);
        let worker = thread::Builder::new()
            .name("mengxia-core-log".into())
            .spawn(move || log_writer(receiver, writer, worker_state))
            .map_err(|_| ErrorCode::InternalError)?;
        Ok(Self {
            level,
            encoder,
            sender,
            state,
            worker: Some(worker),
        })
    }

    pub fn emit(&self, event: &CoreLogEvent) {
        if !self.level.allows(event.severity()) {
            return;
        }
        let line = match self.encoder.encode(event) {
            Ok(line) => line,
            Err(_) => {
                if let Ok(mut state) = self.state.lock() {
                    state.encode_drops = state.encode_drops.saturating_add(1);
                }
                return;
            }
        };
        match self.sender.try_send(LogCommand::Line(line)) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                if let Ok(mut state) = self.state.lock() {
                    state.queue_drops = state.queue_drops.saturating_add(1);
                }
            }
        }
    }

    pub fn snapshot(&self) -> Result<CoreLogSinkSnapshot, ErrorCode> {
        let state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        Ok(CoreLogSinkSnapshot {
            queue_drops: state.queue_drops,
            encode_drops: state.encode_drops,
            writer_failed: state.writer_failed,
        })
    }

    pub fn shutdown(mut self) -> Result<(), ErrorCode> {
        self.shutdown_inner()
    }

    fn shutdown_inner(&mut self) -> Result<(), ErrorCode> {
        if self.worker.is_none() {
            return Ok(());
        }
        let send_failed = self.sender.send(LogCommand::Shutdown).is_err();
        let worker = self.worker.take().ok_or(ErrorCode::InternalError)?;
        if send_failed
            || worker.join().is_err()
            || self
                .state
                .lock()
                .map_err(|_| ErrorCode::InternalError)?
                .writer_failed
        {
            Err(ErrorCode::InternalError)
        } else {
            Ok(())
        }
    }
}

impl Drop for CoreLogSink {
    fn drop(&mut self) {
        let _ = self.shutdown_inner();
    }
}

fn log_writer(
    receiver: mpsc::Receiver<LogCommand>,
    mut writer: Box<dyn Write + Send>,
    state: Arc<Mutex<LogSinkState>>,
) {
    while let Ok(command) = receiver.recv() {
        match command {
            LogCommand::Line(line) => {
                if writer
                    .write_all(&line)
                    .and_then(|_| writer.write_all(b"\n"))
                    .is_err()
                {
                    if let Ok(mut state) = state.lock() {
                        state.writer_failed = true;
                    }
                    return;
                }
            }
            LogCommand::Shutdown => return,
        }
    }
}

pub struct Sensitive<T>(T);

impl<T> Sensitive<T> {
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self(value)
    }
}

impl<T> fmt::Debug for Sensitive<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

impl<T> fmt::Display for Sensitive<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

const DURATION_BUCKETS_MS: [u64; 17] = [
    1,
    5,
    10,
    25,
    50,
    100,
    250,
    500,
    1_000,
    2_500,
    5_000,
    10_000,
    60_000,
    300_000,
    3_600_000,
    86_400_000,
    u64::MAX,
];
const METRIC_OPERATION_COUNT: usize = 22;
const METRIC_CORE_OUTCOME_COUNT: usize = 8;
const DB_KIND_COUNT: usize = 9;
const DB_OUTCOME_COUNT: usize = 6;
const STORAGE_OPERATION_COUNT: usize = 7;
const STORAGE_OUTCOME_COUNT: usize = 8;
const DIRECTION_COUNT: usize = 2;
const ISSUE_KIND_COUNT: usize = 14;
const READINESS_COUNT: usize = 2;
const DEGRADED_CLASS_COUNT: usize = 5;
const DROP_REASON_COUNT: usize = 4;
const PROVIDER_CLASS_COUNT: usize = 7;
const ERROR_CODE_COUNT: usize = 31;

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricOperation {
    AssetIngestV1,
    LibraryStatusV1,
    LibraryVerifyV1,
    LibraryIntegrityIssuesListV1,
    AssetInspectV1,
    AssetListV1,
    AssetMaterializeV1,
    AssetRevisionCreateV1,
    AssetRetireV1,
    AssetRestoreV1,
    ProjectCreateV1,
    ProjectSpecReviseV1,
    ProjectListV1,
    SubjectCreateV1,
    SubjectListV1,
    WorkCreateV1,
    WorkReviseV1,
    WorkListV1,
    TakeCreateV1,
    TakeTransitionV1,
    TakeReopenV1,
    TakeListV1,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetricCoreOutcome {
    Succeeded,
    Replayed,
    TerminalRejected,
    RecoveryRequired,
    Cancelled,
    DeadlineExceeded,
    Backpressure,
    Failed,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DbMetricKind {
    StartupCheck,
    QueryPage,
    VerifyPage,
    CommandObserve,
    CommandClaim,
    CommandReacquire,
    CommandComplete,
    CommandDisposition,
    RecoveryClassify,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DbMetricOutcome {
    ReadSucceeded,
    Committed,
    RolledBack,
    Busy,
    Interrupted,
    Failed,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageMetricOperation {
    CasRead,
    VerifyMetadata,
    VerifyHash,
    MaterializeCopy,
    MaterializeSync,
    MaterializePublish,
    MaterializeCleanup,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageMetricOutcome {
    Succeeded,
    ExistingVerified,
    Cancelled,
    DeadlineExceeded,
    ConfigurationError,
    IoError,
    Corruption,
    Failed,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageDirection {
    Read,
    Write,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealthDegradedClass {
    None,
    CustodyReadOnly,
    CustodyBlob,
    Dependency,
    Observability,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservabilityDropReason {
    QueueFull,
    WriterFailed,
    CounterSaturated,
    EncodeRejected,
}

#[repr(usize)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderMetricClass {
    Unavailable,
    Authentication,
    RateLimited,
    Transient,
    Permanent,
    Malformed,
    UnknownRedacted,
}

struct CoreMetricsState {
    core_commands: [[u64; METRIC_CORE_OUTCOME_COUNT]; METRIC_OPERATION_COUNT],
    core_duration:
        [[[u64; DURATION_BUCKETS_MS.len()]; METRIC_CORE_OUTCOME_COUNT]; METRIC_OPERATION_COUNT],
    errors: [u64; ERROR_CODE_COUNT],
    provider_errors: [u64; PROVIDER_CLASS_COUNT],
    db_duration: [[[u64; DURATION_BUCKETS_MS.len()]; DB_OUTCOME_COUNT]; DB_KIND_COUNT],
    db_write_queue_depth: u64,
    db_wal_bytes: u64,
    storage_bytes: [[u64; DIRECTION_COUNT]; STORAGE_OPERATION_COUNT],
    storage_duration:
        [[[u64; DURATION_BUCKETS_MS.len()]; STORAGE_OUTCOME_COUNT]; STORAGE_OPERATION_COUNT],
    staging_orphan_count: u64,
    staging_orphan_bytes: u64,
    integrity_issues: [u64; ISSUE_KIND_COUNT],
    health_transitions: [[u64; DEGRADED_CLASS_COUNT]; READINESS_COUNT],
    drops: [u64; DROP_REASON_COUNT],
    saturated: bool,
}

impl Default for CoreMetricsState {
    fn default() -> Self {
        Self {
            core_commands: [[0; METRIC_CORE_OUTCOME_COUNT]; METRIC_OPERATION_COUNT],
            core_duration: [[[0; DURATION_BUCKETS_MS.len()]; METRIC_CORE_OUTCOME_COUNT];
                METRIC_OPERATION_COUNT],
            errors: [0; ERROR_CODE_COUNT],
            provider_errors: [0; PROVIDER_CLASS_COUNT],
            db_duration: [[[0; DURATION_BUCKETS_MS.len()]; DB_OUTCOME_COUNT]; DB_KIND_COUNT],
            db_write_queue_depth: 0,
            db_wal_bytes: 0,
            storage_bytes: [[0; DIRECTION_COUNT]; STORAGE_OPERATION_COUNT],
            storage_duration: [[[0; DURATION_BUCKETS_MS.len()]; STORAGE_OUTCOME_COUNT];
                STORAGE_OPERATION_COUNT],
            staging_orphan_count: 0,
            staging_orphan_bytes: 0,
            integrity_issues: [0; ISSUE_KIND_COUNT],
            health_transitions: [[0; DEGRADED_CLASS_COUNT]; READINESS_COUNT],
            drops: [0; DROP_REASON_COUNT],
            saturated: false,
        }
    }
}

#[derive(Default)]
pub struct CoreMetrics {
    state: Mutex<CoreMetricsState>,
}

impl CoreMetrics {
    pub fn record_core_terminal(
        &self,
        operation: MetricOperation,
        outcome: MetricCoreOutcome,
        duration_ms: u64,
        error: Option<(ErrorCode, Option<ProviderMetricClass>)>,
    ) -> Result<(), ErrorCode> {
        let outcome_succeeded = matches!(
            outcome,
            MetricCoreOutcome::Succeeded | MetricCoreOutcome::Replayed
        );
        if outcome_succeeded != error.is_none() {
            return Err(ErrorCode::InternalError);
        }
        if let Some((code, provider_class)) = error {
            let provider = matches!(
                code,
                ErrorCode::ProviderValidation
                    | ErrorCode::InvalidCredential
                    | ErrorCode::ProviderRateLimited
                    | ErrorCode::ProviderTimeout
                    | ErrorCode::ProviderUnavailable
            );
            if provider != provider_class.is_some() {
                return Err(ErrorCode::InternalError);
            }
            if code != ErrorCode::NotFound
                && provider_class.is_none()
                && error_index(code).is_none()
            {
                return Err(ErrorCode::InternalError);
            }
        }
        let mut state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        let mut saturated =
            increment(&mut state.core_commands[operation as usize][outcome as usize]);
        saturated |= increment(
            &mut state.core_duration[operation as usize][outcome as usize]
                [duration_bucket(duration_ms)],
        );
        if let Some((code, provider_class)) = error {
            if let Some(class) = provider_class {
                saturated |= increment(&mut state.provider_errors[class as usize]);
            } else if code != ErrorCode::NotFound {
                let index = error_index(code).ok_or(ErrorCode::InternalError)?;
                saturated |= increment(&mut state.errors[index]);
            }
        }
        mark_saturation(&mut state, saturated);
        Ok(())
    }

    pub fn record_db(
        &self,
        kind: DbMetricKind,
        outcome: DbMetricOutcome,
        duration_ms: u64,
    ) -> Result<(), ErrorCode> {
        let mut state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        let saturated = increment(
            &mut state.db_duration[kind as usize][outcome as usize][duration_bucket(duration_ms)],
        );
        mark_saturation(&mut state, saturated);
        Ok(())
    }

    pub fn record_storage(
        &self,
        operation: StorageMetricOperation,
        outcome: StorageMetricOutcome,
        direction: StorageDirection,
        bytes: u64,
        duration_ms: u64,
    ) -> Result<(), ErrorCode> {
        let mut state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        let mut saturated = add(
            &mut state.storage_bytes[operation as usize][direction as usize],
            bytes,
        );
        saturated |= increment(
            &mut state.storage_duration[operation as usize][outcome as usize]
                [duration_bucket(duration_ms)],
        );
        mark_saturation(&mut state, saturated);
        Ok(())
    }

    pub fn record_integrity_issue(&self, kind: IntegrityIssueKind) -> Result<(), ErrorCode> {
        let mut state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        let saturated = increment(&mut state.integrity_issues[issue_index(kind)]);
        mark_saturation(&mut state, saturated);
        Ok(())
    }

    pub fn record_health_transition(
        &self,
        readiness: CoreReadiness,
        degraded: HealthDegradedClass,
    ) -> Result<(), ErrorCode> {
        let mut state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        let saturated =
            increment(&mut state.health_transitions[readiness_index(readiness)][degraded as usize]);
        mark_saturation(&mut state, saturated);
        Ok(())
    }

    pub fn record_drop(&self, reason: ObservabilityDropReason) -> Result<(), ErrorCode> {
        let mut state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        let saturated = increment(&mut state.drops[reason as usize]);
        mark_saturation(&mut state, saturated);
        Ok(())
    }

    pub fn set_db_gauges(&self, queue_depth: u64, wal_bytes: u64) -> Result<(), ErrorCode> {
        let mut state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        state.db_write_queue_depth = queue_depth;
        state.db_wal_bytes = wal_bytes;
        Ok(())
    }

    pub fn set_staging_gauges(&self, count: u16, bytes: u64) -> Result<(), ErrorCode> {
        let mut state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        state.staging_orphan_count = u64::from(count);
        state.staging_orphan_bytes = bytes;
        Ok(())
    }

    pub fn core_count(
        &self,
        operation: MetricOperation,
        outcome: MetricCoreOutcome,
    ) -> Result<u64, ErrorCode> {
        let state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        Ok(state.core_commands[operation as usize][outcome as usize])
    }

    pub fn drop_count(&self, reason: ObservabilityDropReason) -> Result<u64, ErrorCode> {
        let state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        Ok(state.drops[reason as usize])
    }

    pub fn is_degraded(&self) -> Result<bool, ErrorCode> {
        let state = self.state.lock().map_err(|_| ErrorCode::InternalError)?;
        Ok(state.saturated)
    }

    #[cfg(test)]
    fn set_core_count_for_test(
        &self,
        operation: MetricOperation,
        outcome: MetricCoreOutcome,
        value: u64,
    ) {
        self.state.lock().unwrap().core_commands[operation as usize][outcome as usize] = value;
    }
}

fn duration_bucket(duration_ms: u64) -> usize {
    DURATION_BUCKETS_MS
        .iter()
        .position(|upper| duration_ms <= *upper)
        .expect("final duration bucket is unbounded")
}

fn increment(value: &mut u64) -> bool {
    add(value, 1)
}

fn add(value: &mut u64, amount: u64) -> bool {
    match value.checked_add(amount) {
        Some(next) => {
            *value = next;
            false
        }
        None => {
            *value = u64::MAX;
            true
        }
    }
}

fn mark_saturation(state: &mut CoreMetricsState, saturated: bool) {
    if saturated && !state.saturated {
        state.saturated = true;
        state.drops[ObservabilityDropReason::CounterSaturated as usize] =
            state.drops[ObservabilityDropReason::CounterSaturated as usize].saturating_add(1);
    }
}

fn readiness_index(readiness: CoreReadiness) -> usize {
    match readiness {
        CoreReadiness::NotReady => 0,
        CoreReadiness::Ready => 1,
    }
}

fn issue_index(kind: IntegrityIssueKind) -> usize {
    match kind {
        IntegrityIssueKind::DatabaseIntegrityFailure => 0,
        IntegrityIssueKind::SchemaOrMigrationMismatch => 1,
        IntegrityIssueKind::LibraryAuthorityMismatch => 2,
        IntegrityIssueKind::CommandRecoveryRequired => 3,
        IntegrityIssueKind::EventOrGraphInconsistent => 4,
        IntegrityIssueKind::LocalBackendMismatch => 5,
        IntegrityIssueKind::ManagedBlobMissing => 6,
        IntegrityIssueKind::ManagedBlobUnsafe => 7,
        IntegrityIssueKind::ManagedBlobLengthMismatch => 8,
        IntegrityIssueKind::ManagedBlobDigestMismatch => 9,
        IntegrityIssueKind::UnregisteredCanonicalBlob => 10,
        IntegrityIssueKind::StagingOrphan => 11,
        IntegrityIssueKind::UnsafeCasNamespaceEntry => 12,
        IntegrityIssueKind::MaterializationRecoveryRequired => 13,
    }
}

fn error_index(code: ErrorCode) -> Option<usize> {
    Some(match code {
        ErrorCode::ValidationError => 0,
        ErrorCode::AuthenticationError => 1,
        ErrorCode::AuthorizationDenied => 2,
        ErrorCode::NotFound => 3,
        ErrorCode::Conflict => 4,
        ErrorCode::InvalidTransition => 5,
        ErrorCode::SourceModifiedDuringIngest => 6,
        ErrorCode::StorageIoError => 7,
        ErrorCode::StorageCorruption => 8,
        ErrorCode::StorageBusy => 9,
        ErrorCode::StorageConfigurationError => 10,
        ErrorCode::IpcTransportError => 11,
        ErrorCode::ProtocolVersionUnsupported => 12,
        ErrorCode::DeadlineExceeded => 13,
        ErrorCode::OperationCancelled => 14,
        ErrorCode::ProviderValidation => 15,
        ErrorCode::InvalidCredential => 16,
        ErrorCode::ProviderRateLimited => 17,
        ErrorCode::ProviderTimeout => 18,
        ErrorCode::ProviderUnavailable => 19,
        ErrorCode::SubmissionUnknown => 20,
        ErrorCode::PluginProtocolViolation => 21,
        ErrorCode::SandboxUnavailable => 22,
        ErrorCode::PluginRevoked => 23,
        ErrorCode::Backpressure => 24,
        ErrorCode::InternalError => 25,
        ErrorCode::CommandInProgress => 26,
        ErrorCode::AdminAuthUnavailable => 27,
        ErrorCode::UnsupportedCapability => 28,
        ErrorCode::IdGenerationUnavailable => 29,
        ErrorCode::RevisionExhausted => 30,
        _ => return None,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreLiveness {
    Live,
    Stopping,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreReadiness {
    NotReady,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreAvailability {
    Full,
    ReadOnlyCustody,
    DegradedCustody,
    DegradedDependency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalSecurityBaseline {
    Verified,
    Unsafe,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodyObservation {
    Unassessed,
    NormalVerified,
    DeepVerified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadinessBlockReason {
    None,
    LocalRecoveryPending,
    LocalRecoveryOperatorAction,
}

#[derive(Clone, Copy)]
pub struct LibraryHealthInput {
    pub liveness: CoreLiveness,
    pub readiness_block_reason: ReadinessBlockReason,
    pub local_security_baseline: LocalSecurityBaseline,
    pub custody_observation: CustodyObservation,
    pub staging_orphan_count: u16,
    pub staging_orphan_bytes: u64,
    pub local_backend_matches: bool,
    pub observability_degraded: bool,
    pub recovery_observation_available: bool,
    pub recovery_required_command_count: u64,
    pub custody_degraded: bool,
    pub dependency_degraded: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LibraryHealthState {
    liveness: CoreLiveness,
    readiness: CoreReadiness,
    availability: CoreAvailability,
    local_security_baseline: LocalSecurityBaseline,
    custody_observation: CustodyObservation,
    readiness_block_reason: ReadinessBlockReason,
    can_read_metadata: bool,
    can_verify: bool,
    can_ingest: bool,
    can_materialize: bool,
    staging_orphan_count: u16,
    staging_orphan_bytes: u64,
    local_backend_matches: bool,
    observability_degraded: bool,
    recovery_observation_available: bool,
    recovery_required_command_count: u64,
}

impl LibraryHealthState {
    pub fn new(input: LibraryHealthInput) -> Result<Self, ErrorCode> {
        if (!input.recovery_observation_available && input.recovery_required_command_count != 0)
            || (input.readiness_block_reason == ReadinessBlockReason::None
                && !input.recovery_observation_available)
        {
            return Err(ErrorCode::InternalError);
        }
        let readiness = if input.readiness_block_reason == ReadinessBlockReason::None {
            CoreReadiness::Ready
        } else {
            CoreReadiness::NotReady
        };
        if readiness == CoreReadiness::Ready
            && (input.liveness != CoreLiveness::Live
                || input.local_security_baseline != LocalSecurityBaseline::Verified)
        {
            return Err(ErrorCode::InternalError);
        }
        let availability = if !input.local_backend_matches {
            CoreAvailability::ReadOnlyCustody
        } else if input.custody_degraded {
            CoreAvailability::DegradedCustody
        } else if input.dependency_degraded {
            CoreAvailability::DegradedDependency
        } else {
            CoreAvailability::Full
        };
        let (can_read_metadata, can_verify, can_ingest, can_materialize) =
            if readiness == CoreReadiness::NotReady || input.liveness != CoreLiveness::Live {
                (false, false, false, false)
            } else {
                match availability {
                    CoreAvailability::ReadOnlyCustody => (true, true, false, false),
                    CoreAvailability::Full
                    | CoreAvailability::DegradedCustody
                    | CoreAvailability::DegradedDependency => (true, true, true, true),
                }
            };
        Ok(Self {
            liveness: input.liveness,
            readiness,
            availability,
            local_security_baseline: input.local_security_baseline,
            custody_observation: input.custody_observation,
            readiness_block_reason: input.readiness_block_reason,
            can_read_metadata,
            can_verify,
            can_ingest,
            can_materialize,
            staging_orphan_count: input.staging_orphan_count,
            staging_orphan_bytes: input.staging_orphan_bytes,
            local_backend_matches: input.local_backend_matches,
            observability_degraded: input.observability_degraded,
            recovery_observation_available: input.recovery_observation_available,
            recovery_required_command_count: input.recovery_required_command_count,
        })
    }

    #[must_use]
    pub const fn liveness(self) -> CoreLiveness {
        self.liveness
    }

    #[must_use]
    pub const fn readiness(self) -> CoreReadiness {
        self.readiness
    }

    #[must_use]
    pub const fn availability(self) -> CoreAvailability {
        self.availability
    }

    #[must_use]
    pub const fn local_security_baseline(self) -> LocalSecurityBaseline {
        self.local_security_baseline
    }

    #[must_use]
    pub const fn custody_observation(self) -> CustodyObservation {
        self.custody_observation
    }

    #[must_use]
    pub const fn readiness_block_reason(self) -> ReadinessBlockReason {
        self.readiness_block_reason
    }

    #[must_use]
    pub const fn can_read_metadata(self) -> bool {
        self.can_read_metadata
    }

    /// Whether local semantic metadata mutations are safe in the current
    /// availability state. Read-only custody intentionally permits inspection
    /// and verification while denying every durable mutation.
    #[must_use]
    pub fn can_mutate_metadata(self) -> bool {
        self.readiness == CoreReadiness::Ready
            && self.liveness == CoreLiveness::Live
            && !matches!(self.availability, CoreAvailability::ReadOnlyCustody)
    }

    #[must_use]
    pub const fn can_verify(self) -> bool {
        self.can_verify
    }

    #[must_use]
    pub const fn can_ingest(self) -> bool {
        self.can_ingest
    }

    #[must_use]
    pub const fn can_materialize(self) -> bool {
        self.can_materialize
    }

    #[must_use]
    pub const fn staging_orphan_count(self) -> u16 {
        self.staging_orphan_count
    }

    #[must_use]
    pub const fn staging_orphan_bytes(self) -> u64 {
        self.staging_orphan_bytes
    }

    #[must_use]
    pub const fn local_backend_matches(self) -> bool {
        self.local_backend_matches
    }

    #[must_use]
    pub const fn observability_degraded(self) -> bool {
        self.observability_degraded
    }

    #[must_use]
    pub const fn recovery_observation_available(self) -> bool {
        self.recovery_observation_available
    }

    #[must_use]
    pub const fn recovery_required_command_count(self) -> u64 {
        self.recovery_required_command_count
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    struct BlockingWriter {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
        blocked: AtomicBool,
        output: Arc<Mutex<Vec<u8>>>,
    }

    impl Write for BlockingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if !self.blocked.swap(true, Ordering::SeqCst) {
                self.entered.wait();
                self.release.wait();
            }
            self.output.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn service_event() -> CoreLogEvent {
        CoreLogEvent::new(CoreLogEventDraft {
            timestamp: None,
            severity: CoreLogSeverity::Info,
            event_kind: CoreLogEventKind::ServiceStarting,
            operation_kind: CoreOperationKind::None,
            outcome_kind: CoreOutcomeKind::None,
            context: None,
            error_code: None,
            retryable: None,
            duration_ms: None,
            issue_kind: None,
        })
        .unwrap()
    }

    fn context(command: bool) -> CoreLogContext {
        CoreLogContext::new(
            Id::try_new().unwrap(),
            Id::try_new().unwrap(),
            command.then(|| Id::try_new().unwrap()),
        )
    }

    #[test]
    fn log_matrix_encoder_filter_and_sensitive_redaction_are_closed() {
        let event = CoreLogEvent::new(CoreLogEventDraft {
            timestamp: None,
            severity: CoreLogSeverity::Info,
            event_kind: CoreLogEventKind::OperationCompleted,
            operation_kind: CoreOperationKind::AssetMaterializeV1,
            outcome_kind: CoreOutcomeKind::Succeeded,
            context: Some(context(true)),
            error_code: None,
            retryable: None,
            duration_ms: Some(12),
            issue_kind: None,
        })
        .unwrap();
        let line = String::from_utf8(
            CoreLogEncoder::new("0.1.0+test")
                .unwrap()
                .encode(&event)
                .unwrap(),
        )
        .unwrap();
        assert!(
            line.starts_with(
                "schema_version=1 timestamp=UNAVAILABLE severity=INFO service=MENGXIAD"
            )
        );
        assert!(line.contains("operation_kind=ASSET_MATERIALIZE_V1"));
        assert!(line.contains("causation_id=-"));
        assert!(line.len() <= CORE_LOG_LINE_BYTES_MAX);
        assert!(CoreLogLevel::Error.allows(CoreLogSeverity::Alert));
        assert!(!CoreLogLevel::Error.allows(CoreLogSeverity::Warn));
        assert_eq!(format!("{}", Sensitive::new("secret\nvalue")), "[REDACTED]");
        assert_eq!(format!("{:?}", Sensitive::new(vec![1, 2, 3])), "[REDACTED]");

        let invalid = CoreLogEvent::new(CoreLogEventDraft {
            timestamp: None,
            severity: CoreLogSeverity::Info,
            event_kind: CoreLogEventKind::OperationCompleted,
            operation_kind: CoreOperationKind::AssetListV1,
            outcome_kind: CoreOutcomeKind::Succeeded,
            context: Some(context(true)),
            error_code: None,
            retryable: None,
            duration_ms: Some(1),
            issue_kind: None,
        });
        assert_eq!(invalid.err(), Some(ErrorCode::InternalError));
    }

    #[test]
    fn log_sink_is_non_blocking_bounded_and_joined() {
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let output = Arc::new(Mutex::new(Vec::new()));
        let sink = CoreLogSink::start(
            CoreLogLevel::Info,
            "0.1.0",
            Box::new(BlockingWriter {
                entered: Arc::clone(&entered),
                release: Arc::clone(&release),
                blocked: AtomicBool::new(false),
                output: Arc::clone(&output),
            }),
        )
        .unwrap();
        sink.emit(&service_event());
        entered.wait();
        for _ in 0..=OBSERVABILITY_QUEUE_MAX {
            sink.emit(&service_event());
        }
        assert_eq!(sink.snapshot().unwrap().queue_drops, 1);
        release.wait();
        sink.shutdown().unwrap();
        assert_eq!(
            output
                .lock()
                .unwrap()
                .iter()
                .filter(|byte| **byte == b'\n')
                .count(),
            OBSERVABILITY_QUEUE_MAX + 1
        );
    }

    #[test]
    fn health_constructor_enforces_priority_readiness_and_capabilities() {
        let ready = LibraryHealthInput {
            liveness: CoreLiveness::Live,
            readiness_block_reason: ReadinessBlockReason::None,
            local_security_baseline: LocalSecurityBaseline::Verified,
            custody_observation: CustodyObservation::Unassessed,
            staging_orphan_count: 0,
            staging_orphan_bytes: 0,
            local_backend_matches: true,
            observability_degraded: false,
            recovery_observation_available: true,
            recovery_required_command_count: 0,
            custody_degraded: true,
            dependency_degraded: true,
        };
        let state = LibraryHealthState::new(ready).unwrap();
        assert_eq!(state.readiness(), CoreReadiness::Ready);
        assert_eq!(state.availability(), CoreAvailability::DegradedCustody);
        assert!(state.can_materialize());
        assert!(state.can_mutate_metadata());

        let read_only = LibraryHealthState::new(LibraryHealthInput {
            local_backend_matches: false,
            ..ready
        })
        .unwrap();
        assert_eq!(read_only.readiness(), CoreReadiness::Ready);
        assert_eq!(read_only.availability(), CoreAvailability::ReadOnlyCustody);
        assert!(read_only.can_read_metadata());
        assert!(read_only.can_verify());
        assert!(!read_only.can_mutate_metadata());
        assert!(!read_only.can_ingest());
        assert!(!read_only.can_materialize());

        let not_ready = LibraryHealthState::new(LibraryHealthInput {
            readiness_block_reason: ReadinessBlockReason::LocalRecoveryPending,
            recovery_observation_available: false,
            recovery_required_command_count: 0,
            local_backend_matches: false,
            ..ready
        })
        .unwrap();
        assert_eq!(not_ready.availability(), CoreAvailability::ReadOnlyCustody);
        assert!(!not_ready.can_read_metadata());
        assert!(!not_ready.can_mutate_metadata());
        assert!(!not_ready.can_verify());

        assert_eq!(
            LibraryHealthState::new(LibraryHealthInput {
                recovery_observation_available: false,
                recovery_required_command_count: 1,
                ..ready
            })
            .err(),
            Some(ErrorCode::InternalError)
        );
    }

    #[test]
    fn metric_labels_histograms_and_counter_saturation_are_bounded() {
        let metrics = CoreMetrics::default();
        metrics
            .record_core_terminal(
                MetricOperation::LibraryVerifyV1,
                MetricCoreOutcome::Succeeded,
                86_400_001,
                None,
            )
            .unwrap();
        assert_eq!(
            metrics
                .core_count(
                    MetricOperation::LibraryVerifyV1,
                    MetricCoreOutcome::Succeeded
                )
                .unwrap(),
            1
        );
        metrics.set_core_count_for_test(
            MetricOperation::LibraryVerifyV1,
            MetricCoreOutcome::Succeeded,
            u64::MAX,
        );
        metrics
            .record_core_terminal(
                MetricOperation::LibraryVerifyV1,
                MetricCoreOutcome::Succeeded,
                1,
                None,
            )
            .unwrap();
        assert!(metrics.is_degraded().unwrap());
        assert_eq!(
            metrics
                .drop_count(ObservabilityDropReason::CounterSaturated)
                .unwrap(),
            1
        );
        assert_eq!(
            metrics
                .core_count(
                    MetricOperation::LibraryVerifyV1,
                    MetricCoreOutcome::Succeeded
                )
                .unwrap(),
            u64::MAX
        );
        assert_eq!(
            metrics.record_core_terminal(
                MetricOperation::AssetListV1,
                MetricCoreOutcome::Failed,
                1,
                Some((ErrorCode::ProviderUnavailable, None)),
            ),
            Err(ErrorCode::InternalError)
        );
    }
}
