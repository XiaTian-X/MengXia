use std::sync::Arc;

use mengxia_domain::{
    AssetGraph, AssetRecord, CreateAssetRevisionValues, RegisterManagedAssetValues, RevisionMember,
    RevisionRepresentation, RevisionResource,
};
use mengxia_ports::{
    ASSET_INGEST_COPY_V1, ASSET_MATERIALIZE_V1, ASSET_RESTORE_V1, ASSET_RETIRE_V1,
    ASSET_REVISION_CREATE_V1, AssetLifecycleCommand, AssetPortFuture, AssetRevisionResult,
    AssetStoreError, AssetUnitOfWork, BLOB_LOCATION_RECORD_V1, Command, CommandBinding,
    CommandResult, CreateAssetRevisionCommand, DeferredAssetLifecycleCommand,
    DeferredCreateAssetRevisionCommand, ExternalClaimOutcome, ExternalDisposition,
    ExternalDispositionOutcome, ExternalIngestClaim, ExternalIngestCompletion,
    ExternalIngestDisposition, IngestDirective, IngestStop, InterruptibleSqliteControl,
    LocationResult, ManagedRegistrationResult, MaterializationCommandBinding,
    MaterializationDisposition, MaterializationFinish, MaterializationObservation,
    MaterializationResult, MaterializationTransition, MaterializationUnitOfWork, MutationOutcome,
    PROJECT_CREATE_V1, PROJECT_SPEC_REVISE_V1, RecordManagedLocationCommand, SUBJECT_CREATE_V1,
    SqliteInterrupt, StartupMutationBoundary, StartupMutationClassifierPort, StartupMutationPage,
    StartupMutationPageRequest, StartupMutationState, TAKE_CREATE_V1, TAKE_REOPEN_V1,
    TAKE_TRANSITION_V1, VersionedCommandResult, VersionedResultPayload, WORK_CREATE_V1,
    WORK_REVISE_V1,
};
use mengxia_types::{ErrorCode, Id, RevisionNo, Sha256Digest, Timestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest as _, Sha256};
use tokio::sync::oneshot;

use super::StoreError;
use super::error::map_sqlite_error;
use super::lifecycle::StoreHandle;
use super::migration::OpenedLibraryMetadata;

const LOCAL_BACKEND_PREFIX: &str = "mengxia.local-cas.v1/";
const LOCAL_BACKEND_LOWER_SQL: &str = "SELECT 1 FROM locations WHERE backend_id >= 'mengxia.local-cas.v1/' AND backend_id < ?1 LIMIT 1";
const LOCAL_BACKEND_UPPER_SQL: &str = "SELECT 1 FROM locations WHERE backend_id > ?1 AND backend_id < 'mengxia.local-cas.v10' LIMIT 1";

#[derive(Clone)]
pub struct SqliteAssetStoreHandle {
    pub(crate) inner: StoreHandle,
}

impl SqliteAssetStoreHandle {
    pub(crate) const fn new(inner: StoreHandle) -> Self {
        Self { inner }
    }

    pub(crate) fn submit<T, F>(&self, operation: F) -> AssetPortFuture<'_, T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T, AssetStoreError> + Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let job = AssetWriterEnvelope::new(AssetWriterJob {
            operation: Some(operation),
            sender,
        });
        let lifecycle_receipt = self.inner.enqueue_asset(job);
        Box::pin(async move {
            let lifecycle_receipt = lifecycle_receipt.map_err(map_store_error)?;
            match receiver.await {
                Ok(result) => {
                    let lifecycle = lifecycle_receipt
                        .await
                        .map_err(|_| AssetStoreError::Internal)?;
                    match result {
                        Err(error) => Err(error),
                        Ok(value) => {
                            lifecycle.map_err(map_store_error)?;
                            Ok(value)
                        }
                    }
                }
                Err(_) => {
                    match lifecycle_receipt.await {
                        Ok(result) => result.map_err(map_store_error),
                        Err(_) => Err(AssetStoreError::Internal),
                    }?;
                    Err(AssetStoreError::Internal)
                }
            }
        })
    }

    pub(crate) const fn context(&self) -> StoreContext {
        StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        }
    }

    /// Fails closed when durable local-managed custody names a different backend.
    pub fn validate_local_managed_backend(
        &self,
        current_backend_id: &str,
    ) -> AssetPortFuture<'_, ()> {
        let valid = current_backend_id.len() == LOCAL_BACKEND_PREFIX.len() + 64
            && current_backend_id.starts_with(LOCAL_BACKEND_PREFIX)
            && current_backend_id[LOCAL_BACKEND_PREFIX.len()..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if !valid {
            return Box::pin(async { Err(AssetStoreError::StorageConfiguration) });
        }
        let candidate = current_backend_id.to_owned();
        self.submit(move |connection| validate_local_backend_rows(connection, &candidate))
    }
}

fn validate_local_backend_rows(
    connection: &Connection,
    candidate: &str,
) -> Result<(), AssetStoreError> {
    let lower = connection
        .query_row(LOCAL_BACKEND_LOWER_SQL, params![candidate], |_| Ok(()))
        .optional()
        .map_err(sqlite)?;
    let upper = connection
        .query_row(LOCAL_BACKEND_UPPER_SQL, params![candidate], |_| Ok(()))
        .optional()
        .map_err(sqlite)?;
    if lower.is_some() || upper.is_some() {
        Err(AssetStoreError::StorageConfiguration)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(crate) struct StoreContext {
    pub(crate) metadata: OpenedLibraryMetadata,
    pub(crate) runtime_id: [u8; 16],
}

struct AssetWriterJob<T, F> {
    operation: Option<F>,
    sender: oneshot::Sender<Result<T, AssetStoreError>>,
}

trait ErasedAssetWriterJob: Send {
    fn execute(self: Box<Self>, connection: &mut Connection) -> Result<(), StoreError>;
}

pub(crate) struct AssetWriterEnvelope {
    job: Box<dyn ErasedAssetWriterJob>,
}

impl AssetWriterEnvelope {
    fn new<Job>(job: Job) -> Self
    where
        Job: ErasedAssetWriterJob + 'static,
    {
        Self { job: Box::new(job) }
    }

    pub(crate) fn execute(self, connection: &mut Connection) -> Result<(), StoreError> {
        self.job.execute(connection)
    }
}

impl<T, F> ErasedAssetWriterJob for AssetWriterJob<T, F>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> Result<T, AssetStoreError> + Send + 'static,
{
    fn execute(mut self: Box<Self>, connection: &mut Connection) -> Result<(), StoreError> {
        let operation = self.operation.take().ok_or(StoreError::Internal)?;
        let result = operation(connection);
        let fatal = matches!(
            result,
            Err(AssetStoreError::StorageIo
                | AssetStoreError::StorageCorruption
                | AssetStoreError::Internal)
        );
        let _ = self.sender.send(result);
        if fatal {
            Err(StoreError::Internal)
        } else {
            Ok(())
        }
    }
}

impl AssetUnitOfWork for SqliteAssetStoreHandle {
    fn claim_external_ingest(
        &self,
        request: ExternalIngestClaim,
    ) -> AssetPortFuture<'_, ExternalClaimOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| claim_external(connection, context, request))
    }

    fn complete_external_ingest(
        &self,
        request: ExternalIngestCompletion,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| complete_external(connection, context, request))
    }

    fn finish_external_ingest(
        &self,
        request: ExternalIngestDisposition,
    ) -> AssetPortFuture<'_, ExternalDispositionOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| finish_external(connection, context, request))
    }

    fn fail_current_runtime_for_unresolved_external_ingest(&self) {
        self.inner.fail_current_runtime();
    }

    fn execute_create_revision(
        &self,
        request: CreateAssetRevisionCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| create_revision(connection, context, request))
    }

    fn execute_deferred_create_revision(
        &self,
        request: DeferredCreateAssetRevisionCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| create_revision_deferred(connection, context, request))
    }

    fn execute_record_location(
        &self,
        request: RecordManagedLocationCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| record_location(connection, context, request))
    }

    fn execute_asset_lifecycle(
        &self,
        request: AssetLifecycleCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| change_asset_lifecycle(connection, context, request))
    }

    fn execute_deferred_asset_lifecycle(
        &self,
        request: DeferredAssetLifecycleCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| change_asset_lifecycle_deferred(connection, context, request))
    }
}

impl MaterializationUnitOfWork for SqliteAssetStoreHandle {
    fn observe_materialization(
        &self,
        command: MaterializationCommandBinding,
    ) -> AssetPortFuture<'_, MaterializationObservation> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| observe_materialization(connection, context, command))
    }

    fn claim_new_materialization(
        &self,
        transition: MaterializationTransition,
    ) -> AssetPortFuture<'_, ()> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| claim_new_materialization(connection, context, transition))
    }

    fn reacquire_materialization(
        &self,
        transition: MaterializationTransition,
        expected_safe_error_code: Option<ErrorCode>,
    ) -> AssetPortFuture<'_, ()> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| {
            reacquire_materialization(connection, context, transition, expected_safe_error_code)
        })
    }

    fn complete_materialization(
        &self,
        transition: MaterializationTransition,
    ) -> AssetPortFuture<'_, MaterializationResult> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| complete_materialization(connection, context, transition))
    }

    fn finish_materialization(
        &self,
        request: MaterializationFinish,
    ) -> AssetPortFuture<'_, ExternalDispositionOutcome> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| finish_materialization(connection, context, request))
    }

    fn fail_current_runtime_for_unresolved_materialization(&self) {
        self.inner.fail_current_runtime();
    }
}

impl StartupMutationClassifierPort for SqliteAssetStoreHandle {
    fn capture_startup_mutation_boundary(
        &self,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> AssetPortFuture<'_, StartupMutationBoundary> {
        self.submit(move |connection| {
            with_controlled_interrupt(connection, control, capture_startup_mutation_boundary)
        })
    }

    fn classify_startup_mutation_page(
        &self,
        request: StartupMutationPageRequest,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> AssetPortFuture<'_, StartupMutationPage> {
        let context = StoreContext {
            metadata: self.inner.metadata(),
            runtime_id: self.inner.runtime_id(),
        };
        self.submit(move |connection| {
            with_controlled_interrupt(connection, control, |connection, control| {
                classify_startup_mutation_page(connection, context, request, control)
            })
        })
    }
}

fn map_store_error(error: StoreError) -> AssetStoreError {
    match error {
        StoreError::Configuration => AssetStoreError::StorageConfiguration,
        StoreError::IdGenerationUnavailable => AssetStoreError::IdGenerationUnavailable,
        StoreError::Busy => AssetStoreError::StorageBusy,
        StoreError::Io => AssetStoreError::StorageIo,
        StoreError::Corruption => AssetStoreError::StorageCorruption,
        StoreError::Conflict => AssetStoreError::Conflict,
        StoreError::Backpressure => AssetStoreError::Backpressure,
        StoreError::ShuttingDown => AssetStoreError::ShuttingDown,
        StoreError::Internal => AssetStoreError::Internal,
    }
}

pub(crate) fn sqlite(error: rusqlite::Error) -> AssetStoreError {
    map_store_error(map_sqlite_error(error))
}

#[derive(Debug)]
pub(crate) struct CommandRow {
    pub(crate) command_id: Vec<u8>,
    pub(crate) operation_id: String,
    pub(crate) principal_kind: String,
    pub(crate) principal_uid: i64,
    pub(crate) digest: Vec<u8>,
    pub(crate) runtime_id: Vec<u8>,
    pub(crate) state: String,
    pub(crate) result_kind: Option<String>,
    pub(crate) result_id: Option<Vec<u8>>,
    pub(crate) result_location_id: Option<Vec<u8>>,
    pub(crate) result_schema_version: Option<i64>,
    pub(crate) result_payload: Option<Vec<u8>>,
    pub(crate) result_payload_sha256: Option<Vec<u8>>,
    pub(crate) safe_error_code: Option<String>,
    pub(crate) created_at_seconds: i64,
    pub(crate) created_at_nanos: i64,
    pub(crate) updated_at_seconds: i64,
    pub(crate) updated_at_nanos: i64,
}

enum StoredRuntime {}

struct BlobFactRow {
    revision: Vec<u8>,
    byte_length: i64,
    media_type: Option<String>,
    lifecycle: String,
    verified_at_seconds: i64,
    verified_at_nanos: i64,
}

struct LocationFactRow {
    location_id: Vec<u8>,
    blob_digest: Vec<u8>,
    custody: String,
    durability: String,
    lifecycle: String,
    revision: Vec<u8>,
    verified_at_seconds: i64,
    verified_at_nanos: i64,
}

pub(crate) fn read_command(
    transaction: &Transaction<'_>,
    binding: &CommandBinding,
) -> Result<Option<CommandRow>, AssetStoreError> {
    transaction.query_row(
        "SELECT command_id, operation_id, principal_kind, principal_uid, canonical_request_digest, store_runtime_id, state, result_kind, result_id, result_location_id, result_schema_version, result_payload, result_payload_sha256, safe_error_code, created_at_seconds, created_at_nanos, updated_at_seconds, updated_at_nanos FROM commands WHERE command_id = ?1",
        params![binding.command_id().to_bytes().as_slice()],
        command_row_from_sql,
    ).optional().map_err(sqlite)
}

fn capture_startup_mutation_boundary(
    connection: &mut Connection,
    control: &Arc<dyn InterruptibleSqliteControl>,
) -> Result<StartupMutationBoundary, AssetStoreError> {
    controlled_checkpoint(control)?;
    let maximum: Option<Vec<u8>> = connection
        .query_row("SELECT max(command_id) FROM commands", [], |row| row.get(0))
        .map_err(|error| controlled_sqlite(error, control))?;
    let maximum = maximum
        .map(|bytes| {
            Id::<Command>::from_bytes(id_bytes(Some(&bytes))?)
                .map_err(|_| AssetStoreError::StorageCorruption)
        })
        .transpose()?;
    Ok(StartupMutationBoundary::__from_store(maximum))
}

fn with_controlled_interrupt<T, F>(
    connection: &mut Connection,
    control: Arc<dyn InterruptibleSqliteControl>,
    operation: F,
) -> Result<T, AssetStoreError>
where
    F: FnOnce(&mut Connection, &Arc<dyn InterruptibleSqliteControl>) -> Result<T, AssetStoreError>,
{
    let interrupt = Box::new(StoreSqliteInterrupt(connection.get_interrupt_handle()));
    match control
        .register_interrupt(interrupt)
        .map_err(|_| AssetStoreError::Internal)?
    {
        IngestDirective::Continue => {}
        IngestDirective::Stop(stop) => return Err(controlled_stop_error(stop)),
    }
    let result = operation(connection, &control);
    control
        .clear_interrupt()
        .map_err(|_| AssetStoreError::Internal)?;
    result
}

struct StoreSqliteInterrupt(rusqlite::InterruptHandle);

impl SqliteInterrupt for StoreSqliteInterrupt {
    fn interrupt(&self) {
        self.0.interrupt();
    }
}

fn classify_startup_mutation_page(
    connection: &mut Connection,
    context: StoreContext,
    request: StartupMutationPageRequest,
    control: &Arc<dyn InterruptibleSqliteControl>,
) -> Result<StartupMutationPage, AssetStoreError> {
    controlled_checkpoint(control)?;
    let Some(boundary) = request.boundary().maximum_command_id() else {
        if request.after_command_id().is_some() {
            return Err(AssetStoreError::Validation);
        }
        return StartupMutationPage::__from_store(None, 0, true);
    };
    if request
        .after_command_id()
        .is_some_and(|after| after > boundary)
    {
        return Err(AssetStoreError::Validation);
    }

    let state = match request.state() {
        StartupMutationState::Claimed => "CLAIMED",
        StartupMutationState::RecoveryRequired => "RECOVERY_REQUIRED",
    };
    let after = request
        .after_command_id()
        .map(|command_id| command_id.to_bytes().to_vec());
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| controlled_sqlite(error, control))?;
    let rows = {
        let mut statement = transaction
            .prepare(
                "SELECT command_id, operation_id, principal_kind, principal_uid, canonical_request_digest, store_runtime_id, state, result_kind, result_id, result_location_id, result_schema_version, result_payload, result_payload_sha256, safe_error_code, created_at_seconds, created_at_nanos, updated_at_seconds, updated_at_nanos FROM commands WHERE state=?1 AND command_id<=?2 AND (?3 IS NULL OR command_id>?3) ORDER BY command_id LIMIT 256",
            )
            .map_err(|error| controlled_sqlite(error, control))?;
        statement
            .query_map(
                params![state, boundary.to_bytes().as_slice(), after],
                command_row_from_sql,
            )
            .map_err(|error| controlled_sqlite(error, control))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| controlled_sqlite(error, control))?
    };

    let mut last_command_id = None;
    let mut recovery_required_count = 0_u64;
    for raw_row in &rows {
        controlled_checkpoint(control)?;
        let command_id = Id::<Command>::from_bytes(id_bytes(Some(&raw_row.command_id))?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        let row = validate_command_row_ref(raw_row)?;
        if row.principal_uid != i64::from(context.metadata.owner_uid)
            || row.state != state
            || !matches!(
                row.operation_id.as_str(),
                operation
                    if operation == ASSET_INGEST_COPY_V1.as_str()
                        || operation == ASSET_MATERIALIZE_V1.as_str()
            )
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        match request.state() {
            StartupMutationState::Claimed => {
                if row.runtime_id.as_slice() == context.runtime_id {
                    return Err(AssetStoreError::StorageCorruption);
                }
                let changed = transaction
                    .execute(
                        "UPDATE commands SET state='RECOVERY_REQUIRED', safe_error_code='STORAGE_CONFIGURATION_ERROR', updated_at_seconds=?2, updated_at_nanos=?3 WHERE command_id=?1 AND state='CLAIMED' AND store_runtime_id=?4",
                        params![
                            row.command_id,
                            request.classified_at().unix_seconds(),
                            i64::from(request.classified_at().subsec_nanoseconds()),
                            row.runtime_id,
                        ],
                    )
                    .map_err(|error| controlled_sqlite(error, control))?;
                if changed != 1 {
                    return Err(AssetStoreError::StorageCorruption);
                }
            }
            StartupMutationState::RecoveryRequired => {
                recovery_required_count = recovery_required_count
                    .checked_add(1)
                    .ok_or(AssetStoreError::StorageCorruption)?;
            }
        }
        last_command_id = Some(command_id);
    }
    controlled_checkpoint(control)?;
    transaction
        .commit()
        .map_err(|error| controlled_sqlite(error, control))?;
    let complete = rows.len() < 256 || last_command_id == Some(boundary);
    StartupMutationPage::__from_store(last_command_id, recovery_required_count, complete)
}

fn controlled_checkpoint(
    control: &Arc<dyn InterruptibleSqliteControl>,
) -> Result<(), AssetStoreError> {
    match control.checkpoint() {
        IngestDirective::Continue => Ok(()),
        IngestDirective::Stop(stop) => Err(controlled_stop_error(stop)),
    }
}

fn controlled_stop_error(stop: IngestStop) -> AssetStoreError {
    match stop {
        IngestStop::Cancelled => AssetStoreError::OperationCancelled,
        IngestStop::DeadlineReached => AssetStoreError::DeadlineExceeded,
    }
}

fn controlled_sqlite(
    error: rusqlite::Error,
    control: &Arc<dyn InterruptibleSqliteControl>,
) -> AssetStoreError {
    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::OperationInterrupted) {
        match control.checkpoint() {
            IngestDirective::Stop(stop) => controlled_stop_error(stop),
            IngestDirective::Continue => AssetStoreError::Internal,
        }
    } else {
        sqlite(error)
    }
}

pub(crate) fn command_row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<CommandRow> {
    Ok(CommandRow {
        command_id: row.get(0)?,
        operation_id: row.get(1)?,
        principal_kind: row.get(2)?,
        principal_uid: row.get(3)?,
        digest: row.get(4)?,
        runtime_id: row.get(5)?,
        state: row.get(6)?,
        result_kind: row.get(7)?,
        result_id: row.get(8)?,
        result_location_id: row.get(9)?,
        result_schema_version: row.get(10)?,
        result_payload: row.get(11)?,
        result_payload_sha256: row.get(12)?,
        safe_error_code: row.get(13)?,
        created_at_seconds: row.get(14)?,
        created_at_nanos: row.get(15)?,
        updated_at_seconds: row.get(16)?,
        updated_at_nanos: row.get(17)?,
    })
}

fn validate_command_row_ref(row: &CommandRow) -> Result<&CommandRow, AssetStoreError> {
    Id::<Command>::from_bytes(id_bytes(Some(&row.command_id))?)
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    Id::<StoredRuntime>::from_bytes(id_bytes(Some(&row.runtime_id))?)
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    let _: [u8; 32] = row
        .digest
        .as_slice()
        .try_into()
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    if row.principal_kind != "LOCAL_OWNER_UID_V1"
        || u32::try_from(row.principal_uid).is_err()
        || row.operation_id.is_empty()
        || row.operation_id.len() > 128
        || !row.operation_id.ends_with(".v1")
        || !row
            .operation_id
            .starts_with(|character: char| character.is_ascii_lowercase())
        || row.operation_id.chars().any(|character| {
            !(character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '_' | '-'))
        })
    {
        return Err(AssetStoreError::StorageCorruption);
    }
    persisted_timestamp(row.created_at_seconds, row.created_at_nanos)?;
    persisted_timestamp(row.updated_at_seconds, row.updated_at_nanos)?;

    let code = row
        .safe_error_code
        .as_deref()
        .map(str::parse::<ErrorCode>)
        .transpose()
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    let payload_present = match (
        row.result_schema_version,
        row.result_payload.as_deref(),
        row.result_payload_sha256.as_deref(),
    ) {
        (None, None, None) => false,
        (Some(version), Some(payload), Some(digest))
            if version == 1
                && !payload.is_empty()
                && payload.len() <= 512
                && digest == Sha256::digest(payload).as_slice() =>
        {
            true
        }
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    match row.state.as_str() {
        "CLAIMED"
            if row.result_kind.is_none()
                && row.result_id.is_none()
                && row.result_location_id.is_none()
                && !payload_present
                && code.is_none() => {}
        "COMPLETED" if code.is_none() => match (row.result_kind.as_deref(), payload_present) {
            (Some("ASSET"), false) => {
                Id::<mengxia_domain::Asset>::from_bytes(id_bytes(row.result_id.as_deref())?)
                    .map_err(|_| AssetStoreError::StorageCorruption)?;
                Id::<mengxia_domain::Location>::from_bytes(id_bytes(
                    row.result_location_id.as_deref(),
                )?)
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            }
            (Some("ASSET_REVISION"), false) if row.result_location_id.is_none() => {
                Id::<mengxia_domain::AssetRevision>::from_bytes(id_bytes(
                    row.result_id.as_deref(),
                )?)
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            }
            (Some("LOCATION"), false) if row.result_location_id.is_none() => {
                Id::<mengxia_domain::Location>::from_bytes(id_bytes(row.result_id.as_deref())?)
                    .map_err(|_| AssetStoreError::StorageCorruption)?;
            }
            (Some(kind), true) if row.result_location_id.is_none() => {
                Id::<()>::from_bytes(id_bytes(row.result_id.as_deref())?)
                    .map_err(|_| AssetStoreError::StorageCorruption)?;
                VersionedResultPayload::decode(
                    kind,
                    row.result_payload
                        .as_deref()
                        .ok_or(AssetStoreError::StorageCorruption)?,
                )?;
            }
            _ => return Err(AssetStoreError::StorageCorruption),
        },
        "TERMINAL_REJECTED" | "RECOVERY_REQUIRED"
            if row.result_kind.is_none()
                && row.result_id.is_none()
                && row.result_location_id.is_none()
                && !payload_present
                && code.is_some() => {}
        _ => return Err(AssetStoreError::StorageCorruption),
    }
    validate_known_command_matrix(row, code)?;
    Ok(row)
}

pub(crate) fn validate_command_row(row: CommandRow) -> Result<CommandRow, AssetStoreError> {
    validate_command_row_ref(&row)?;
    Ok(row)
}

pub(crate) fn is_current_operation(operation: &str) -> bool {
    matches!(
        operation,
        value if value == ASSET_INGEST_COPY_V1.as_str()
            || value == ASSET_REVISION_CREATE_V1.as_str()
            || value == BLOB_LOCATION_RECORD_V1.as_str()
            || value == ASSET_MATERIALIZE_V1.as_str()
            || value == ASSET_RETIRE_V1.as_str()
            || value == ASSET_RESTORE_V1.as_str()
            || value == PROJECT_CREATE_V1.as_str()
            || value == PROJECT_SPEC_REVISE_V1.as_str()
            || value == SUBJECT_CREATE_V1.as_str()
            || value == WORK_CREATE_V1.as_str()
            || value == WORK_REVISE_V1.as_str()
            || value == TAKE_CREATE_V1.as_str()
            || value == TAKE_TRANSITION_V1.as_str()
            || value == TAKE_REOPEN_V1.as_str()
    )
}

fn validate_known_command_matrix(
    row: &CommandRow,
    code: Option<ErrorCode>,
) -> Result<(), AssetStoreError> {
    let valid = if row.operation_id == ASSET_INGEST_COPY_V1.as_str() {
        match row.state.as_str() {
            "CLAIMED" => true,
            "COMPLETED" => row.result_kind.as_deref() == Some("ASSET"),
            "TERMINAL_REJECTED" => code.is_some_and(is_external_terminal_code),
            "RECOVERY_REQUIRED" => code.is_some_and(is_external_recovery_code),
            _ => false,
        }
    } else if row.operation_id == ASSET_REVISION_CREATE_V1.as_str() {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("ASSET_REVISION"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" => false,
            _ => false,
        }
    } else if row.operation_id == BLOB_LOCATION_RECORD_V1.as_str() {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("LOCATION"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" => false,
            _ => false,
        }
    } else if row.operation_id == ASSET_MATERIALIZE_V1.as_str() {
        match row.state.as_str() {
            "CLAIMED" => true,
            "COMPLETED" => row.result_kind.as_deref() == Some("ASSET_REVISION"),
            "TERMINAL_REJECTED" => code.is_some_and(is_materialization_terminal_code),
            "RECOVERY_REQUIRED" => code.is_some_and(is_materialization_recovery_code),
            _ => false,
        }
    } else if row.operation_id == ASSET_RETIRE_V1.as_str()
        || row.operation_id == ASSET_RESTORE_V1.as_str()
    {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("ASSET_LIFECYCLE"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" | "RECOVERY_REQUIRED" => false,
            _ => false,
        }
    } else if row.operation_id == PROJECT_CREATE_V1.as_str() {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("PROJECT"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" | "RECOVERY_REQUIRED" => false,
            _ => false,
        }
    } else if row.operation_id == PROJECT_SPEC_REVISE_V1.as_str() {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("PROJECT_SPEC_REVISION"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" | "RECOVERY_REQUIRED" => false,
            _ => false,
        }
    } else if row.operation_id == SUBJECT_CREATE_V1.as_str() {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("SUBJECT"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" | "RECOVERY_REQUIRED" => false,
            _ => false,
        }
    } else if row.operation_id == WORK_CREATE_V1.as_str() {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("WORK_ITEM"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" | "RECOVERY_REQUIRED" => false,
            _ => false,
        }
    } else if row.operation_id == WORK_REVISE_V1.as_str() {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("WORK_REVISION"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" | "RECOVERY_REQUIRED" => false,
            _ => false,
        }
    } else if row.operation_id == TAKE_CREATE_V1.as_str()
        || row.operation_id == TAKE_TRANSITION_V1.as_str()
        || row.operation_id == TAKE_REOPEN_V1.as_str()
    {
        match row.state.as_str() {
            "COMPLETED" => row.result_kind.as_deref() == Some("TAKE"),
            "TERMINAL_REJECTED" => code.is_some_and(is_pure_rejection_code),
            "CLAIMED" | "RECOVERY_REQUIRED" => false,
            _ => false,
        }
    } else {
        true
    };
    if valid {
        Ok(())
    } else {
        Err(AssetStoreError::StorageCorruption)
    }
}

fn persisted_timestamp(seconds: i64, nanos: i64) -> Result<Timestamp, AssetStoreError> {
    Timestamp::from_unix_seconds_nanos(
        seconds,
        u32::try_from(nanos).map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)
}

fn is_pure_rejection_code(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::NotFound
            | ErrorCode::Conflict
            | ErrorCode::InvalidTransition
            | ErrorCode::RevisionExhausted
    )
}

fn is_external_recovery_code(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::StorageConfigurationError
            | ErrorCode::StorageIoError
            | ErrorCode::IdGenerationUnavailable
            | ErrorCode::InternalError
    )
}

fn is_external_terminal_code(code: ErrorCode) -> bool {
    matches!(
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
    )
}

fn is_materialization_recovery_code(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::StorageConfigurationError
            | ErrorCode::StorageIoError
            | ErrorCode::IdGenerationUnavailable
            | ErrorCode::InternalError
    )
}

fn is_materialization_terminal_code(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::ValidationError
            | ErrorCode::Conflict
            | ErrorCode::StorageIoError
            | ErrorCode::StorageCorruption
            | ErrorCode::StorageConfigurationError
            | ErrorCode::Backpressure
            | ErrorCode::InternalError
            | ErrorCode::IdGenerationUnavailable
            | ErrorCode::DeadlineExceeded
            | ErrorCode::OperationCancelled
    )
}

pub(crate) fn binding_matches(row: &CommandRow, binding: &CommandBinding, owner_uid: u32) -> bool {
    row.command_id.as_slice() == binding.command_id().to_bytes()
        && row.operation_id == binding.operation_id().as_str()
        && row.principal_kind == "LOCAL_OWNER_UID_V1"
        && row.principal_uid == i64::from(owner_uid)
        && row.digest.as_slice() == binding.canonical_request_digest().to_bytes()
}

pub(crate) fn insert_claim(
    transaction: &Transaction<'_>,
    context: StoreContext,
    binding: &CommandBinding,
    at: Timestamp,
) -> Result<(), AssetStoreError> {
    transaction.execute(
        "INSERT INTO commands (command_id, operation_id, principal_kind, principal_uid, canonical_request_digest, store_runtime_id, state, created_at_seconds, created_at_nanos, updated_at_seconds, updated_at_nanos) VALUES (?1, ?2, 'LOCAL_OWNER_UID_V1', ?3, ?4, ?5, 'CLAIMED', ?6, ?7, ?6, ?7)",
        params![binding.command_id().to_bytes().as_slice(), binding.operation_id().as_str(), i64::from(context.metadata.owner_uid), binding.canonical_request_digest().to_bytes().as_slice(), context.runtime_id.as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())],
    ).map_err(sqlite)?;
    Ok(())
}

fn observe_materialization(
    connection: &mut Connection,
    context: StoreContext,
    command: MaterializationCommandBinding,
) -> Result<MaterializationObservation, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let outcome = match read_command(&transaction, command.binding())? {
        None => MaterializationObservation::Absent,
        Some(row) => {
            if !binding_matches(&row, command.binding(), context.metadata.owner_uid) {
                return Err(AssetStoreError::Conflict);
            }
            let row = validate_command_row(row)?;
            match row.state.as_str() {
                "CLAIMED" if row.runtime_id.as_slice() == context.runtime_id => {
                    MaterializationObservation::InProgress
                }
                "CLAIMED" => MaterializationObservation::RecoveryCandidate {
                    safe_error_code: None,
                },
                "RECOVERY_REQUIRED" => MaterializationObservation::RecoveryCandidate {
                    safe_error_code: Some(parse_safe_code(&row)?),
                },
                "COMPLETED" => MaterializationObservation::Replay(replay_materialization(
                    &transaction,
                    &row,
                    &command,
                )?),
                "TERMINAL_REJECTED" => MaterializationObservation::TerminalRejected {
                    safe_error_code: parse_safe_code(&row)?,
                },
                _ => return Err(AssetStoreError::StorageCorruption),
            }
        }
    };
    transaction.commit().map_err(sqlite)?;
    Ok(outcome)
}

fn claim_new_materialization(
    connection: &mut Connection,
    context: StoreContext,
    transition: MaterializationTransition,
) -> Result<(), AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if read_command(&transaction, transition.command().binding())?.is_some() {
        return Err(AssetStoreError::Conflict);
    }
    insert_claim(
        &transaction,
        context,
        transition.command().binding(),
        transition.at(),
    )?;
    transaction.commit().map_err(sqlite)
}

fn reacquire_materialization(
    connection: &mut Connection,
    context: StoreContext,
    transition: MaterializationTransition,
    expected_safe_error_code: Option<ErrorCode>,
) -> Result<(), AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    let row = read_command(&transaction, transition.command().binding())?
        .ok_or(AssetStoreError::Conflict)?;
    if !binding_matches(
        &row,
        transition.command().binding(),
        context.metadata.owner_uid,
    ) {
        return Err(AssetStoreError::Conflict);
    }
    let row = validate_command_row(row)?;
    let accepted = match expected_safe_error_code {
        None => row.state == "CLAIMED" && row.runtime_id.as_slice() != context.runtime_id,
        Some(code) => {
            row.state == "RECOVERY_REQUIRED"
                && is_materialization_recovery_code(code)
                && parse_safe_code(&row)? == code
        }
    };
    if !accepted {
        return Err(AssetStoreError::Conflict);
    }
    let changed = transaction
        .execute(
            "UPDATE commands SET store_runtime_id=?2, state='CLAIMED', safe_error_code=NULL, updated_at_seconds=?3, updated_at_nanos=?4 WHERE command_id=?1 AND state=?5 AND store_runtime_id=?6 AND ((?7 IS NULL AND safe_error_code IS NULL) OR safe_error_code=?7)",
            params![
                transition.command().binding().command_id().to_bytes().as_slice(),
                context.runtime_id.as_slice(),
                transition.at().unix_seconds(),
                i64::from(transition.at().subsec_nanoseconds()),
                row.state,
                row.runtime_id,
                expected_safe_error_code.map(ErrorCode::as_str),
            ],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::Conflict);
    }
    transaction.commit().map_err(sqlite)
}

fn complete_materialization(
    connection: &mut Connection,
    context: StoreContext,
    transition: MaterializationTransition,
) -> Result<MaterializationResult, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    let row = read_command(&transaction, transition.command().binding())?
        .ok_or(AssetStoreError::Conflict)?;
    if !binding_matches(
        &row,
        transition.command().binding(),
        context.metadata.owner_uid,
    ) {
        return Err(AssetStoreError::Conflict);
    }
    let row = validate_command_row(row)?;
    if row.state != "CLAIMED" || row.runtime_id.as_slice() != context.runtime_id {
        return Err(AssetStoreError::Conflict);
    }
    let changed = transaction
        .execute(
            "UPDATE commands SET state='COMPLETED', result_kind='ASSET_REVISION', result_id=?2, safe_error_code=NULL, updated_at_seconds=?3, updated_at_nanos=?4 WHERE command_id=?1 AND state='CLAIMED' AND store_runtime_id=?5",
            params![
                transition.command().binding().command_id().to_bytes().as_slice(),
                transition.command().asset_revision_id().to_bytes().as_slice(),
                transition.at().unix_seconds(),
                i64::from(transition.at().subsec_nanoseconds()),
                context.runtime_id.as_slice(),
            ],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    let completed = validate_command_row(
        read_command(&transaction, transition.command().binding())?
            .ok_or(AssetStoreError::StorageCorruption)?,
    )?;
    let result = replay_materialization(&transaction, &completed, transition.command())?;
    transaction.commit().map_err(sqlite)?;
    Ok(result)
}

fn finish_materialization(
    connection: &mut Connection,
    context: StoreContext,
    request: MaterializationFinish,
) -> Result<ExternalDispositionOutcome, AssetStoreError> {
    let transition = request.transition();
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    let row = read_command(&transaction, transition.command().binding())?
        .ok_or(AssetStoreError::Conflict)?;
    if !binding_matches(
        &row,
        transition.command().binding(),
        context.metadata.owner_uid,
    ) {
        return Err(AssetStoreError::Conflict);
    }
    let row = validate_command_row(row)?;
    let (state, code) = match request.disposition() {
        MaterializationDisposition::TerminalRejected(code) => ("TERMINAL_REJECTED", code),
        MaterializationDisposition::RecoveryRequired(code) => ("RECOVERY_REQUIRED", code),
    };
    let outcome = if row.state == "CLAIMED" && row.runtime_id.as_slice() == context.runtime_id {
        let changed = transaction.execute(
            "UPDATE commands SET state=?2, safe_error_code=?3, updated_at_seconds=?4, updated_at_nanos=?5 WHERE command_id=?1 AND state='CLAIMED' AND store_runtime_id=?6",
            params![transition.command().binding().command_id().to_bytes().as_slice(), state, code.as_str(), transition.at().unix_seconds(), i64::from(transition.at().subsec_nanoseconds()), context.runtime_id.as_slice()],
        ).map_err(sqlite)?;
        if changed != 1 {
            return Err(AssetStoreError::StorageCorruption);
        }
        ExternalDispositionOutcome::Stored
    } else if row.state == state && parse_safe_code(&row)? == code {
        ExternalDispositionOutcome::Replay {
            safe_error_code: code,
        }
    } else {
        return Err(AssetStoreError::Conflict);
    };
    transaction.commit().map_err(sqlite)?;
    Ok(outcome)
}

fn replay_materialization(
    transaction: &Transaction<'_>,
    row: &CommandRow,
    command: &MaterializationCommandBinding,
) -> Result<MaterializationResult, AssetStoreError> {
    if row.operation_id != ASSET_MATERIALIZE_V1.as_str()
        || row.state != "COMPLETED"
        || row.result_kind.as_deref() != Some("ASSET_REVISION")
        || row.result_id.as_deref() != Some(command.asset_revision_id().to_bytes().as_slice())
        || row.result_location_id.is_some()
    {
        return Err(AssetStoreError::StorageCorruption);
    }
    let tuple = transaction
        .query_row(
            "SELECT rm.blob_digest, b.byte_length FROM assets a JOIN asset_revisions ar ON ar.asset_id=a.asset_id JOIN representations rp ON rp.asset_revision_id=ar.asset_revision_id JOIN resources rs ON rs.representation_id=rp.representation_id JOIN resource_members rm ON rm.resource_id=rs.resource_id JOIN blobs b ON b.digest=rm.blob_digest WHERE a.asset_id=?1 AND ar.asset_revision_id=?2 AND rp.representation_id=?3 AND rs.resource_id=?4 AND rm.ordinal=?5 AND b.lifecycle='AVAILABLE'",
            params![command.asset_id().to_bytes().as_slice(), command.asset_revision_id().to_bytes().as_slice(), command.representation_id().to_bytes().as_slice(), command.resource_id().to_bytes().as_slice(), i64::from(command.member_ordinal())],
            |result| Ok((result.get::<_, Vec<u8>>(0)?, result.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(sqlite)?
        .ok_or(AssetStoreError::StorageCorruption)?;
    let digest = Sha256Digest::from_bytes(
        tuple
            .0
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
    );
    let length = u64::try_from(tuple.1).map_err(|_| AssetStoreError::StorageCorruption)?;
    MaterializationResult::__from_store(
        command.binding().command_id(),
        command.asset_revision_id(),
        command.representation_id(),
        command.resource_id(),
        command.member_ordinal(),
        digest,
        length,
    )
}

fn claim_external(
    connection: &mut Connection,
    context: StoreContext,
    request: ExternalIngestClaim,
) -> Result<ExternalClaimOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    let row = read_command(&transaction, request.binding())?;
    let outcome = match row {
        None => {
            insert_claim(
                &transaction,
                context,
                request.binding(),
                request.claimed_at(),
            )?;
            ExternalClaimOutcome::Claimed
        }
        Some(row) => external_existing(
            &transaction,
            context,
            request.binding(),
            row,
            request.claimed_at(),
        )?,
    };
    transaction.commit().map_err(sqlite)?;
    Ok(outcome)
}

fn external_existing(
    transaction: &Transaction<'_>,
    context: StoreContext,
    binding: &CommandBinding,
    row: CommandRow,
    at: Timestamp,
) -> Result<ExternalClaimOutcome, AssetStoreError> {
    if !binding_matches(&row, binding, context.metadata.owner_uid) {
        return Err(AssetStoreError::Conflict);
    }
    let row = validate_command_row(row)?;
    match row.state.as_str() {
        "CLAIMED" if row.runtime_id.as_slice() == context.runtime_id => {
            Ok(ExternalClaimOutcome::InProgress)
        }
        "CLAIMED" => {
            transaction.execute("UPDATE commands SET state='RECOVERY_REQUIRED', safe_error_code='STORAGE_CONFIGURATION_ERROR', updated_at_seconds=?2, updated_at_nanos=?3 WHERE command_id=?1 AND state='CLAIMED'",
                params![binding.command_id().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
            Ok(ExternalClaimOutcome::RecoveryRequired {
                safe_error_code: ErrorCode::StorageConfigurationError,
            })
        }
        "COMPLETED" => Ok(ExternalClaimOutcome::Replay(replay_external_result(
            transaction,
            &row,
        )?)),
        "TERMINAL_REJECTED" => Ok(ExternalClaimOutcome::TerminalRejected {
            safe_error_code: parse_safe_code(&row)?,
        }),
        "RECOVERY_REQUIRED" => Ok(ExternalClaimOutcome::RecoveryRequired {
            safe_error_code: parse_safe_code(&row)?,
        }),
        _ => Err(AssetStoreError::StorageCorruption),
    }
}

fn complete_external(
    connection: &mut Connection,
    context: StoreContext,
    request: ExternalIngestCompletion,
) -> Result<MutationOutcome, AssetStoreError> {
    complete_external_inner(connection, context, request, |_| Ok(()))
}

fn complete_external_inner(
    connection: &mut Connection,
    context: StoreContext,
    request: ExternalIngestCompletion,
    mut boundary: impl FnMut(u8) -> Result<(), AssetStoreError>,
) -> Result<MutationOutcome, AssetStoreError> {
    validate_descriptor(request.durable_blob().location().backend_id(), 255)?;
    validate_descriptor(request.durable_blob().location().locator(), 1024)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    let row = read_command(&transaction, request.binding())?.ok_or(AssetStoreError::Conflict)?;
    if !binding_matches(&row, request.binding(), context.metadata.owner_uid) {
        return Err(AssetStoreError::Conflict);
    }
    let row = validate_command_row(row)?;
    if row.state == "COMPLETED" {
        return Ok(MutationOutcome::Replay(replay_external_result(
            &transaction,
            &row,
        )?));
    }
    if row.state == "RECOVERY_REQUIRED" {
        return Ok(MutationOutcome::RecoveryRequired {
            safe_error_code: parse_safe_code(&row)?,
        });
    }
    if row.state != "CLAIMED" || row.runtime_id.as_slice() != context.runtime_id {
        return Err(AssetStoreError::Conflict);
    }
    boundary(1)?;

    let plan = request.plan();
    let _graph = AssetGraph::register_managed(RegisterManagedAssetValues {
        asset_id: plan.asset_id(),
        asset_kind: plan.asset_kind().clone(),
        asset_revision_id: plan.asset_revision_id(),
        content_kind: plan.content_kind().clone(),
        representation_id: plan.representation_id(),
        representation_purpose: plan.representation_purpose().clone(),
        resource_id: plan.resource_id(),
        resource_kind: plan.resource_kind().clone(),
        logical_name: plan.logical_name().clone(),
        media_type: plan.media_type().cloned(),
        blob_digest: request.durable_blob().digest(),
        created_at: request.completed_at(),
    })
    .map_err(|_| AssetStoreError::Validation)?;
    let location_id = persist_blob_and_location(
        &transaction,
        request.durable_blob(),
        plan.candidate_location_id(),
        plan.media_type(),
        request.completed_at(),
    )?;
    boundary(2)?;
    insert_registration_graph(&transaction, context, &request)?;
    boundary(3)?;
    let event_sequence = allocate_event_sequences(&transaction, 1)?;
    boundary(4)?;
    insert_registration_events(&transaction, &request, event_sequence)?;
    boundary(5)?;
    let changed = transaction.execute("UPDATE commands SET state='COMPLETED', result_kind='ASSET', result_id=?2, result_location_id=?3, safe_error_code=NULL, updated_at_seconds=?4, updated_at_nanos=?5 WHERE command_id=?1 AND state='CLAIMED'",
        params![request.binding().command_id().to_bytes().as_slice(), plan.asset_id().to_bytes().as_slice(), location_id.as_slice(), request.completed_at().unix_seconds(), i64::from(request.completed_at().subsec_nanoseconds())]).map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    boundary(6)?;
    let completed = validate_command_row(
        read_command(&transaction, request.binding())?.ok_or(AssetStoreError::StorageCorruption)?,
    )?;
    let result = replay_external_result(&transaction, &completed)?;
    boundary(7)?;
    transaction.commit().map_err(sqlite)?;
    boundary(8)?;
    Ok(MutationOutcome::Applied(result))
}

fn persist_blob_and_location(
    transaction: &Transaction<'_>,
    blob: &mengxia_ports::DurableBlob,
    candidate_location_id: Id<mengxia_domain::Location>,
    media_type: Option<&mengxia_domain::MediaType>,
    at: Timestamp,
) -> Result<[u8; 16], AssetStoreError> {
    let digest = blob.digest().to_bytes();
    let existing_blob = transaction
        .query_row(
            "SELECT byte_length, media_type, lifecycle, revision, verified_at_seconds, verified_at_nanos FROM blobs WHERE digest=?1",
            params![digest.as_slice()],
            |row| {
                Ok(BlobFactRow {
                    byte_length: row.get(0)?,
                    media_type: row.get(1)?,
                    lifecycle: row.get(2)?,
                    revision: row.get(3)?,
                    verified_at_seconds: row.get(4)?,
                    verified_at_nanos: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(sqlite)?;
    match existing_blob {
        Some(row) => {
            let parsed_media = row
                .media_type
                .as_deref()
                .map(mengxia_domain::MediaType::new)
                .transpose()
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            if parse_revision(&row.revision)?.get() == 0 {
                return Err(AssetStoreError::StorageCorruption);
            }
            let nanos = u32::try_from(row.verified_at_nanos)
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            Timestamp::from_unix_seconds_nanos(row.verified_at_seconds, nanos)
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            if u64::try_from(row.byte_length).ok() != Some(blob.byte_length())
                || parsed_media.as_ref().map(|value| value.as_str())
                    != media_type.map(|value| value.as_str())
                || row.lifecycle != "AVAILABLE"
            {
                return Err(AssetStoreError::StorageCorruption);
            }
        }
        None => {
            let length =
                i64::try_from(blob.byte_length()).map_err(|_| AssetStoreError::Validation)?;
            transaction.execute("INSERT INTO blobs (digest, byte_length, media_type, lifecycle, revision, verified_at_seconds, verified_at_nanos) VALUES (?1, ?2, ?3, 'AVAILABLE', ?4, ?5, ?6)", params![digest.as_slice(), length, media_type.map(|value| value.as_str()), revision_bytes(1).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
        }
    }
    let descriptor = blob.location();
    let existing_location = transaction.query_row("SELECT location_id, blob_digest, custody, durability, lifecycle, revision, verified_at_seconds, verified_at_nanos FROM locations WHERE backend_id=?1 AND locator=?2", params![descriptor.backend_id(), descriptor.locator()], |row| Ok(LocationFactRow { location_id: row.get(0)?, blob_digest: row.get(1)?, custody: row.get(2)?, durability: row.get(3)?, lifecycle: row.get(4)?, revision: row.get(5)?, verified_at_seconds: row.get(6)?, verified_at_nanos: row.get(7)? })).optional().map_err(sqlite)?;
    match existing_location {
        Some(row) => {
            Id::<mengxia_domain::Location>::from_bytes(
                row.location_id
                    .as_slice()
                    .try_into()
                    .map_err(|_| AssetStoreError::StorageCorruption)?,
            )
            .map_err(|_| AssetStoreError::StorageCorruption)?;
            if parse_revision(&row.revision)? != RevisionNo::new(1) {
                return Err(AssetStoreError::StorageCorruption);
            }
            let nanos = u32::try_from(row.verified_at_nanos)
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            Timestamp::from_unix_seconds_nanos(row.verified_at_seconds, nanos)
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            if row.blob_digest.as_slice() != digest
                || row.custody != "MANAGED"
                || row.durability != "DURABLE"
                || row.lifecycle != "AVAILABLE"
            {
                return Err(AssetStoreError::StorageCorruption);
            }
            row.location_id
                .try_into()
                .map_err(|_| AssetStoreError::StorageCorruption)
        }
        None => {
            let id = candidate_location_id.to_bytes();
            transaction.execute("INSERT INTO locations (location_id, blob_digest, backend_id, locator, custody, durability, lifecycle, revision, verified_at_seconds, verified_at_nanos) VALUES (?1, ?2, ?3, ?4, 'MANAGED', 'DURABLE', 'AVAILABLE', ?5, ?6, ?7)", params![id.as_slice(), digest.as_slice(), descriptor.backend_id(), descriptor.locator(), revision_bytes(1).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
            Ok(id)
        }
    }
}

fn insert_registration_graph(
    transaction: &Transaction<'_>,
    context: StoreContext,
    request: &ExternalIngestCompletion,
) -> Result<(), AssetStoreError> {
    let plan = request.plan();
    let at = request.completed_at();
    let uid = i64::from(context.metadata.owner_uid);
    transaction.execute("INSERT INTO assets (asset_id, kind, lifecycle, revision, created_at_seconds, created_at_nanos, created_by_uid) VALUES (?1, ?2, 'ACTIVE', ?3, ?4, ?5, ?6)", params![plan.asset_id().to_bytes().as_slice(), plan.asset_kind().as_str(), revision_bytes(1).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds()), uid]).map_err(sqlite)?;
    transaction.execute("INSERT INTO asset_revisions (asset_revision_id, asset_id, sequence, content_kind, custody, created_at_seconds, created_at_nanos, created_by_uid) VALUES (?1, ?2, 1, ?3, 'MANAGED', ?4, ?5, ?6)", params![plan.asset_revision_id().to_bytes().as_slice(), plan.asset_id().to_bytes().as_slice(), plan.content_kind().as_str(), at.unix_seconds(), i64::from(at.subsec_nanoseconds()), uid]).map_err(sqlite)?;
    transaction.execute("INSERT INTO representations (representation_id, asset_revision_id, purpose) VALUES (?1, ?2, ?3)", params![plan.representation_id().to_bytes().as_slice(), plan.asset_revision_id().to_bytes().as_slice(), plan.representation_purpose().as_str()]).map_err(sqlite)?;
    let changed = transaction
        .execute(
            "INSERT INTO resources (resource_id, representation_id, kind) VALUES (?1, ?2, ?3)",
            params![
                plan.resource_id().to_bytes().as_slice(),
                plan.representation_id().to_bytes().as_slice(),
                plan.resource_kind().as_str()
            ],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    transaction.execute("INSERT INTO resource_members (resource_id, ordinal, logical_name, blob_digest) VALUES (?1, 0, ?2, ?3)", params![plan.resource_id().to_bytes().as_slice(), plan.logical_name().as_str(), request.durable_blob().digest().to_bytes().as_slice()]).map_err(sqlite)?;
    Ok(())
}

fn insert_registration_events(
    transaction: &Transaction<'_>,
    request: &ExternalIngestCompletion,
    sequence: i64,
) -> Result<(), AssetStoreError> {
    let at = request.completed_at();
    let plan = request.plan();
    let command = request.binding().command_id().to_bytes();
    transaction.execute("INSERT INTO domain_events (domain_event_id, commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, occurred_at_seconds, occurred_at_nanos) VALUES (?1, ?2, ?3, 'asset.registered.v1', 1, 'ASSET', ?4, ?5, ?6, ?7)", params![request.domain_event_id().to_bytes().as_slice(), sequence, command.as_slice(), plan.asset_id().to_bytes().as_slice(), revision_bytes(1).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
    transaction.execute("INSERT INTO provenance_events (provenance_event_id, command_id, event_type, schema_version, asset_revision_id, blob_digest, verification, occurred_at_seconds, occurred_at_nanos, recorded_at_seconds, recorded_at_nanos, correction_of) VALUES (?1, ?2, 'asset.ingested.copy.v1', 1, ?3, ?4, 'VERIFIED', ?5, ?6, ?5, ?6, NULL)", params![request.provenance_event_id().to_bytes().as_slice(), command.as_slice(), plan.asset_revision_id().to_bytes().as_slice(), request.durable_blob().digest().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
    Ok(())
}

fn finish_external(
    connection: &mut Connection,
    context: StoreContext,
    request: ExternalIngestDisposition,
) -> Result<ExternalDispositionOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    let row = read_command(&transaction, request.binding())?.ok_or(AssetStoreError::Conflict)?;
    if !binding_matches(&row, request.binding(), context.metadata.owner_uid) {
        return Err(AssetStoreError::Conflict);
    }
    let row = validate_command_row(row)?;
    let (state, code) = match request.disposition() {
        ExternalDisposition::TerminalRejected(code) => ("TERMINAL_REJECTED", code),
        ExternalDisposition::RecoveryRequired(code) => ("RECOVERY_REQUIRED", code),
    };
    let outcome = if row.state == "CLAIMED" && row.runtime_id.as_slice() == context.runtime_id {
        transaction.execute("UPDATE commands SET state=?2, safe_error_code=?3, updated_at_seconds=?4, updated_at_nanos=?5 WHERE command_id=?1 AND state='CLAIMED'", params![request.binding().command_id().to_bytes().as_slice(), state, code.as_str(), request.observed_at().unix_seconds(), i64::from(request.observed_at().subsec_nanoseconds())]).map_err(sqlite)?;
        ExternalDispositionOutcome::Stored
    } else if row.state == state && parse_safe_code(&row)? == code {
        ExternalDispositionOutcome::Replay {
            safe_error_code: code,
        }
    } else {
        return Err(AssetStoreError::Conflict);
    };
    transaction.commit().map_err(sqlite)?;
    Ok(outcome)
}

fn create_revision(
    connection: &mut Connection,
    context: StoreContext,
    request: CreateAssetRevisionCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    create_revision_inner(connection, context, request, |_| Ok(()))
}

fn generated_value_id<T>(
    source: &dyn mengxia_ports::PureCommandValueSource,
) -> Result<Id<T>, AssetStoreError> {
    Id::from_bytes(source.next_uuid_v7()?).map_err(|_| AssetStoreError::IdGenerationUnavailable)
}

fn create_revision_deferred(
    connection: &mut Connection,
    context: StoreContext,
    request: DeferredCreateAssetRevisionCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    // The writer is the only mutation executor. Holding IMMEDIATE here makes this
    // absent-command decision the linearization point before any ID/clock sample.
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(row) = read_command(&transaction, request.binding())? {
        if !binding_matches(&row, request.binding(), context.metadata.owner_uid) {
            return Err(AssetStoreError::Conflict);
        }
        return replay_pure(&transaction, validate_command_row(row)?, "ASSET_REVISION");
    }

    let asset = transaction
        .query_row(
            "SELECT kind, lifecycle, revision, created_at_seconds, created_at_nanos FROM assets WHERE asset_id=?1",
            [request.asset_id().to_bytes().as_slice()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(sqlite)?;

    let revision_id = generated_value_id::<mengxia_domain::AssetRevision>(request.values())?;
    let event_id = generated_value_id::<mengxia_events::DomainEvent>(request.values())?;
    let provenance_id = generated_value_id::<mengxia_events::ProvenanceEvent>(request.values())?;
    let mut generated = vec![
        request.asset_id().to_bytes(),
        revision_id.to_bytes(),
        event_id.to_bytes(),
        provenance_id.to_bytes(),
    ];
    let mut representations = Vec::with_capacity(request.representations().len());
    for representation in request.representations() {
        let representation_id =
            generated_value_id::<mengxia_domain::Representation>(request.values())?;
        generated.push(representation_id.to_bytes());
        let mut resources = Vec::with_capacity(representation.resources().len());
        for resource in representation.resources() {
            let resource_id = generated_value_id::<mengxia_domain::Resource>(request.values())?;
            generated.push(resource_id.to_bytes());
            let members = resource
                .members()
                .iter()
                .map(|member| {
                    RevisionMember::new(member.logical_name().clone(), member.blob_digest())
                })
                .collect();
            resources.push(
                RevisionResource::new(resource_id, resource.kind().clone(), members)
                    .map_err(|_| AssetStoreError::Validation)?,
            );
        }
        representations.push(
            RevisionRepresentation::new(
                representation_id,
                representation.purpose().clone(),
                resources,
            )
            .map_err(|_| AssetStoreError::Validation)?,
        );
    }
    generated.extend(request.parent_revision_ids().iter().map(|id| id.to_bytes()));
    if generated
        .iter()
        .enumerate()
        .any(|(index, value)| generated[index + 1..].contains(value))
    {
        return Err(AssetStoreError::IdGenerationUnavailable);
    }
    let at = request.values().now()?;
    request.values().checkpoint()?;

    let Some((kind, lifecycle, current_revision, created_seconds, created_nanos)) = asset else {
        insert_claim(&transaction, context, request.binding(), at)?;
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    };
    let kind =
        mengxia_domain::AssetKind::new(kind).map_err(|_| AssetStoreError::StorageCorruption)?;
    let lifecycle = match lifecycle.as_str() {
        "ACTIVE" => mengxia_domain::AssetLifecycle::Active,
        "RETIRED" => mengxia_domain::AssetLifecycle::Retired,
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    let current_revision = parse_revision(&current_revision)?;
    let created_at = Timestamp::from_unix_seconds_nanos(
        created_seconds,
        u32::try_from(created_nanos).map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)?;
    let record = AssetRecord::__from_store(
        request.asset_id(),
        kind,
        lifecycle,
        current_revision,
        created_at,
    );
    let revision = match record.create_revision(CreateAssetRevisionValues {
        expected_revision: request.expected_revision(),
        revision_id,
        parent_revision_ids: request.parent_revision_ids().to_vec(),
        content_kind: request.content_kind().clone(),
        representations,
        created_at: at,
    }) {
        Ok(revision) => revision,
        Err(mengxia_domain::AssetError::Conflict) => {
            insert_claim(&transaction, context, request.binding(), at)?;
            return commit_rejection(transaction, request.binding(), ErrorCode::Conflict, at);
        }
        Err(mengxia_domain::AssetError::RevisionExhausted) => {
            insert_claim(&transaction, context, request.binding(), at)?;
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(_) => return Err(AssetStoreError::Validation),
    };
    drop(transaction);
    create_revision(
        connection,
        context,
        CreateAssetRevisionCommand::new(*request.binding(), revision, event_id, provenance_id, at)?,
    )
}

fn create_revision_inner(
    connection: &mut Connection,
    context: StoreContext,
    request: CreateAssetRevisionCommand,
    mut boundary: impl FnMut(u8) -> Result<(), AssetStoreError>,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(row) = read_command(&transaction, request.binding())? {
        if !binding_matches(&row, request.binding(), context.metadata.owner_uid) {
            return Err(AssetStoreError::Conflict);
        }
        let row = validate_command_row(row)?;
        return replay_pure(&transaction, row, "ASSET_REVISION");
    }
    insert_claim(
        &transaction,
        context,
        request.binding(),
        request.operation_at(),
    )?;
    boundary(1)?;
    let revision = request.revision();
    let asset_id = revision.asset_id().to_bytes();
    let current: Option<(Vec<u8>, Option<i64>)> = transaction
        .query_row(
            "SELECT revision, (SELECT max(sequence) FROM asset_revisions WHERE asset_id=assets.asset_id) FROM assets WHERE asset_id=?1 AND lifecycle='ACTIVE'",
            params![asset_id.as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite)?;
    let Some((current, maximum_sequence)) = current else {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::NotFound,
            request.operation_at(),
        );
    };
    let expected = revision
        .resulting_revision()
        .get()
        .checked_sub(1)
        .ok_or(AssetStoreError::Validation)?;
    let current_revision = parse_revision(&current)?;
    let maximum_sequence = maximum_sequence
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(AssetStoreError::StorageCorruption)?;
    if maximum_sequence != current_revision.get() {
        return Err(AssetStoreError::StorageCorruption);
    }
    if current_revision.get() != expected {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::Conflict,
            request.operation_at(),
        );
    }
    if expected >= u64::from(u32::MAX) {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::RevisionExhausted,
            request.operation_at(),
        );
    }
    for parent in revision.parent_revision_ids() {
        let belongs_to_asset = transaction
            .query_row(
                "SELECT count(*) FROM asset_revisions WHERE asset_id=?1 AND asset_revision_id=?2",
                params![asset_id.as_slice(), parent.to_bytes().as_slice()],
                |row| row.get::<_, i64>(0),
            )
            .map_err(sqlite)?;
        if belongs_to_asset != 1 {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::Conflict,
                request.operation_at(),
            );
        }
    }
    for representation in revision.representations() {
        for resource in representation.resources() {
            for member in resource.members() {
                let (blob_count, has_managed_custody) = transaction.query_row("SELECT (SELECT count(*) FROM blobs WHERE digest=?1), (SELECT count(*) FROM blobs b WHERE b.digest=?1 AND b.lifecycle='AVAILABLE' AND EXISTS (SELECT 1 FROM locations l WHERE l.blob_digest=b.digest AND l.custody='MANAGED' AND l.durability='DURABLE' AND l.lifecycle='AVAILABLE'))", params![member.blob_digest().to_bytes().as_slice()], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))).map_err(sqlite)?;
                if blob_count == 0 {
                    return commit_rejection(
                        transaction,
                        request.binding(),
                        ErrorCode::NotFound,
                        request.operation_at(),
                    );
                }
                if blob_count != 1 || has_managed_custody != 1 {
                    return Err(AssetStoreError::StorageCorruption);
                }
            }
        }
    }
    let sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                request.operation_at(),
            );
        }
        Err(error) => return Err(error),
    };
    boundary(2)?;
    let changed = transaction
        .execute(
            "UPDATE assets SET revision=?2, updated_at_seconds=?4, updated_at_nanos=?5 WHERE asset_id=?1 AND revision=?3",
            params![
                asset_id.as_slice(),
                revision_bytes(revision.resulting_revision().get()).as_slice(),
                revision_bytes(expected).as_slice(),
                request.operation_at().unix_seconds(),
                i64::from(request.operation_at().subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    boundary(3)?;
    let uid = i64::from(context.metadata.owner_uid);
    let at = request.operation_at();
    transaction.execute("INSERT INTO asset_revisions (asset_revision_id, asset_id, sequence, content_kind, custody, created_at_seconds, created_at_nanos, created_by_uid) VALUES (?1, ?2, (SELECT max(sequence)+1 FROM asset_revisions WHERE asset_id=?2), ?3, 'MANAGED', ?4, ?5, ?6)", params![revision.revision_id().to_bytes().as_slice(), asset_id.as_slice(), revision.content_kind().as_str(), at.unix_seconds(), i64::from(at.subsec_nanoseconds()), uid]).map_err(sqlite)?;
    boundary(4)?;
    for (ordinal, parent) in revision.parent_revision_ids().iter().enumerate() {
        transaction.execute("INSERT INTO asset_revision_parents (asset_id, child_revision_id, ordinal, parent_revision_id) VALUES (?1, ?2, ?3, ?4)", params![asset_id.as_slice(), revision.revision_id().to_bytes().as_slice(), i64::try_from(ordinal).map_err(|_| AssetStoreError::Validation)?, parent.to_bytes().as_slice()]).map_err(sqlite)?;
    }
    boundary(5)?;
    for representation in revision.representations() {
        transaction.execute("INSERT INTO representations (representation_id, asset_revision_id, purpose) VALUES (?1, ?2, ?3)", params![representation.id().to_bytes().as_slice(), revision.revision_id().to_bytes().as_slice(), representation.purpose().as_str()]).map_err(sqlite)?;
        for resource in representation.resources() {
            transaction.execute("INSERT INTO resources (resource_id, representation_id, kind) VALUES (?1, ?2, ?3)", params![resource.id().to_bytes().as_slice(), representation.id().to_bytes().as_slice(), resource.kind().as_str()]).map_err(sqlite)?;
            for (ordinal, member) in resource.members().iter().enumerate() {
                transaction.execute("INSERT INTO resource_members (resource_id, ordinal, logical_name, blob_digest) VALUES (?1, ?2, ?3, ?4)", params![resource.id().to_bytes().as_slice(), i64::try_from(ordinal).map_err(|_| AssetStoreError::Validation)?, member.logical_name().as_str(), member.blob_digest().to_bytes().as_slice()]).map_err(sqlite)?;
            }
        }
    }
    boundary(6)?;
    transaction.execute("INSERT INTO domain_events (domain_event_id, commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, occurred_at_seconds, occurred_at_nanos) VALUES (?1, ?2, ?3, 'asset.revision.created.v1', 1, 'ASSET_REVISION', ?4, ?5, ?6, ?7)", params![request.domain_event_id().to_bytes().as_slice(), sequence, request.binding().command_id().to_bytes().as_slice(), revision.revision_id().to_bytes().as_slice(), revision_bytes(revision.resulting_revision().get()).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
    boundary(7)?;
    transaction.execute("INSERT INTO provenance_events (provenance_event_id, command_id, event_type, schema_version, asset_revision_id, blob_digest, verification, occurred_at_seconds, occurred_at_nanos, recorded_at_seconds, recorded_at_nanos, correction_of) VALUES (?1, ?2, 'asset.revision.derived.v1', 1, ?3, NULL, 'VERIFIED', ?4, ?5, ?4, ?5, NULL)", params![request.provenance_event_id().to_bytes().as_slice(), request.binding().command_id().to_bytes().as_slice(), revision.revision_id().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
    boundary(8)?;
    let changed = transaction.execute("UPDATE commands SET state='COMPLETED', result_kind='ASSET_REVISION', result_id=?2, updated_at_seconds=?3, updated_at_nanos=?4 WHERE command_id=?1 AND state='CLAIMED'", params![request.binding().command_id().to_bytes().as_slice(), revision.revision_id().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    boundary(9)?;
    let completed = validate_command_row(
        read_command(&transaction, request.binding())?.ok_or(AssetStoreError::StorageCorruption)?,
    )?;
    let result = replay_result(&transaction, &completed)?;
    if !matches!(result, CommandResult::AssetRevision(_)) {
        return Err(AssetStoreError::StorageCorruption);
    }
    boundary(10)?;
    transaction.commit().map_err(sqlite)?;
    boundary(11)?;
    Ok(MutationOutcome::Applied(result))
}

fn change_asset_lifecycle(
    connection: &mut Connection,
    context: StoreContext,
    request: AssetLifecycleCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(row) = read_command(&transaction, request.binding())? {
        if !binding_matches(&row, request.binding(), context.metadata.owner_uid) {
            return Err(AssetStoreError::Conflict);
        }
        let row = validate_command_row(row)?;
        return replay_pure(&transaction, row, "ASSET_LIFECYCLE");
    }
    insert_claim(
        &transaction,
        context,
        request.binding(),
        request.operation_at(),
    )?;
    let current: Option<(String, Vec<u8>)> = transaction
        .query_row(
            "SELECT lifecycle, revision FROM assets WHERE asset_id=?1",
            params![request.asset_id().to_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(sqlite)?;
    let Some((current_lifecycle, current_revision)) = current else {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::NotFound,
            request.operation_at(),
        );
    };
    let current_revision = parse_revision(&current_revision)?;
    if current_revision != request.expected_revision() {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::Conflict,
            request.operation_at(),
        );
    }
    let valid_transition = matches!(
        (current_lifecycle.as_str(), request.target()),
        ("ACTIVE", mengxia_domain::AssetLifecycle::Retired)
            | ("RETIRED", mengxia_domain::AssetLifecycle::Active)
    );
    if !valid_transition {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::InvalidTransition,
            request.operation_at(),
        );
    }
    let Some(next) = current_revision.get().checked_add(1) else {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::RevisionExhausted,
            request.operation_at(),
        );
    };
    let sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                request.operation_at(),
            );
        }
        Err(error) => return Err(error),
    };
    let lifecycle = match request.target() {
        mengxia_domain::AssetLifecycle::Active => "ACTIVE",
        mengxia_domain::AssetLifecycle::Retired => "RETIRED",
    };
    let at = request.operation_at();
    let changed = transaction
        .execute(
            "UPDATE assets SET lifecycle=?2, revision=?3, updated_at_seconds=?4, updated_at_nanos=?5 WHERE asset_id=?1 AND revision=?6",
            params![
                request.asset_id().to_bytes().as_slice(), lifecycle,
                revision_bytes(next).as_slice(), at.unix_seconds(),
                i64::from(at.subsec_nanoseconds()),
                revision_bytes(current_revision.get()).as_slice(),
            ],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    transaction
        .execute(
            "INSERT INTO domain_events (domain_event_id, commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, event_payload, event_payload_sha256, occurred_at_seconds, occurred_at_nanos) VALUES (?1, ?2, ?3, ?4, 1, 'ASSET', ?5, ?6, NULL, NULL, ?7, ?8)",
            params![
                request.domain_event_id().to_bytes().as_slice(), sequence,
                request.binding().command_id().to_bytes().as_slice(),
                if request.target() == mengxia_domain::AssetLifecycle::Retired {
                    "asset.retired.v1"
                } else {
                    "asset.restored.v1"
                },
                request.asset_id().to_bytes().as_slice(), revision_bytes(next).as_slice(),
                at.unix_seconds(), i64::from(at.subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    let payload = VersionedResultPayload::AssetLifecycle {
        revision: next,
        lifecycle: request.target(),
    };
    let payload_bytes = payload.encode();
    let payload_hash: [u8; 32] = Sha256::digest(&payload_bytes).into();
    let changed = transaction
        .execute(
            "UPDATE commands SET state='COMPLETED', result_kind='ASSET_LIFECYCLE', result_id=?2, result_schema_version=1, result_payload=?3, result_payload_sha256=?4, updated_at_seconds=?5, updated_at_nanos=?6 WHERE command_id=?1 AND state='CLAIMED'",
            params![
                request.binding().command_id().to_bytes().as_slice(),
                request.asset_id().to_bytes().as_slice(), payload_bytes, payload_hash.as_slice(),
                at.unix_seconds(), i64::from(at.subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    let completed = validate_command_row(
        read_command(&transaction, request.binding())?.ok_or(AssetStoreError::StorageCorruption)?,
    )?;
    let result = replay_result(&transaction, &completed)?;
    transaction.commit().map_err(sqlite)?;
    Ok(MutationOutcome::Applied(result))
}

fn change_asset_lifecycle_deferred(
    connection: &mut Connection,
    context: StoreContext,
    request: DeferredAssetLifecycleCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(row) = read_command(&transaction, request.binding())? {
        if !binding_matches(&row, request.binding(), context.metadata.owner_uid) {
            return Err(AssetStoreError::Conflict);
        }
        return replay_pure(&transaction, validate_command_row(row)?, "ASSET_LIFECYCLE");
    }
    let event_id = generated_value_id::<mengxia_events::DomainEvent>(request.values())?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    drop(transaction);
    change_asset_lifecycle(
        connection,
        context,
        AssetLifecycleCommand::new(
            *request.binding(),
            request.asset_id(),
            request.expected_revision(),
            request.target(),
            event_id,
            at,
        )?,
    )
}

fn record_location(
    connection: &mut Connection,
    context: StoreContext,
    request: RecordManagedLocationCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    record_location_inner(connection, context, request, |_| Ok(()))
}

fn record_location_inner(
    connection: &mut Connection,
    context: StoreContext,
    request: RecordManagedLocationCommand,
    mut boundary: impl FnMut(u8) -> Result<(), AssetStoreError>,
) -> Result<MutationOutcome, AssetStoreError> {
    validate_descriptor(request.durable_blob().location().backend_id(), 255)?;
    validate_descriptor(request.durable_blob().location().locator(), 1024)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(row) = read_command(&transaction, request.binding())? {
        if !binding_matches(&row, request.binding(), context.metadata.owner_uid) {
            return Err(AssetStoreError::Conflict);
        }
        let row = validate_command_row(row)?;
        return replay_pure(&transaction, row, "LOCATION");
    }
    insert_claim(
        &transaction,
        context,
        request.binding(),
        request.operation_at(),
    )?;
    boundary(1)?;
    let digest = request.durable_blob().digest().to_bytes();
    let current: Option<BlobFactRow> = transaction
        .query_row(
            "SELECT revision, byte_length, media_type, lifecycle, verified_at_seconds, verified_at_nanos FROM blobs WHERE digest=?1",
            params![digest.as_slice()],
            |row| Ok(BlobFactRow { revision: row.get(0)?, byte_length: row.get(1)?, media_type: row.get(2)?, lifecycle: row.get(3)?, verified_at_seconds: row.get(4)?, verified_at_nanos: row.get(5)? }),
        )
        .optional()
        .map_err(sqlite)?;
    let Some(current) = current else {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::NotFound,
            request.operation_at(),
        );
    };
    if parse_revision(&current.revision)?.get() == 0 {
        return Err(AssetStoreError::StorageCorruption);
    }
    current
        .media_type
        .as_deref()
        .map(mengxia_domain::MediaType::new)
        .transpose()
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    let nanos =
        u32::try_from(current.verified_at_nanos).map_err(|_| AssetStoreError::StorageCorruption)?;
    Timestamp::from_unix_seconds_nanos(current.verified_at_seconds, nanos)
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    if current.lifecycle != "AVAILABLE" {
        return Err(AssetStoreError::StorageCorruption);
    }
    if u64::try_from(current.byte_length).ok() != Some(request.durable_blob().byte_length()) {
        return Err(AssetStoreError::StorageCorruption);
    }
    boundary(2)?;
    if current.revision.as_slice() != revision_bytes(request.expected_revision().get()) {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::Conflict,
            request.operation_at(),
        );
    }
    let next = match request.expected_revision().checked_next() {
        Ok(next) => next,
        Err(_) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                request.operation_at(),
            );
        }
    };
    let existing = transaction
        .query_row(
            "SELECT location_id, blob_digest, custody, durability, lifecycle, revision, verified_at_seconds, verified_at_nanos FROM locations WHERE backend_id=?1 AND locator=?2",
            params![
                request.durable_blob().location().backend_id(),
                request.durable_blob().location().locator()
            ],
            |row| Ok(LocationFactRow { location_id: row.get(0)?, blob_digest: row.get(1)?, custody: row.get(2)?, durability: row.get(3)?, lifecycle: row.get(4)?, revision: row.get(5)?, verified_at_seconds: row.get(6)?, verified_at_nanos: row.get(7)? }),
        )
        .optional()
        .map_err(sqlite)?;
    if let Some(row) = existing {
        let id: [u8; 16] = row
            .location_id
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        Id::<mengxia_domain::Location>::from_bytes(id)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        if parse_revision(&row.revision)? != RevisionNo::new(1) {
            return Err(AssetStoreError::StorageCorruption);
        }
        persisted_timestamp(row.verified_at_seconds, row.verified_at_nanos)?;
        if row.blob_digest.as_slice() != digest
            || row.custody != "MANAGED"
            || row.durability != "DURABLE"
            || row.lifecycle != "AVAILABLE"
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::Conflict,
            request.operation_at(),
        );
    }
    let sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                request.operation_at(),
            );
        }
        Err(error) => return Err(error),
    };
    boundary(3)?;
    let location_id = request.candidate_location_id().to_bytes();
    let at = request.operation_at();
    transaction.execute("INSERT INTO locations (location_id, blob_digest, backend_id, locator, custody, durability, lifecycle, revision, verified_at_seconds, verified_at_nanos) VALUES (?1, ?2, ?3, ?4, 'MANAGED', 'DURABLE', 'AVAILABLE', ?5, ?6, ?7)", params![location_id.as_slice(), digest.as_slice(), request.durable_blob().location().backend_id(), request.durable_blob().location().locator(), revision_bytes(1).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
    boundary(4)?;
    transaction.execute("UPDATE blobs SET revision=?2, verified_at_seconds=?3, verified_at_nanos=?4 WHERE digest=?1", params![digest.as_slice(), revision_bytes(next.get()).as_slice(), request.operation_at().unix_seconds(), i64::from(request.operation_at().subsec_nanoseconds())]).map_err(sqlite)?;
    boundary(5)?;
    transaction.execute("INSERT INTO domain_events (domain_event_id, commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, occurred_at_seconds, occurred_at_nanos) VALUES (?1, ?2, ?3, 'blob.location.recorded.v1', 1, 'BLOB', ?4, ?5, ?6, ?7)", params![request.domain_event_id().to_bytes().as_slice(), sequence, request.binding().command_id().to_bytes().as_slice(), digest.as_slice(), revision_bytes(next.get()).as_slice(), request.operation_at().unix_seconds(), i64::from(request.operation_at().subsec_nanoseconds())]).map_err(sqlite)?;
    boundary(6)?;
    let changed = transaction.execute("UPDATE commands SET state='COMPLETED', result_kind='LOCATION', result_id=?2, updated_at_seconds=?3, updated_at_nanos=?4 WHERE command_id=?1 AND state='CLAIMED'", params![request.binding().command_id().to_bytes().as_slice(), location_id.as_slice(), request.operation_at().unix_seconds(), i64::from(request.operation_at().subsec_nanoseconds())]).map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    boundary(7)?;
    let completed = validate_command_row(
        read_command(&transaction, request.binding())?.ok_or(AssetStoreError::StorageCorruption)?,
    )?;
    let result = replay_result(&transaction, &completed)?;
    if !matches!(result, CommandResult::Location(_)) {
        return Err(AssetStoreError::StorageCorruption);
    }
    boundary(8)?;
    transaction.commit().map_err(sqlite)?;
    boundary(9)?;
    Ok(MutationOutcome::Applied(result))
}

pub(crate) fn replay_pure(
    transaction: &Transaction<'_>,
    row: CommandRow,
    expected_kind: &str,
) -> Result<MutationOutcome, AssetStoreError> {
    match row.state.as_str() {
        "COMPLETED" if row.result_kind.as_deref() == Some(expected_kind) => {
            Ok(MutationOutcome::Replay(replay_result(transaction, &row)?))
        }
        "TERMINAL_REJECTED" => Ok(MutationOutcome::TerminalRejected {
            safe_error_code: parse_safe_code(&row)?,
        }),
        "CLAIMED" | "RECOVERY_REQUIRED" => Err(AssetStoreError::StorageCorruption),
        _ => Err(AssetStoreError::StorageCorruption),
    }
}

pub(crate) fn commit_rejection(
    transaction: Transaction<'_>,
    binding: &CommandBinding,
    code: ErrorCode,
    at: Timestamp,
) -> Result<MutationOutcome, AssetStoreError> {
    transaction.execute("UPDATE commands SET state='TERMINAL_REJECTED', safe_error_code=?2, updated_at_seconds=?3, updated_at_nanos=?4 WHERE command_id=?1", params![binding.command_id().to_bytes().as_slice(), code.as_str(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())]).map_err(sqlite)?;
    transaction.commit().map_err(sqlite)?;
    Ok(MutationOutcome::TerminalRejected {
        safe_error_code: code,
    })
}

pub(crate) fn allocate_event_sequences(
    transaction: &Transaction<'_>,
    count: i64,
) -> Result<i64, AssetStoreError> {
    transaction.query_row("UPDATE event_commit_sequence SET last_sequence=last_sequence+?1 WHERE singleton=1 AND ?1 BETWEEN 1 AND 64 AND last_sequence <= 9223372036854775807-?1 RETURNING last_sequence-?1+1", params![count], |row| row.get(0)).optional().map_err(sqlite)?.ok_or(AssetStoreError::RevisionExhausted)
}

pub(crate) fn replay_result(
    transaction: &Transaction<'_>,
    row: &CommandRow,
) -> Result<CommandResult, AssetStoreError> {
    if matches!(
        row.operation_id.as_str(),
        "project.create.v1"
            | "project.spec.revise.v1"
            | "subject.create.v1"
            | "work.create.v1"
            | "work.revise.v1"
            | "take.create.v1"
            | "take.transition.v1"
            | "take.reopen.v1"
    ) {
        return super::creative_repository::replay_creative_result(transaction, row);
    }
    let result_id = id_bytes(row.result_id.as_deref())?;
    if row.operation_id == ASSET_RETIRE_V1.as_str() || row.operation_id == ASSET_RESTORE_V1.as_str()
    {
        let payload = VersionedResultPayload::decode(
            row.result_kind
                .as_deref()
                .ok_or(AssetStoreError::StorageCorruption)?,
            row.result_payload
                .as_deref()
                .ok_or(AssetStoreError::StorageCorruption)?,
        )?;
        let VersionedResultPayload::AssetLifecycle {
            revision,
            lifecycle: _,
        } = payload
        else {
            return Err(AssetStoreError::StorageCorruption);
        };
        let (stored_revision, event_count): (Vec<u8>, i64) = transaction
            .query_row(
                "SELECT a.revision, (SELECT count(*) FROM domain_events de WHERE de.command_id=?2 AND de.event_type=?3 AND de.aggregate_kind='ASSET' AND de.aggregate_id=a.asset_id AND de.aggregate_revision=?4 AND de.event_payload IS NULL AND de.event_payload_sha256 IS NULL) FROM assets a WHERE a.asset_id=?1",
                params![
                    result_id.as_slice(),
                    row.command_id.as_slice(),
                    if row.operation_id == ASSET_RETIRE_V1.as_str() {
                        "asset.retired.v1"
                    } else {
                        "asset.restored.v1"
                    },
                    revision_bytes(revision).as_slice(),
                ],
                |result| Ok((result.get(0)?, result.get(1)?)),
            )
            .optional()
            .map_err(sqlite)?
            .ok_or(AssetStoreError::StorageCorruption)?;
        if parse_revision(&stored_revision)?.get() < revision || event_count != 1 {
            return Err(AssetStoreError::StorageCorruption);
        }
        return Ok(CommandResult::Versioned(VersionedCommandResult::new(
            result_id,
            payload,
            Timestamp::from_unix_seconds_nanos(
                row.updated_at_seconds,
                u32::try_from(row.updated_at_nanos)
                    .map_err(|_| AssetStoreError::StorageCorruption)?,
            )
            .map_err(|_| AssetStoreError::StorageCorruption)?,
        )));
    }
    match row.result_kind.as_deref() {
        Some("ASSET") => {
            let location = id_bytes(row.result_location_id.as_deref())?;
            let tuple=transaction.query_row("SELECT ar.asset_revision_id, r.representation_id, rs.resource_id, rm.blob_digest FROM assets a JOIN asset_revisions ar ON ar.asset_id=a.asset_id AND ar.sequence=1 JOIN representations r ON r.asset_revision_id=ar.asset_revision_id JOIN resources rs ON rs.representation_id=r.representation_id JOIN resource_members rm ON rm.resource_id=rs.resource_id AND rm.ordinal=0 JOIN locations l ON l.location_id=?2 AND l.blob_digest=rm.blob_digest AND l.custody='MANAGED' AND l.durability='DURABLE' AND l.lifecycle='AVAILABLE' AND l.revision=?4 WHERE a.asset_id=?1 AND (SELECT count(*) FROM representations WHERE asset_revision_id=ar.asset_revision_id)=1 AND (SELECT count(*) FROM resources WHERE representation_id=r.representation_id)=1 AND (SELECT count(*) FROM resource_members WHERE resource_id=rs.resource_id)=1 AND (SELECT count(*) FROM domain_events WHERE command_id=?3 AND event_type='asset.registered.v1' AND aggregate_kind='ASSET' AND aggregate_id=a.asset_id)=1 AND (SELECT count(*) FROM provenance_events WHERE command_id=?3 AND event_type='asset.ingested.copy.v1' AND asset_revision_id=ar.asset_revision_id AND blob_digest=rm.blob_digest)=1", params![result_id.as_slice(), location.as_slice(), row.command_id.as_slice(), revision_bytes(1).as_slice()], |r| Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,Vec<u8>>(1)?,r.get::<_,Vec<u8>>(2)?,r.get::<_,Vec<u8>>(3)?))).optional().map_err(sqlite)?.ok_or(AssetStoreError::StorageCorruption)?;
            Ok(CommandResult::ManagedRegistration(
                ManagedRegistrationResult::new(
                    Id::from_bytes(result_id).map_err(|_| AssetStoreError::StorageCorruption)?,
                    Id::from_bytes(id_bytes(Some(&tuple.0))?)
                        .map_err(|_| AssetStoreError::StorageCorruption)?,
                    Id::from_bytes(id_bytes(Some(&tuple.1))?)
                        .map_err(|_| AssetStoreError::StorageCorruption)?,
                    Id::from_bytes(id_bytes(Some(&tuple.2))?)
                        .map_err(|_| AssetStoreError::StorageCorruption)?,
                    Id::from_bytes(location).map_err(|_| AssetStoreError::StorageCorruption)?,
                    Sha256Digest::from_bytes(
                        tuple
                            .3
                            .try_into()
                            .map_err(|_| AssetStoreError::StorageCorruption)?,
                    ),
                ),
            ))
        }
        Some("ASSET_REVISION") => {
            let tuple = transaction
                .query_row(
                    "SELECT ar.asset_id, de.aggregate_revision, de.occurred_at_seconds, de.occurred_at_nanos FROM asset_revisions ar JOIN domain_events de ON de.command_id=?2 AND de.event_type='asset.revision.created.v1' AND de.aggregate_kind='ASSET_REVISION' AND de.aggregate_id=ar.asset_revision_id WHERE ar.asset_revision_id=?1 AND (SELECT count(*) FROM domain_events WHERE command_id=?2 AND event_type='asset.revision.created.v1' AND aggregate_kind='ASSET_REVISION' AND aggregate_id=ar.asset_revision_id)=1",
                    params![result_id.as_slice(), row.command_id.as_slice()],
                    |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)),
                )
                .optional()
                .map_err(sqlite)?
                .ok_or(AssetStoreError::StorageCorruption)?;
            let asset = id_bytes(Some(&tuple.0))?;
            Ok(CommandResult::AssetRevision(AssetRevisionResult::new(
                Id::from_bytes(asset).map_err(|_| AssetStoreError::StorageCorruption)?,
                Id::from_bytes(result_id).map_err(|_| AssetStoreError::StorageCorruption)?,
                parse_revision(&tuple.1)?,
                Timestamp::from_unix_seconds_nanos(
                    tuple.2,
                    u32::try_from(tuple.3).map_err(|_| AssetStoreError::StorageCorruption)?,
                )
                .map_err(|_| AssetStoreError::StorageCorruption)?,
            )))
        }
        Some("LOCATION") => {
            let tuple=transaction.query_row("SELECT l.blob_digest,de.aggregate_revision FROM locations l JOIN domain_events de ON de.command_id=?2 AND de.event_type='blob.location.recorded.v1' AND de.aggregate_kind='BLOB' AND de.aggregate_id=l.blob_digest WHERE l.location_id=?1 AND l.custody='MANAGED' AND l.durability='DURABLE' AND l.lifecycle='AVAILABLE' AND (SELECT count(*) FROM domain_events WHERE command_id=?2 AND event_type='blob.location.recorded.v1' AND aggregate_kind='BLOB' AND aggregate_id=l.blob_digest)=1", params![result_id.as_slice(), row.command_id.as_slice()], |r| Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,Vec<u8>>(1)?))).optional().map_err(sqlite)?.ok_or(AssetStoreError::StorageCorruption)?;
            Ok(CommandResult::Location(LocationResult::new(
                Sha256Digest::from_bytes(
                    tuple
                        .0
                        .try_into()
                        .map_err(|_| AssetStoreError::StorageCorruption)?,
                ),
                Id::from_bytes(result_id).map_err(|_| AssetStoreError::StorageCorruption)?,
                parse_revision(&tuple.1)?,
            )))
        }
        _ => Err(AssetStoreError::StorageCorruption),
    }
}

fn replay_external_result(
    transaction: &Transaction<'_>,
    row: &CommandRow,
) -> Result<CommandResult, AssetStoreError> {
    match replay_result(transaction, row)? {
        result @ CommandResult::ManagedRegistration(_) => Ok(result),
        _ => Err(AssetStoreError::StorageCorruption),
    }
}

fn parse_safe_code(row: &CommandRow) -> Result<ErrorCode, AssetStoreError> {
    row.safe_error_code
        .as_deref()
        .ok_or(AssetStoreError::StorageCorruption)?
        .parse()
        .map_err(|_| AssetStoreError::StorageCorruption)
}
fn id_bytes(value: Option<&[u8]>) -> Result<[u8; 16], AssetStoreError> {
    value
        .ok_or(AssetStoreError::StorageCorruption)?
        .try_into()
        .map_err(|_| AssetStoreError::StorageCorruption)
}
pub(crate) fn revision_bytes(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}
pub(crate) fn parse_revision(value: &[u8]) -> Result<RevisionNo, AssetStoreError> {
    Ok(RevisionNo::new(u64::from_be_bytes(
        value
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
    )))
}
fn validate_descriptor(value: &str, max: usize) -> Result<(), AssetStoreError> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        Err(AssetStoreError::Validation)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command as ProcessCommand, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use mengxia_domain::{
        Asset, AssetGraph, AssetKind, AssetRevision, ContentKind, CreateAssetRevisionValues,
        Location, LogicalName, Representation, RepresentationPurpose, Resource, ResourceKind,
        RevisionMember, RevisionRepresentation, RevisionResource,
    };
    use mengxia_events::{DomainEvent, ProvenanceEvent};
    use mengxia_ports::ASSET_REVISION_CREATE_V1;

    use super::*;
    use crate::migration::{
        LibraryIdentity, bootstrap_schema, prepare_current_library_schema, verify_bootstrap_schema,
        verify_current_library_schema,
    };
    use crate::runtime::verify_and_harden;

    const OWNER_UID: u32 = 501;
    const CRASH_DATABASE_ENV: &str = "MENGXIA_TASK006_TRANSACTION_CRASH_DB";
    const CRASH_BOUNDARY_ENV: &str = "MENGXIA_TASK006_TRANSACTION_CRASH_BOUNDARY";

    fn fixed_id<T>(tail: u8) -> Id<T> {
        let mut bytes = [
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x00,
        ];
        bytes[15] = tail;
        Id::from_bytes(bytes).expect("fixed UUIDv7")
    }

    fn fixed_timestamp() -> Timestamp {
        Timestamp::from_unix_seconds_nanos(1_777_000_100, 123_456_789).expect("fixed timestamp")
    }

    fn fixed_runtime_id(tail: u8) -> [u8; 16] {
        fixed_id::<StoredRuntime>(tail).to_bytes()
    }

    fn local_backend_id(digit: char) -> String {
        format!("{LOCAL_BACKEND_PREFIX}{}", digit.to_string().repeat(64))
    }

    fn local_backend_fixture() -> Connection {
        let connection = Connection::open_in_memory().expect("open backend preflight fixture");
        connection
            .execute_batch(
                "CREATE TABLE locations (
                    location_id BLOB NOT NULL PRIMARY KEY,
                    backend_id TEXT NOT NULL,
                    locator TEXT NOT NULL,
                    UNIQUE (backend_id, locator)
                ) STRICT;",
            )
            .expect("create backend preflight fixture");
        connection
    }

    fn insert_backend(connection: &Connection, ordinal: u8, backend_id: &str) {
        connection
            .execute(
                "INSERT INTO locations (location_id, backend_id, locator) VALUES (?1, ?2, ?3)",
                params![vec![ordinal; 16], backend_id, format!("locator-{ordinal}")],
            )
            .expect("insert backend preflight row");
    }

    #[test]
    fn validate_local_managed_backend_is_exact_and_uses_the_unique_index() {
        let candidate = local_backend_id('a');
        let connection = local_backend_fixture();

        validate_local_backend_rows(&connection, &candidate).expect("empty Library is valid");
        insert_backend(&connection, 1, "provider.v1/external");
        insert_backend(&connection, 2, &candidate);
        validate_local_backend_rows(&connection, &candidate)
            .expect("external and matching local backends are valid");

        insert_backend(&connection, 3, &local_backend_id('b'));
        assert_eq!(
            validate_local_backend_rows(&connection, &candidate),
            Err(AssetStoreError::StorageConfiguration)
        );

        for (ordinal, malformed) in [
            (4, "mengxia.local-cas.v1/short"),
            (5, "mengxia.local-cas.v1/not-hex-but-in-family"),
            (
                6,
                "mengxia.local-cas.v1/FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            ),
        ] {
            let malformed_connection = local_backend_fixture();
            insert_backend(&malformed_connection, ordinal, malformed);
            assert_eq!(
                validate_local_backend_rows(&malformed_connection, &candidate),
                Err(AssetStoreError::StorageConfiguration),
                "malformed local-family backend {malformed} must fail closed"
            );
        }

        for sql in [LOCAL_BACKEND_LOWER_SQL, LOCAL_BACKEND_UPPER_SQL] {
            let mut statement = connection
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .expect("prepare backend preflight query plan");
            let details = statement
                .query_map(params![candidate], |row| row.get::<_, String>(3))
                .expect("query backend preflight plan")
                .collect::<Result<Vec<_>, _>>()
                .expect("collect backend preflight plan");
            assert!(
                details
                    .iter()
                    .any(|detail| detail.contains("COVERING INDEX")),
                "backend preflight must use the covering unique index: {details:?}"
            );
            assert!(
                details
                    .iter()
                    .all(|detail| !detail.contains("SCAN locations")),
                "backend preflight must not scan locations: {details:?}"
            );
        }
    }

    struct FixedSqliteControl(IngestDirective);

    impl mengxia_ports::IngestControl for FixedSqliteControl {
        fn checkpoint(&self) -> IngestDirective {
            self.0
        }
    }

    impl InterruptibleSqliteControl for FixedSqliteControl {
        fn register_interrupt(
            &self,
            _interrupt: Box<dyn SqliteInterrupt>,
        ) -> Result<IngestDirective, mengxia_ports::SqliteInterruptControlError> {
            Ok(self.0)
        }

        fn clear_interrupt(&self) -> Result<(), mengxia_ports::SqliteInterruptControlError> {
            Ok(())
        }
    }

    #[test]
    fn owned_sqlite_interrupt_maps_exactly_and_unowned_interrupt_is_internal() {
        for (directive, expected) in [
            (
                IngestDirective::Stop(IngestStop::DeadlineReached),
                AssetStoreError::DeadlineExceeded,
            ),
            (
                IngestDirective::Stop(IngestStop::Cancelled),
                AssetStoreError::OperationCancelled,
            ),
            (IngestDirective::Continue, AssetStoreError::Internal),
        ] {
            let control: Arc<dyn InterruptibleSqliteControl> =
                Arc::new(FixedSqliteControl(directive));
            let error = rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_INTERRUPT),
                None,
            );
            assert_eq!(controlled_sqlite(error, &control), expected);
        }
    }

    fn valid_claimed_command_row() -> CommandRow {
        CommandRow {
            command_id: fixed_id::<mengxia_ports::Command>(0x61).to_bytes().to_vec(),
            operation_id: ASSET_INGEST_COPY_V1.as_str().to_owned(),
            principal_kind: "LOCAL_OWNER_UID_V1".to_owned(),
            principal_uid: i64::from(OWNER_UID),
            digest: vec![0x62; 32],
            runtime_id: fixed_runtime_id(0x63).to_vec(),
            state: "CLAIMED".to_owned(),
            result_kind: None,
            result_id: None,
            result_location_id: None,
            result_schema_version: None,
            result_payload: None,
            result_payload_sha256: None,
            safe_error_code: None,
            created_at_seconds: fixed_timestamp().unix_seconds(),
            created_at_nanos: i64::from(fixed_timestamp().subsec_nanoseconds()),
            updated_at_seconds: fixed_timestamp().unix_seconds(),
            updated_at_nanos: i64::from(fixed_timestamp().subsec_nanoseconds()),
        }
    }

    #[test]
    fn command_row_mapper_rejects_malformed_typed_fields_and_matrices() {
        validate_command_row(valid_claimed_command_row()).expect("valid external claim row");

        let mut row = valid_claimed_command_row();
        row.command_id = vec![0; 16];
        assert!(matches!(
            validate_command_row(row),
            Err(AssetStoreError::StorageCorruption)
        ));

        let mut row = valid_claimed_command_row();
        row.digest.pop();
        assert!(matches!(
            validate_command_row(row),
            Err(AssetStoreError::StorageCorruption)
        ));

        let mut row = valid_claimed_command_row();
        row.runtime_id = vec![0; 16];
        assert!(matches!(
            validate_command_row(row),
            Err(AssetStoreError::StorageCorruption)
        ));

        let mut row = valid_claimed_command_row();
        row.updated_at_seconds = 253_402_300_800;
        assert!(matches!(
            validate_command_row(row),
            Err(AssetStoreError::StorageCorruption)
        ));

        let mut row = valid_claimed_command_row();
        row.operation_id = ASSET_REVISION_CREATE_V1.as_str().to_owned();
        assert!(matches!(
            validate_command_row(row),
            Err(AssetStoreError::StorageCorruption)
        ));

        let mut row = valid_claimed_command_row();
        row.state = "COMPLETED".to_owned();
        row.result_kind = Some("ASSET_REVISION".to_owned());
        row.result_id = Some(fixed_id::<AssetRevision>(0x64).to_bytes().to_vec());
        assert!(matches!(
            validate_command_row(row),
            Err(AssetStoreError::StorageCorruption)
        ));
    }

    fn crash_request() -> CreateAssetRevisionCommand {
        let blob_digest = Sha256Digest::from_bytes([0x71; 32]);
        let initial = AssetGraph::register_managed(RegisterManagedAssetValues {
            asset_id: fixed_id::<Asset>(0x11),
            asset_kind: AssetKind::new("image").expect("asset kind"),
            asset_revision_id: fixed_id::<AssetRevision>(0x12),
            content_kind: ContentKind::new("raster").expect("content kind"),
            representation_id: fixed_id::<Representation>(0x13),
            representation_purpose: RepresentationPurpose::new("original")
                .expect("representation purpose"),
            resource_id: fixed_id::<Resource>(0x14),
            resource_kind: ResourceKind::new("file").expect("resource kind"),
            logical_name: LogicalName::new("original.bin").expect("logical name"),
            media_type: None,
            blob_digest,
            created_at: fixed_timestamp(),
        })
        .expect("fixed initial Asset graph");
        let revision = initial
            .asset()
            .create_revision(CreateAssetRevisionValues {
                expected_revision: RevisionNo::new(1),
                revision_id: fixed_id::<AssetRevision>(0x15),
                parent_revision_ids: vec![fixed_id::<AssetRevision>(0x12)],
                content_kind: ContentKind::new("raster").expect("revision content kind"),
                representations: vec![
                    RevisionRepresentation::new(
                        fixed_id::<Representation>(0x16),
                        RepresentationPurpose::new("edited")
                            .expect("revision representation purpose"),
                        vec![
                            RevisionResource::new(
                                fixed_id::<Resource>(0x17),
                                ResourceKind::new("file").expect("revision resource kind"),
                                vec![RevisionMember::new(
                                    LogicalName::new("edited.bin").expect("revision logical name"),
                                    blob_digest,
                                )],
                            )
                            .expect("revision resource"),
                        ],
                    )
                    .expect("revision representation"),
                ],
                created_at: fixed_timestamp(),
            })
            .expect("fixed new Asset revision");
        let binding = CommandBinding::new(
            fixed_id::<mengxia_ports::Command>(0x18),
            ASSET_REVISION_CREATE_V1,
            Sha256Digest::from_bytes([0x52; 32]),
        );
        CreateAssetRevisionCommand::new(
            binding,
            revision,
            fixed_id::<DomainEvent>(0x19),
            fixed_id::<ProvenanceEvent>(0x1a),
            fixed_timestamp(),
        )
        .expect("fixed transaction crash request")
    }

    fn registration_fault_binding() -> CommandBinding {
        CommandBinding::new(
            fixed_id::<mengxia_ports::Command>(0x28),
            ASSET_INGEST_COPY_V1,
            Sha256Digest::from_bytes([0x62; 32]),
        )
    }

    fn registration_fault_completion() -> ExternalIngestCompletion {
        ExternalIngestCompletion::new(
            registration_fault_binding(),
            mengxia_ports::DurableBlob::__from_verified_local_adapter(
                Sha256Digest::from_bytes([0x72; 32]),
                4096,
                [0x31; 32],
            ),
            mengxia_ports::ManagedRegistrationPlan::new(
                fixed_id::<Asset>(0x21),
                AssetKind::new("image").expect("registration fault asset kind"),
                fixed_id::<AssetRevision>(0x22),
                ContentKind::new("raster").expect("registration fault content kind"),
                fixed_id::<Representation>(0x23),
                RepresentationPurpose::new("original")
                    .expect("registration fault representation purpose"),
                fixed_id::<Resource>(0x24),
                ResourceKind::new("file").expect("registration fault resource kind"),
                LogicalName::new("fault.bin").expect("registration fault logical name"),
                None,
                fixed_id::<Location>(0x25),
            ),
            fixed_id::<DomainEvent>(0x26),
            fixed_id::<ProvenanceEvent>(0x27),
            fixed_timestamp(),
        )
        .expect("registration fault completion")
    }

    fn location_fault_request() -> RecordManagedLocationCommand {
        RecordManagedLocationCommand::new(
            CommandBinding::new(
                fixed_id::<mengxia_ports::Command>(0x32),
                BLOB_LOCATION_RECORD_V1,
                Sha256Digest::from_bytes([0x63; 32]),
            ),
            mengxia_ports::DurableBlob::__from_verified_local_adapter(
                Sha256Digest::from_bytes([0x71; 32]),
                8192,
                [0x32; 32],
            ),
            fixed_id::<Location>(0x31),
            RevisionNo::new(1),
            fixed_id::<DomainEvent>(0x33),
            fixed_timestamp(),
        )
        .expect("location fault request")
    }

    fn lifecycle_request(
        command_tail: u8,
        digest_byte: u8,
        event_tail: u8,
        operation: mengxia_ports::OperationId,
        expected_revision: u64,
        target: mengxia_domain::AssetLifecycle,
    ) -> AssetLifecycleCommand {
        AssetLifecycleCommand::new(
            CommandBinding::new(
                fixed_id::<Command>(command_tail),
                operation,
                Sha256Digest::from_bytes([digest_byte; 32]),
            ),
            fixed_id::<Asset>(0x11),
            RevisionNo::new(expected_revision),
            target,
            fixed_id::<DomainEvent>(event_tail),
            fixed_timestamp(),
        )
        .expect("valid lifecycle request")
    }

    #[test]
    fn asset_lifecycle_is_atomic_replayable_and_transition_guarded() {
        let directory = create_crash_fixture("asset-lifecycle", 0);
        let database = std::path::Path::new(&directory).join("library.sqlite3");
        let mut connection = Connection::open(&database).expect("open lifecycle fixture");
        verify_and_harden(&connection, Duration::from_millis(5000))
            .expect("harden lifecycle fixture");
        let metadata = verify_current_library_schema(&connection).expect("exact lifecycle schema");
        let context = StoreContext {
            metadata,
            runtime_id: fixed_runtime_id(0x51),
        };

        let retire = lifecycle_request(
            0x52,
            0x53,
            0x54,
            ASSET_RETIRE_V1,
            1,
            mengxia_domain::AssetLifecycle::Retired,
        );
        assert!(matches!(
            change_asset_lifecycle(&mut connection, context, retire),
            Ok(MutationOutcome::Applied(CommandResult::Versioned(
                VersionedCommandResult { .. }
            )))
        ));
        let retired_state: (String, Vec<u8>, Option<i64>, Option<i64>, i64, i64) = connection
            .query_row(
                "SELECT lifecycle, revision, updated_at_seconds, updated_at_nanos, (SELECT count(*) FROM domain_events WHERE event_type='asset.retired.v1'), (SELECT count(*) FROM commands WHERE state='COMPLETED' AND result_kind='ASSET_LIFECYCLE') FROM assets WHERE asset_id=?1",
                [fixed_id::<Asset>(0x11).to_bytes().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .expect("read retired state");
        assert_eq!(
            retired_state,
            (
                "RETIRED".to_owned(),
                2_u64.to_be_bytes().to_vec(),
                Some(fixed_timestamp().unix_seconds()),
                Some(i64::from(fixed_timestamp().subsec_nanoseconds())),
                1,
                1,
            )
        );

        let replay = lifecycle_request(
            0x52,
            0x53,
            0x55,
            ASSET_RETIRE_V1,
            1,
            mengxia_domain::AssetLifecycle::Retired,
        );
        assert!(matches!(
            change_asset_lifecycle(&mut connection, context, replay),
            Ok(MutationOutcome::Replay(CommandResult::Versioned(_)))
        ));
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM domain_events WHERE event_type='asset.retired.v1'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );

        let same_state = lifecycle_request(
            0x56,
            0x57,
            0x58,
            ASSET_RETIRE_V1,
            2,
            mengxia_domain::AssetLifecycle::Retired,
        );
        assert_eq!(
            change_asset_lifecycle(&mut connection, context, same_state),
            Ok(MutationOutcome::TerminalRejected {
                safe_error_code: ErrorCode::InvalidTransition,
            })
        );

        let restore = lifecycle_request(
            0x59,
            0x5a,
            0x5b,
            ASSET_RESTORE_V1,
            2,
            mengxia_domain::AssetLifecycle::Active,
        );
        assert!(matches!(
            change_asset_lifecycle(&mut connection, context, restore),
            Ok(MutationOutcome::Applied(CommandResult::Versioned(_)))
        ));
        let restored: (String, Vec<u8>, i64) = connection
            .query_row(
                "SELECT lifecycle, revision, (SELECT count(*) FROM domain_events WHERE event_type='asset.restored.v1') FROM assets WHERE asset_id=?1",
                [fixed_id::<Asset>(0x11).to_bytes().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            restored,
            ("ACTIVE".to_owned(), 3_u64.to_be_bytes().to_vec(), 1)
        );

        drop(connection);
        fs::remove_dir_all(directory).expect("remove lifecycle fixture");
    }

    fn create_crash_fixture(case: &str, boundary: u8) -> String {
        let directory = std::env::temp_dir().join(format!(
            "mengxia-task006-transaction-{case}-{}-{boundary}",
            std::process::id(),
        ));
        fs::create_dir(&directory).expect("create transaction crash fixture");
        let mut connection = Connection::open(directory.join("library.sqlite3"))
            .expect("open transaction crash fixture");
        verify_and_harden(&connection, Duration::from_millis(5000))
            .expect("harden transaction crash fixture");
        bootstrap_schema(
            &mut connection,
            fixed_id::<LibraryIdentity>(0x10),
            OWNER_UID,
            fixed_timestamp(),
        )
        .expect("bootstrap transaction crash fixture");
        let metadata = verify_bootstrap_schema(&connection).expect("read bootstrap metadata");
        prepare_current_library_schema(&mut connection, metadata)
            .expect("apply asset migration to transaction crash fixture");
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("begin transaction crash fixture setup");
        transaction
            .execute(
                "INSERT INTO assets (asset_id, kind, lifecycle, revision, created_at_seconds, created_at_nanos, created_by_uid) VALUES (?1, 'image', 'ACTIVE', ?2, ?3, ?4, ?5)",
                params![
                    fixed_id::<Asset>(0x11).to_bytes().as_slice(),
                    1_u64.to_be_bytes().as_slice(),
                    fixed_timestamp().unix_seconds(),
                    i64::from(fixed_timestamp().subsec_nanoseconds()),
                    i64::from(OWNER_UID),
                ],
            )
            .expect("insert initial Asset fixture");
        transaction
            .execute(
                "INSERT INTO asset_revisions (asset_revision_id, asset_id, sequence, content_kind, custody, created_at_seconds, created_at_nanos, created_by_uid) VALUES (?1, ?2, 1, 'raster', 'MANAGED', ?3, ?4, ?5)",
                params![
                    fixed_id::<AssetRevision>(0x12).to_bytes().as_slice(),
                    fixed_id::<Asset>(0x11).to_bytes().as_slice(),
                    fixed_timestamp().unix_seconds(),
                    i64::from(fixed_timestamp().subsec_nanoseconds()),
                    i64::from(OWNER_UID),
                ],
            )
            .expect("insert initial AssetRevision fixture");
        transaction
            .execute(
                "INSERT INTO blobs (digest, byte_length, media_type, lifecycle, revision, verified_at_seconds, verified_at_nanos) VALUES (?1, 8192, NULL, 'AVAILABLE', ?2, ?3, ?4)",
                params![
                    Sha256Digest::from_bytes([0x71; 32]).to_bytes().as_slice(),
                    1_u64.to_be_bytes().as_slice(),
                    fixed_timestamp().unix_seconds(),
                    i64::from(fixed_timestamp().subsec_nanoseconds()),
                ],
            )
            .expect("insert verified Blob fixture");
        transaction
            .execute(
                "INSERT INTO locations (location_id, blob_digest, backend_id, locator, custody, durability, lifecycle, revision, verified_at_seconds, verified_at_nanos) VALUES (?1, ?2, 'local-cas-v1', '71/fixture', 'MANAGED', 'DURABLE', 'AVAILABLE', ?3, ?4, ?5)",
                params![
                    fixed_id::<Location>(0x1b).to_bytes().as_slice(),
                    Sha256Digest::from_bytes([0x71; 32]).to_bytes().as_slice(),
                    1_u64.to_be_bytes().as_slice(),
                    fixed_timestamp().unix_seconds(),
                    i64::from(fixed_timestamp().subsec_nanoseconds()),
                ],
            )
            .expect("insert managed Location fixture");
        transaction
            .commit()
            .expect("commit transaction crash fixture setup");
        drop(connection);
        directory
            .into_os_string()
            .into_string()
            .expect("ASCII transaction crash fixture path")
    }

    #[test]
    fn pure_transaction_sigkill_child_entrypoint() {
        let Some(database) = std::env::var_os(CRASH_DATABASE_ENV) else {
            return;
        };
        let boundary = std::env::var(CRASH_BOUNDARY_ENV)
            .expect("transaction crash boundary")
            .parse::<u8>()
            .expect("numeric transaction crash boundary");
        let mut connection = Connection::open(database).expect("open transaction crash database");
        verify_and_harden(&connection, Duration::from_millis(5000))
            .expect("harden transaction crash connection");
        let metadata = verify_current_library_schema(&connection).expect("exact current schema");
        let context = StoreContext {
            metadata,
            runtime_id: fixed_runtime_id(0x41),
        };
        create_revision_inner(&mut connection, context, crash_request(), |observed| {
            if observed == boundary {
                println!("TASK006-TRANSACTION-BOUNDARY-{observed}");
                std::io::stdout()
                    .flush()
                    .expect("flush crash acknowledgement");
                loop {
                    thread::park();
                }
            }
            Ok(())
        })
        .expect("transaction reaches selected crash boundary");
    }

    #[test]
    fn pure_transaction_fault_boundaries_rollback_every_statement_group() {
        for boundary in 1_u8..=10 {
            let directory = create_crash_fixture("fault", boundary);
            let database = std::path::Path::new(&directory).join("library.sqlite3");
            let mut connection =
                Connection::open(&database).expect("open transaction fault database");
            verify_and_harden(&connection, Duration::from_millis(5000))
                .expect("harden transaction fault connection");
            let metadata =
                verify_current_library_schema(&connection).expect("exact transaction fault schema");
            let context = StoreContext {
                metadata,
                runtime_id: fixed_runtime_id(0x43),
            };
            assert_eq!(
                create_revision_inner(&mut connection, context, crash_request(), |observed| {
                    if observed == boundary {
                        Err(AssetStoreError::Internal)
                    } else {
                        Ok(())
                    }
                }),
                Err(AssetStoreError::Internal),
                "transaction fault boundary {boundary}"
            );
            let state: (i64, i64, i64, i64, i64, Vec<u8>) = connection
                .query_row(
                    "SELECT (SELECT count(*) FROM commands), (SELECT count(*) FROM asset_revisions), (SELECT count(*) FROM domain_events), (SELECT count(*) FROM provenance_events), (SELECT last_sequence FROM event_commit_sequence WHERE singleton=1), revision FROM assets WHERE asset_id=?1",
                    [fixed_id::<Asset>(0x11).to_bytes().as_slice()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )
                .expect("read rolled-back transaction fault state");
            assert_eq!(
                state,
                (0, 1, 0, 0, 0, 1_u64.to_be_bytes().to_vec()),
                "transaction fault boundary {boundary}"
            );
            drop(connection);
            fs::remove_dir_all(directory).expect("remove transaction fault fixture");
        }
    }

    #[test]
    fn external_and_location_statement_fault_boundaries_rollback_every_group() {
        for boundary in 1_u8..=7 {
            let directory = create_crash_fixture("registration-fault", boundary);
            let database = std::path::Path::new(&directory).join("library.sqlite3");
            let mut connection = Connection::open(&database).expect("open registration fixture");
            verify_and_harden(&connection, Duration::from_millis(5000))
                .expect("harden registration fixture");
            let metadata =
                verify_current_library_schema(&connection).expect("exact registration schema");
            let context = StoreContext {
                metadata,
                runtime_id: fixed_runtime_id(0x44),
            };
            assert_eq!(
                claim_external(
                    &mut connection,
                    context,
                    ExternalIngestClaim::new(registration_fault_binding(), fixed_timestamp())
                        .expect("registration claim"),
                ),
                Ok(ExternalClaimOutcome::Claimed)
            );
            assert_eq!(
                complete_external_inner(
                    &mut connection,
                    context,
                    registration_fault_completion(),
                    |observed| {
                        if observed == boundary {
                            Err(AssetStoreError::Internal)
                        } else {
                            Ok(())
                        }
                    },
                ),
                Err(AssetStoreError::Internal),
                "registration fault boundary {boundary}"
            );
            let state: (i64, i64, i64, i64, i64, i64, String) = connection
                .query_row(
                    "SELECT (SELECT count(*) FROM assets), (SELECT count(*) FROM blobs), (SELECT count(*) FROM locations), (SELECT count(*) FROM domain_events), (SELECT count(*) FROM provenance_events), (SELECT last_sequence FROM event_commit_sequence WHERE singleton=1), (SELECT state FROM commands WHERE command_id=?1)",
                    [registration_fault_binding().command_id().to_bytes().as_slice()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
                )
                .expect("read registration rollback state");
            assert_eq!(
                state,
                (1, 1, 1, 0, 0, 0, "CLAIMED".to_owned()),
                "registration fault boundary {boundary}"
            );
            drop(connection);
            fs::remove_dir_all(directory).expect("remove registration fault fixture");
        }

        for boundary in 1_u8..=8 {
            let directory = create_crash_fixture("location-fault", boundary);
            let database = std::path::Path::new(&directory).join("library.sqlite3");
            let mut connection = Connection::open(&database).expect("open location fixture");
            verify_and_harden(&connection, Duration::from_millis(5000))
                .expect("harden location fixture");
            let metadata =
                verify_current_library_schema(&connection).expect("exact location schema");
            let context = StoreContext {
                metadata,
                runtime_id: fixed_runtime_id(0x45),
            };
            assert_eq!(
                record_location_inner(
                    &mut connection,
                    context,
                    location_fault_request(),
                    |observed| {
                        if observed == boundary {
                            Err(AssetStoreError::Internal)
                        } else {
                            Ok(())
                        }
                    },
                ),
                Err(AssetStoreError::Internal),
                "location fault boundary {boundary}"
            );
            let state: (i64, i64, i64, i64, Vec<u8>) = connection
                .query_row(
                    "SELECT (SELECT count(*) FROM commands), (SELECT count(*) FROM locations), (SELECT count(*) FROM domain_events), (SELECT last_sequence FROM event_commit_sequence WHERE singleton=1), revision FROM blobs WHERE digest=?1",
                    [Sha256Digest::from_bytes([0x71; 32]).to_bytes().as_slice()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
                )
                .expect("read location rollback state");
            assert_eq!(
                state,
                (0, 1, 0, 0, 1_u64.to_be_bytes().to_vec()),
                "location fault boundary {boundary}"
            );
            drop(connection);
            fs::remove_dir_all(directory).expect("remove location fault fixture");
        }
    }

    #[test]
    fn pure_transaction_sigkill_before_and_after_commit_is_atomic_and_replayable() {
        for boundary in [10_u8, 11_u8] {
            let directory = create_crash_fixture("sigkill", boundary);
            let database = std::path::Path::new(&directory).join("library.sqlite3");
            let mut child =
                ProcessCommand::new(std::env::current_exe().expect("current test executable"))
                    .arg("asset_repository::tests::pure_transaction_sigkill_child_entrypoint")
                    .arg("--exact")
                    .arg("--nocapture")
                    .env(CRASH_DATABASE_ENV, &database)
                    .env(CRASH_BOUNDARY_ENV, boundary.to_string())
                    .stdout(Stdio::piped())
                    .spawn()
                    .expect("spawn transaction crash child");
            let stdout = child.stdout.take().expect("transaction crash child stdout");
            let (sender, receiver) = mpsc::sync_channel(1);
            let expected = format!("TASK006-TRANSACTION-BOUNDARY-{boundary}\n");
            let expected_reader = expected.clone();
            let reader = thread::spawn(move || {
                let mut stdout = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match stdout.read_line(&mut line) {
                        Ok(0) | Err(_) => {
                            let _ = sender.send(String::new());
                            break;
                        }
                        Ok(_) if line == expected_reader => {
                            let _ = sender.send(line);
                            break;
                        }
                        Ok(_) => {}
                    }
                }
            });
            let acknowledgement = receiver
                .recv_timeout(Duration::from_secs(30))
                .unwrap_or_else(|_| {
                    let _ = child.kill();
                    panic!("transaction crash child timed out at boundary {boundary}")
                });
            assert_eq!(acknowledgement, expected);
            child.kill().expect("SIGKILL transaction crash child");
            let status = child.wait().expect("wait for transaction crash child");
            assert!(!status.success());
            reader
                .join()
                .expect("join transaction acknowledgement reader");

            let mut reopened =
                Connection::open(&database).expect("reopen transaction crash database");
            verify_and_harden(&reopened, Duration::from_millis(5000))
                .expect("recover transaction WAL");
            let metadata = verify_current_library_schema(&reopened)
                .expect("transaction crash preserves exact schema");
            let state: (i64, i64, i64, i64, i64, Vec<u8>) = reopened
                .query_row(
                    "SELECT (SELECT count(*) FROM commands), (SELECT count(*) FROM asset_revisions), (SELECT count(*) FROM domain_events), (SELECT count(*) FROM provenance_events), (SELECT last_sequence FROM event_commit_sequence WHERE singleton=1), revision FROM assets WHERE asset_id=?1",
                    [fixed_id::<Asset>(0x11).to_bytes().as_slice()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )
                .expect("read recovered transaction state");
            if boundary == 10 {
                assert_eq!(state, (0, 1, 0, 0, 0, 1_u64.to_be_bytes().to_vec()));
            } else {
                assert_eq!(state, (1, 2, 1, 1, 1, 2_u64.to_be_bytes().to_vec()));
                let context = StoreContext {
                    metadata,
                    runtime_id: fixed_runtime_id(0x42),
                };
                assert!(matches!(
                    create_revision(&mut reopened, context, crash_request()),
                    Ok(MutationOutcome::Replay(CommandResult::AssetRevision(_)))
                ));
            }
            drop(reopened);
            fs::remove_dir_all(directory).expect("remove transaction SIGKILL fixture");
        }
    }
}
