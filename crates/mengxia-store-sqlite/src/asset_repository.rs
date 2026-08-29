use mengxia_domain::{AssetGraph, RegisterManagedAssetValues};
use mengxia_ports::{
    ASSET_INGEST_COPY_V1, ASSET_REVISION_CREATE_V1, AssetPortFuture, AssetRevisionResult,
    AssetStoreError, AssetUnitOfWork, BLOB_LOCATION_RECORD_V1, Command, CommandBinding,
    CommandResult, CreateAssetRevisionCommand, ExternalClaimOutcome, ExternalDisposition,
    ExternalDispositionOutcome, ExternalIngestClaim, ExternalIngestCompletion,
    ExternalIngestDisposition, LocationResult, ManagedRegistrationResult, MutationOutcome,
    RecordManagedLocationCommand,
};
use mengxia_types::{ErrorCode, Id, RevisionNo, Sha256Digest, Timestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use tokio::sync::oneshot;

use super::StoreError;
use super::error::map_sqlite_error;
use super::lifecycle::StoreHandle;
use super::migration::OpenedLibraryMetadata;

#[derive(Clone)]
pub struct SqliteAssetStoreHandle {
    inner: StoreHandle,
}

impl SqliteAssetStoreHandle {
    pub(crate) const fn new(inner: StoreHandle) -> Self {
        Self { inner }
    }

    fn submit<T, F>(&self, operation: F) -> AssetPortFuture<'_, T>
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
}

#[derive(Clone, Copy)]
struct StoreContext {
    metadata: OpenedLibraryMetadata,
    runtime_id: [u8; 16],
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

fn sqlite(error: rusqlite::Error) -> AssetStoreError {
    map_store_error(map_sqlite_error(error))
}

#[derive(Debug)]
struct CommandRow {
    command_id: Vec<u8>,
    operation_id: String,
    principal_kind: String,
    principal_uid: i64,
    digest: Vec<u8>,
    runtime_id: Vec<u8>,
    state: String,
    result_kind: Option<String>,
    result_id: Option<Vec<u8>>,
    result_location_id: Option<Vec<u8>>,
    safe_error_code: Option<String>,
    created_at_seconds: i64,
    created_at_nanos: i64,
    updated_at_seconds: i64,
    updated_at_nanos: i64,
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

fn read_command(
    transaction: &Transaction<'_>,
    binding: &CommandBinding,
) -> Result<Option<CommandRow>, AssetStoreError> {
    transaction.query_row(
        "SELECT command_id, operation_id, principal_kind, principal_uid, canonical_request_digest, store_runtime_id, state, result_kind, result_id, result_location_id, safe_error_code, created_at_seconds, created_at_nanos, updated_at_seconds, updated_at_nanos FROM commands WHERE command_id = ?1",
        params![binding.command_id().to_bytes().as_slice()],
        |row| Ok(CommandRow { command_id: row.get(0)?, operation_id: row.get(1)?, principal_kind: row.get(2)?, principal_uid: row.get(3)?, digest: row.get(4)?, runtime_id: row.get(5)?, state: row.get(6)?, result_kind: row.get(7)?, result_id: row.get(8)?, result_location_id: row.get(9)?, safe_error_code: row.get(10)?, created_at_seconds: row.get(11)?, created_at_nanos: row.get(12)?, updated_at_seconds: row.get(13)?, updated_at_nanos: row.get(14)? }),
    ).optional().map_err(sqlite)
}

fn validate_command_row(row: CommandRow) -> Result<CommandRow, AssetStoreError> {
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
    match row.state.as_str() {
        "CLAIMED"
            if row.result_kind.is_none()
                && row.result_id.is_none()
                && row.result_location_id.is_none()
                && code.is_none() => {}
        "COMPLETED" if code.is_none() => match row.result_kind.as_deref() {
            Some("ASSET") => {
                Id::<mengxia_domain::Asset>::from_bytes(id_bytes(row.result_id.as_deref())?)
                    .map_err(|_| AssetStoreError::StorageCorruption)?;
                Id::<mengxia_domain::Location>::from_bytes(id_bytes(
                    row.result_location_id.as_deref(),
                )?)
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            }
            Some("ASSET_REVISION") if row.result_location_id.is_none() => {
                Id::<mengxia_domain::AssetRevision>::from_bytes(id_bytes(
                    row.result_id.as_deref(),
                )?)
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            }
            Some("LOCATION") if row.result_location_id.is_none() => {
                Id::<mengxia_domain::Location>::from_bytes(id_bytes(row.result_id.as_deref())?)
                    .map_err(|_| AssetStoreError::StorageCorruption)?;
            }
            _ => return Err(AssetStoreError::StorageCorruption),
        },
        "TERMINAL_REJECTED" | "RECOVERY_REQUIRED"
            if row.result_kind.is_none()
                && row.result_id.is_none()
                && row.result_location_id.is_none()
                && code.is_some() => {}
        _ => return Err(AssetStoreError::StorageCorruption),
    }

    validate_known_command_matrix(&row, code)?;
    Ok(row)
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
        ErrorCode::NotFound | ErrorCode::Conflict | ErrorCode::RevisionExhausted
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

fn binding_matches(row: &CommandRow, binding: &CommandBinding, owner_uid: u32) -> bool {
    row.command_id.as_slice() == binding.command_id().to_bytes()
        && row.operation_id == binding.operation_id().as_str()
        && row.principal_kind == "LOCAL_OWNER_UID_V1"
        && row.principal_uid == i64::from(owner_uid)
        && row.digest.as_slice() == binding.canonical_request_digest().to_bytes()
}

fn insert_claim(
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
            "UPDATE assets SET revision=?2 WHERE asset_id=?1 AND revision=?3",
            params![
                asset_id.as_slice(),
                revision_bytes(revision.resulting_revision().get()).as_slice(),
                revision_bytes(expected).as_slice()
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

fn replay_pure(
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

fn commit_rejection(
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

fn allocate_event_sequences(
    transaction: &Transaction<'_>,
    count: i64,
) -> Result<i64, AssetStoreError> {
    transaction.query_row("UPDATE event_commit_sequence SET last_sequence=last_sequence+?1 WHERE singleton=1 AND ?1 BETWEEN 1 AND 64 AND last_sequence <= 9223372036854775807-?1 RETURNING last_sequence-?1+1", params![count], |row| row.get(0)).optional().map_err(sqlite)?.ok_or(AssetStoreError::RevisionExhausted)
}

fn replay_result(
    transaction: &Transaction<'_>,
    row: &CommandRow,
) -> Result<CommandResult, AssetStoreError> {
    let result_id = id_bytes(row.result_id.as_deref())?;
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
                    "SELECT ar.asset_id, de.aggregate_revision FROM asset_revisions ar JOIN domain_events de ON de.command_id=?2 AND de.event_type='asset.revision.created.v1' AND de.aggregate_kind='ASSET_REVISION' AND de.aggregate_id=ar.asset_revision_id WHERE ar.asset_revision_id=?1 AND (SELECT count(*) FROM domain_events WHERE command_id=?2 AND event_type='asset.revision.created.v1' AND aggregate_kind='ASSET_REVISION' AND aggregate_id=ar.asset_revision_id)=1",
                    params![result_id.as_slice(), row.command_id.as_slice()],
                    |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?)),
                )
                .optional()
                .map_err(sqlite)?
                .ok_or(AssetStoreError::StorageCorruption)?;
            let asset = id_bytes(Some(&tuple.0))?;
            Ok(CommandResult::AssetRevision(AssetRevisionResult::new(
                Id::from_bytes(asset).map_err(|_| AssetStoreError::StorageCorruption)?,
                Id::from_bytes(result_id).map_err(|_| AssetStoreError::StorageCorruption)?,
                parse_revision(&tuple.1)?,
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
fn revision_bytes(value: u64) -> [u8; 8] {
    value.to_be_bytes()
}
fn parse_revision(value: &[u8]) -> Result<RevisionNo, AssetStoreError> {
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
