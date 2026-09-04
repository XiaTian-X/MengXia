use mengxia_domain::Location;
use mengxia_ports::{
    ASSET_INGEST_COPY_V1, ASSET_MATERIALIZE_V1, AssetPortFuture, AssetStoreError, IntegrityFinding,
    IntegrityIssueKind, IntegrityObjectId, IntegrityObjectKind, IntegrityRemediation,
    IntegritySeverity, RegisteredBlobVerificationCandidate, VerificationScanPosition,
    VerificationSnapshot, VerificationStorePage, VerificationStorePort,
};
use mengxia_types::{Id, RevisionNo, Sha256Digest};
use rusqlite::{Connection, params};

use super::StoreError;
use super::asset_repository::SqliteAssetStoreHandle;
use super::error::map_reopen_error;

const STORE_SCAN_PAGE_MAX: usize = 256;

impl VerificationStorePort for SqliteAssetStoreHandle {
    fn capture_verification_snapshot(&self) -> AssetPortFuture<'_, VerificationSnapshot> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        self.submit_read(move |connection| capture_snapshot(connection, library_id))
    }

    fn scan_verification_page(
        &self,
        snapshot: VerificationSnapshot,
        position: VerificationScanPosition,
    ) -> AssetPortFuture<'_, VerificationStorePage> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        let runtime_id = self.inner.runtime_id();
        self.submit_read(move |connection| {
            if snapshot.library_id() != library_id {
                return Err(AssetStoreError::Validation);
            }
            match position {
                VerificationScanPosition::CommandsAfter(after) => {
                    scan_commands(connection, runtime_id, after)
                }
                VerificationScanPosition::ManagedLocationsAfter(after) => {
                    scan_locations(connection, snapshot.snapshot_commit_sequence(), after)
                }
                VerificationScanPosition::Complete => VerificationStorePage::__from_store(
                    Vec::new(),
                    Vec::new(),
                    VerificationScanPosition::Complete,
                ),
            }
        })
    }
}

fn capture_snapshot(
    connection: &Connection,
    library_id: [u8; 16],
) -> Result<VerificationSnapshot, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let quick: String = transaction
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(sqlite)?;
    if quick != "ok" {
        return Err(AssetStoreError::StorageCorruption);
    }
    let foreign_key_failure = transaction
        .prepare("PRAGMA foreign_key_check")
        .map_err(sqlite)?
        .query([])
        .map_err(sqlite)?
        .next()
        .map_err(sqlite)?
        .is_some();
    if foreign_key_failure {
        return Err(AssetStoreError::StorageCorruption);
    }
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
    let allocator = sequence(allocator)?;
    if allocator != sequence(maximum)? {
        return Err(AssetStoreError::StorageCorruption);
    }
    transaction.commit().map_err(sqlite)?;
    VerificationSnapshot::__from_store(library_id, allocator)
}

fn scan_commands(
    connection: &Connection,
    current_runtime_id: [u8; 16],
    after: Option<Id<mengxia_ports::Command>>,
) -> Result<VerificationStorePage, AssetStoreError> {
    let mut statement = connection
        .prepare(
            "SELECT command_id, operation_id, state, store_runtime_id, safe_error_code FROM commands WHERE (?1 IS NULL OR command_id>?1) ORDER BY command_id LIMIT 257",
        )
        .map_err(sqlite)?;
    let after_bytes = after.map(Id::to_bytes);
    let mut rows = statement
        .query(params![after_bytes.as_ref().map(<[u8; 16]>::as_slice)])
        .map_err(sqlite)?;
    let mut findings = Vec::new();
    let mut command_ids = Vec::new();
    while let Some(row) = rows.next().map_err(sqlite)? {
        let id = typed_id::<mengxia_ports::Command>(&row.get::<_, Vec<u8>>(0).map_err(sqlite)?)?;
        let operation = row.get::<_, String>(1).map_err(sqlite)?;
        let state = row.get::<_, String>(2).map_err(sqlite)?;
        let runtime = row.get::<_, Option<Vec<u8>>>(3).map_err(sqlite)?;
        let safe_code = row.get::<_, Option<String>>(4).map_err(sqlite)?;
        command_ids.push(id);
        if command_ids.len() > STORE_SCAN_PAGE_MAX {
            break;
        }
        let known_operation = matches!(
            operation.as_str(),
            "asset.ingest.v1"
                | "asset.revision.create.v1"
                | "blob.location.record.v1"
                | "asset.materialize.v1"
        );
        if !known_operation {
            findings.push(graph_finding(
                IntegrityObjectKind::Command,
                Some(IntegrityObjectId::Uuid(id.to_bytes())),
            )?);
            continue;
        }
        let external = operation == ASSET_INGEST_COPY_V1.as_str()
            || operation == ASSET_MATERIALIZE_V1.as_str();
        let runtime = runtime.as_deref().map(runtime_id).transpose()?;
        let recovery = match state.as_str() {
            "CLAIMED" if external && runtime != Some(current_runtime_id) => Some(false),
            "RECOVERY_REQUIRED" if external && safe_code.is_some() => Some(true),
            "CLAIMED" | "COMPLETED" | "TERMINAL_REJECTED" => None,
            _ => {
                findings.push(graph_finding(
                    IntegrityObjectKind::Command,
                    Some(IntegrityObjectId::Uuid(id.to_bytes())),
                )?);
                None
            }
        };
        if recovery.is_some() {
            let materialize = operation == ASSET_MATERIALIZE_V1.as_str();
            findings.push(IntegrityFinding::new(
                if materialize {
                    IntegrityIssueKind::MaterializationRecoveryRequired
                } else {
                    IntegrityIssueKind::CommandRecoveryRequired
                },
                IntegritySeverity::OperatorAction,
                if materialize {
                    IntegrityObjectKind::Materialization
                } else {
                    IntegrityObjectKind::Command
                },
                Some(IntegrityObjectId::Uuid(id.to_bytes())),
                if materialize {
                    IntegrityRemediation::OperatorConfiguration
                } else {
                    IntegrityRemediation::OperatorOrRuntimeAction
                },
            )?);
        }
    }
    let has_more = command_ids.len() > STORE_SCAN_PAGE_MAX;
    command_ids.truncate(STORE_SCAN_PAGE_MAX);
    let next = if has_more {
        VerificationScanPosition::CommandsAfter(command_ids.last().copied())
    } else {
        VerificationScanPosition::ManagedLocationsAfter(None)
    };
    VerificationStorePage::__from_store(findings, Vec::new(), next)
}

fn scan_locations(
    connection: &Connection,
    snapshot_sequence: u64,
    after: Option<Id<Location>>,
) -> Result<VerificationStorePage, AssetStoreError> {
    let snapshot = i64::try_from(snapshot_sequence).map_err(|_| AssetStoreError::Validation)?;
    let after_bytes = after.map(Id::to_bytes);
    let mut statement = connection
        .prepare(
            "SELECT l.location_id, l.blob_digest, l.backend_id, l.locator, l.custody, l.durability, l.lifecycle, l.revision, b.byte_length, b.lifecycle, b.revision, (SELECT count(*) FROM commands c JOIN domain_events de ON de.command_id=c.command_id WHERE c.result_location_id=l.location_id AND c.state='COMPLETED' AND de.commit_sequence<=?2 AND ((c.operation_id='asset.ingest.v1' AND de.event_type='asset.registered.v1' AND de.aggregate_kind='ASSET') OR (c.operation_id='blob.location.record.v1' AND de.event_type='blob.location.recorded.v1' AND de.aggregate_kind='BLOB'))) FROM locations l JOIN blobs b ON b.digest=l.blob_digest WHERE (?1 IS NULL OR l.location_id>?1) AND EXISTS (SELECT 1 FROM commands c JOIN domain_events de ON de.command_id=c.command_id WHERE c.result_location_id=l.location_id AND c.state='COMPLETED' AND de.commit_sequence<=?2 AND ((c.operation_id='asset.ingest.v1' AND de.event_type='asset.registered.v1' AND de.aggregate_kind='ASSET') OR (c.operation_id='blob.location.record.v1' AND de.event_type='blob.location.recorded.v1' AND de.aggregate_kind='BLOB')) LIMIT 1) ORDER BY l.location_id LIMIT 257",
        )
        .map_err(sqlite)?;
    let mut rows = statement
        .query(params![
            after_bytes.as_ref().map(<[u8; 16]>::as_slice),
            snapshot
        ])
        .map_err(sqlite)?;
    let mut candidates = Vec::new();
    let mut findings = Vec::new();
    let mut location_ids = Vec::new();
    while let Some(row) = rows.next().map_err(sqlite)? {
        let location_id = typed_id::<Location>(&row.get::<_, Vec<u8>>(0).map_err(sqlite)?)?;
        location_ids.push(location_id);
        if location_ids.len() > STORE_SCAN_PAGE_MAX {
            break;
        }
        let digest = digest(&row.get::<_, Vec<u8>>(1).map_err(sqlite)?)?;
        let backend = row.get::<_, String>(2).map_err(sqlite)?;
        let locator = row.get::<_, String>(3).map_err(sqlite)?;
        let custody = row.get::<_, String>(4).map_err(sqlite)?;
        let durability = row.get::<_, String>(5).map_err(sqlite)?;
        let location_lifecycle = row.get::<_, String>(6).map_err(sqlite)?;
        let location_revision = revision(&row.get::<_, Vec<u8>>(7).map_err(sqlite)?)?;
        let byte_length = u64::try_from(row.get::<_, i64>(8).map_err(sqlite)?)
            .ok()
            .filter(|length| *length <= 1_099_511_627_776)
            .ok_or(AssetStoreError::StorageCorruption)?;
        let blob_lifecycle = row.get::<_, String>(9).map_err(sqlite)?;
        let blob_revision = revision(&row.get::<_, Vec<u8>>(10).map_err(sqlite)?)?;
        let event_count = row.get::<_, i64>(11).map_err(sqlite)?;
        let valid = custody == "MANAGED"
            && durability == "DURABLE"
            && location_lifecycle == "AVAILABLE"
            && blob_lifecycle == "AVAILABLE"
            && event_count == 1;
        if !valid {
            findings.push(graph_finding(
                IntegrityObjectKind::Location,
                Some(IntegrityObjectId::Uuid(location_id.to_bytes())),
            )?);
            continue;
        }
        candidates.push(RegisteredBlobVerificationCandidate::__from_store(
            digest,
            byte_length,
            blob_revision,
            location_id,
            location_revision,
            backend,
            locator,
        )?);
    }
    let has_more = location_ids.len() > STORE_SCAN_PAGE_MAX;
    location_ids.truncate(STORE_SCAN_PAGE_MAX);
    let next = if has_more {
        VerificationScanPosition::ManagedLocationsAfter(location_ids.last().copied())
    } else {
        VerificationScanPosition::Complete
    };
    VerificationStorePage::__from_store(findings, candidates, next)
}

fn graph_finding(
    object_kind: IntegrityObjectKind,
    object_id: Option<IntegrityObjectId>,
) -> Result<IntegrityFinding, AssetStoreError> {
    IntegrityFinding::new(
        IntegrityIssueKind::EventOrGraphInconsistent,
        IntegritySeverity::FatalLocal,
        object_kind,
        object_id,
        IntegrityRemediation::None,
    )
}

fn sequence(value: i64) -> Result<u64, AssetStoreError> {
    u64::try_from(value).map_err(|_| AssetStoreError::StorageCorruption)
}

fn typed_id<T>(bytes: &[u8]) -> Result<Id<T>, AssetStoreError> {
    Id::from_bytes(
        bytes
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)
}

fn runtime_id(bytes: &[u8]) -> Result<[u8; 16], AssetStoreError> {
    bytes
        .try_into()
        .map_err(|_| AssetStoreError::StorageCorruption)
}

fn digest(bytes: &[u8]) -> Result<Sha256Digest, AssetStoreError> {
    Ok(Sha256Digest::from_bytes(
        bytes
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
    ))
}

fn revision(bytes: &[u8]) -> Result<RevisionNo, AssetStoreError> {
    let revision = RevisionNo::new(u64::from_be_bytes(
        bytes
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
    ));
    if revision.get() == 0 {
        return Err(AssetStoreError::StorageCorruption);
    }
    Ok(revision)
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
