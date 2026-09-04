use mengxia_domain::{Asset, AssetKind, AssetLifecycle};
use mengxia_ports::{
    AssetPage, AssetPortFuture, AssetQueryPort, AssetStoreError, AssetSummaryView,
    ListAssetsPosition, ListAssetsQuery,
};
use mengxia_types::{Id, RevisionNo, Timestamp};
use rusqlite::{Connection, OptionalExtension as _, params};
use tokio::sync::oneshot;

use super::StoreError;
use super::asset_repository::SqliteAssetStoreHandle;
use super::error::map_reopen_error;

const MAX_EVENT_SCAN: usize = 256;
const MAX_SQLITE_SEQUENCE: u64 = i64::MAX as u64;

struct AssetReadJob<T, F> {
    operation: Option<F>,
    sender: oneshot::Sender<Result<T, AssetStoreError>>,
}

trait ErasedAssetReadJob: Send {
    fn execute(self: Box<Self>, connection: &Connection) -> Result<(), StoreError>;
}

pub(crate) struct AssetReadEnvelope {
    job: Box<dyn ErasedAssetReadJob>,
}

impl AssetReadEnvelope {
    fn new<Job>(job: Job) -> Self
    where
        Job: ErasedAssetReadJob + 'static,
    {
        Self { job: Box::new(job) }
    }

    pub(crate) fn execute(self, connection: &Connection) -> Result<(), StoreError> {
        self.job.execute(connection)
    }
}

impl<T, F> ErasedAssetReadJob for AssetReadJob<T, F>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, AssetStoreError> + Send + 'static,
{
    fn execute(mut self: Box<Self>, connection: &Connection) -> Result<(), StoreError> {
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

impl SqliteAssetStoreHandle {
    fn submit_read<T, F>(&self, operation: F) -> AssetPortFuture<'_, T>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, AssetStoreError> + Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let job = AssetReadEnvelope::new(AssetReadJob {
            operation: Some(operation),
            sender,
        });
        let lifecycle_receipt = self.inner.enqueue_asset_read(job);
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

impl AssetQueryPort for SqliteAssetStoreHandle {
    fn list_assets(&self, request: ListAssetsQuery) -> AssetPortFuture<'_, AssetPage> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        self.submit_read(move |connection| list_assets(connection, library_id, request))
    }
}

fn list_assets(
    connection: &Connection,
    library_id: [u8; 16],
    request: ListAssetsQuery,
) -> Result<AssetPage, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let allocator: i64 = transaction
        .query_row(
            "SELECT last_sequence FROM event_commit_sequence WHERE singleton=1",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite)?;
    let maximum: i64 = transaction
        .query_row(
            "SELECT coalesce(max(commit_sequence), 0) FROM domain_events",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite)?;
    let allocator = valid_sequence(allocator)?;
    let maximum = valid_sequence(maximum)?;

    let (snapshot, mut last_examined) = match request.position() {
        ListAssetsPosition::First => {
            if allocator != maximum {
                return Err(AssetStoreError::StorageCorruption);
            }
            (allocator, 0)
        }
        ListAssetsPosition::After {
            library_id: cursor_library,
            snapshot_sequence,
            last_examined_sequence,
        } => {
            if cursor_library != library_id
                || snapshot_sequence == 0
                || snapshot_sequence > MAX_SQLITE_SEQUENCE
                || last_examined_sequence == 0
                || last_examined_sequence >= snapshot_sequence
            {
                return Err(AssetStoreError::Validation);
            }
            if allocator < snapshot_sequence {
                return Err(AssetStoreError::StorageCorruption);
            }
            require_event(&transaction, last_examined_sequence)?;
            require_event(&transaction, snapshot_sequence)?;
            (snapshot_sequence, last_examined_sequence)
        }
    };

    if snapshot == 0 {
        transaction.commit().map_err(sqlite)?;
        return AssetPage::__from_store(0, Vec::new(), None);
    }

    let page_size = usize::from(request.page_size());
    let mut assets = Vec::with_capacity(page_size);
    let mut statement = transaction
        .prepare(
            "SELECT commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, occurred_at_seconds, occurred_at_nanos FROM domain_events WHERE commit_sequence > ?1 AND commit_sequence <= ?2 ORDER BY commit_sequence LIMIT ?3",
        )
        .map_err(sqlite)?;
    let mut rows = statement
        .query(params![
            i64::try_from(last_examined).map_err(|_| AssetStoreError::Validation)?,
            i64::try_from(snapshot).map_err(|_| AssetStoreError::Validation)?,
            i64::try_from(MAX_EVENT_SCAN).expect("scan cap fits i64"),
        ])
        .map_err(sqlite)?;
    let mut stepped = 0_usize;
    while assets.len() < page_size {
        let Some(row) = rows.next().map_err(sqlite)? else {
            break;
        };
        stepped += 1;
        let sequence = valid_sequence(row.get::<_, i64>(0).map_err(sqlite)?)?;
        if sequence != last_examined + 1 {
            return Err(AssetStoreError::StorageCorruption);
        }
        last_examined = sequence;
        let event = EventRow {
            command_id: row.get(1).map_err(sqlite)?,
            event_type: row.get(2).map_err(sqlite)?,
            schema_version: row.get(3).map_err(sqlite)?,
            aggregate_kind: row.get(4).map_err(sqlite)?,
            aggregate_id: row.get(5).map_err(sqlite)?,
            aggregate_revision: row.get(6).map_err(sqlite)?,
            occurred_at_seconds: row.get(7).map_err(sqlite)?,
            occurred_at_nanos: row.get(8).map_err(sqlite)?,
        };
        if event.event_type == "asset.registered.v1" {
            assets.push(read_registered_asset(&transaction, sequence, event)?);
        }
    }
    drop(rows);
    drop(statement);

    if stepped == 0 && last_examined < snapshot {
        return Err(AssetStoreError::StorageCorruption);
    }
    let next = if last_examined == snapshot {
        None
    } else {
        Some(ListAssetsPosition::after(
            library_id,
            snapshot,
            last_examined,
        )?)
    };
    transaction.commit().map_err(sqlite)?;
    AssetPage::__from_store(snapshot, assets, next)
}

struct EventRow {
    command_id: Vec<u8>,
    event_type: String,
    schema_version: i64,
    aggregate_kind: String,
    aggregate_id: Vec<u8>,
    aggregate_revision: Option<Vec<u8>>,
    occurred_at_seconds: i64,
    occurred_at_nanos: i64,
}

fn read_registered_asset(
    connection: &Connection,
    sequence: u64,
    event: EventRow,
) -> Result<AssetSummaryView, AssetStoreError> {
    if event.schema_version != 1
        || event.aggregate_kind != "ASSET"
        || event.command_id.len() != 16
        || event.aggregate_id.len() != 16
        || parse_revision(event.aggregate_revision.as_deref())?.get() != 1
    {
        return Err(AssetStoreError::StorageCorruption);
    }
    let asset_id = Id::<Asset>::from_bytes(
        event
            .aggregate_id
            .as_slice()
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)?;
    let occurred_at = timestamp(event.occurred_at_seconds, event.occurred_at_nanos)?;

    let command_valid = connection
        .query_row(
            "SELECT 1 FROM commands WHERE command_id=?1 AND operation_id='asset.ingest.v1' AND state='COMPLETED' AND result_kind='ASSET' AND result_id=?2 AND result_location_id IS NOT NULL AND safe_error_code IS NULL",
            params![event.command_id, event.aggregate_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(sqlite)?
        .is_some();
    let event_count: i64 = connection
        .query_row(
            "SELECT count(*) FROM domain_events WHERE event_type='asset.registered.v1' AND schema_version=1 AND aggregate_kind='ASSET' AND aggregate_id=?1",
            params![asset_id.to_bytes().as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite)?;
    if !command_valid || event_count != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }

    let row = connection
        .query_row(
            "SELECT kind, lifecycle, revision, created_at_seconds, created_at_nanos FROM assets WHERE asset_id=?1",
            params![asset_id.to_bytes().as_slice()],
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
        .map_err(sqlite)?
        .ok_or(AssetStoreError::StorageCorruption)?;
    let created_at = timestamp(row.3, row.4)?;
    if created_at != occurred_at {
        return Err(AssetStoreError::StorageCorruption);
    }
    let kind = AssetKind::new(row.0).map_err(|_| AssetStoreError::StorageCorruption)?;
    let lifecycle = match row.1.as_str() {
        "ACTIVE" => AssetLifecycle::Active,
        "RETIRED" => AssetLifecycle::Retired,
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    Ok(AssetSummaryView::__from_store(
        asset_id,
        kind,
        lifecycle,
        parse_revision(Some(&row.2))?,
        created_at,
        sequence,
    ))
}

fn require_event(connection: &Connection, sequence: u64) -> Result<(), AssetStoreError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM domain_events WHERE commit_sequence=?1",
            params![i64::try_from(sequence).map_err(|_| AssetStoreError::Validation)?],
            |_| Ok(()),
        )
        .optional()
        .map_err(sqlite)?;
    if exists.is_some() {
        Ok(())
    } else {
        Err(AssetStoreError::StorageCorruption)
    }
}

fn valid_sequence(value: i64) -> Result<u64, AssetStoreError> {
    u64::try_from(value).map_err(|_| AssetStoreError::StorageCorruption)
}

fn parse_revision(bytes: Option<&[u8]>) -> Result<RevisionNo, AssetStoreError> {
    let bytes: [u8; 8] = bytes
        .ok_or(AssetStoreError::StorageCorruption)?
        .try_into()
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    let value = u64::from_be_bytes(bytes);
    if value == 0 {
        return Err(AssetStoreError::StorageCorruption);
    }
    Ok(RevisionNo::new(value))
}

fn timestamp(seconds: i64, nanos: i64) -> Result<Timestamp, AssetStoreError> {
    Timestamp::from_unix_seconds_nanos(
        seconds,
        u32::try_from(nanos).map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)
}

fn sqlite(error: rusqlite::Error) -> AssetStoreError {
    map_store_error(map_reopen_error(error))
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
