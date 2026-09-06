use std::sync::Arc;

use mengxia_domain::{
    Asset, AssetKind, AssetLifecycle, AssetRevision, ContentKind, Location, LocationCustody,
    LocationDurability, LocationLifecycle, LogicalName, MediaType, Representation,
    RepresentationPurpose, Resource, ResourceKind, RevisionCustody,
};
use mengxia_ports::{
    AssetLocationView, AssetMemberPage, AssetMemberView, AssetPage, AssetPortFuture,
    AssetQueryPort, AssetStoreError, AssetSummaryView, IngestDirective, IngestStop,
    InspectAssetPosition, InspectAssetQuery, InspectAssetStart, InspectMemberPhase,
    InterruptibleSqliteControl, ListAssetsPosition, ListAssetsQuery, MaterializationSelection,
    ResolvedManagedMember, SqliteInterrupt,
};
use mengxia_types::{Id, RevisionNo, Sha256Digest, Timestamp};
use rusqlite::{Connection, OptionalExtension as _, params};
use tokio::sync::oneshot;

use super::StoreError;
use super::asset_repository::SqliteAssetStoreHandle;
use super::error::map_reopen_error;

const MAX_EVENT_SCAN: usize = 256;
const MAX_SQLITE_SEQUENCE: u64 = i64::MAX as u64;
const MAX_INSPECT_SELECTS: usize = 512;

struct SelectBudget {
    used: usize,
}

impl SelectBudget {
    const fn new() -> Self {
        Self { used: 0 }
    }

    fn consume(&mut self, count: usize) -> Result<(), AssetStoreError> {
        self.used = self
            .used
            .checked_add(count)
            .filter(|used| *used <= MAX_INSPECT_SELECTS)
            .ok_or(AssetStoreError::Internal)?;
        Ok(())
    }
}

struct AssetReadJob<T, F> {
    operation: Option<F>,
    sender: oneshot::Sender<Result<T, AssetStoreError>>,
}

trait ErasedAssetReadJob: Send {
    fn execute(self: Box<Self>, connection: &Connection) -> AssetReadExecution;
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

    pub(crate) fn execute(self, connection: &Connection) -> AssetReadExecution {
        self.job.execute(connection)
    }
}

pub(crate) struct AssetReadExecution {
    pub(crate) result: Result<(), StoreError>,
    pub(crate) replace_connection: bool,
}

impl<T, F> ErasedAssetReadJob for AssetReadJob<T, F>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, AssetStoreError> + Send + 'static,
{
    fn execute(mut self: Box<Self>, connection: &Connection) -> AssetReadExecution {
        let Some(operation) = self.operation.take() else {
            let _ = self.sender.send(Err(AssetStoreError::Internal));
            return AssetReadExecution {
                result: Err(StoreError::Internal),
                replace_connection: false,
            };
        };
        let result = operation(connection);
        let replace_connection = matches!(
            result,
            Err(AssetStoreError::OperationCancelled | AssetStoreError::DeadlineExceeded)
        );
        let fatal = matches!(
            result,
            Err(AssetStoreError::StorageIo
                | AssetStoreError::StorageCorruption
                | AssetStoreError::Internal)
        );
        let _ = self.sender.send(result);
        AssetReadExecution {
            result: if fatal {
                Err(StoreError::Internal)
            } else {
                Ok(())
            },
            replace_connection,
        }
    }
}

impl SqliteAssetStoreHandle {
    pub(crate) fn submit_read<T, F>(&self, operation: F) -> AssetPortFuture<'_, T>
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
                        Err(
                            error @ (AssetStoreError::OperationCancelled
                            | AssetStoreError::DeadlineExceeded),
                        ) => {
                            lifecycle.map_err(map_store_error)?;
                            Err(error)
                        }
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
    fn list_assets(
        &self,
        request: ListAssetsQuery,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> AssetPortFuture<'_, AssetPage> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        self.submit_read(move |connection| {
            with_controlled_read_interrupt(connection, control, |connection| {
                list_assets(connection, library_id, request)
            })
        })
    }

    fn inspect_asset(
        &self,
        request: InspectAssetQuery,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> AssetPortFuture<'_, AssetMemberPage> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        self.submit_read(move |connection| {
            with_controlled_read_interrupt(connection, control, |connection| {
                inspect_asset(connection, library_id, request)
            })
        })
    }

    fn resolve_materialization(
        &self,
        request: MaterializationSelection,
    ) -> AssetPortFuture<'_, ResolvedManagedMember> {
        self.submit_read(move |connection| resolve_materialization(connection, request))
    }
}

pub(crate) fn with_controlled_read_interrupt<T, F>(
    connection: &Connection,
    control: Arc<dyn InterruptibleSqliteControl>,
    operation: F,
) -> Result<T, AssetStoreError>
where
    F: FnOnce(&Connection) -> Result<T, AssetStoreError>,
{
    let interrupt = Box::new(ReadSqliteInterrupt(connection.get_interrupt_handle()));
    match control
        .register_interrupt(interrupt)
        .map_err(|_| AssetStoreError::Internal)?
    {
        IngestDirective::Continue => {}
        IngestDirective::Stop(stop) => return Err(controlled_stop_error(stop)),
    }
    let mut result = operation(connection);
    if matches!(result, Err(AssetStoreError::OperationCancelled)) {
        result = Err(match control.checkpoint() {
            IngestDirective::Stop(stop) => controlled_stop_error(stop),
            IngestDirective::Continue => AssetStoreError::Internal,
        });
    } else if let IngestDirective::Stop(stop) = control.checkpoint() {
        result = Err(controlled_stop_error(stop));
    }
    control
        .clear_interrupt()
        .map_err(|_| AssetStoreError::Internal)?;
    result
}

struct ReadSqliteInterrupt(rusqlite::InterruptHandle);

impl SqliteInterrupt for ReadSqliteInterrupt {
    fn interrupt(&self) {
        self.0.interrupt();
    }
}

fn controlled_stop_error(stop: IngestStop) -> AssetStoreError {
    match stop {
        IngestStop::Cancelled => AssetStoreError::OperationCancelled,
        IngestStop::DeadlineReached => AssetStoreError::DeadlineExceeded,
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

fn inspect_asset(
    connection: &Connection,
    library_id: [u8; 16],
    request: InspectAssetQuery,
) -> Result<AssetMemberPage, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let mut selects = SelectBudget::new();
    let asset_id = request.asset_id();
    selects.consume(1)?;
    let (creation_sequence, creation_event) = match registered_event(&transaction, asset_id) {
        Ok(event) => event,
        Err(AssetStoreError::NotFound) => {
            selects.consume(1)?;
            let asset_exists = transaction
                .query_row(
                    "SELECT 1 FROM assets WHERE asset_id=?1",
                    params![asset_id.to_bytes().as_slice()],
                    |_| Ok(()),
                )
                .optional()
                .map_err(sqlite)?
                .is_some();
            return Err(if asset_exists {
                AssetStoreError::StorageCorruption
            } else {
                AssetStoreError::NotFound
            });
        }
        Err(error) => return Err(error),
    };
    selects.consume(2)?;
    let asset = read_registered_asset(&transaction, creation_sequence, creation_event)?;
    let (selected_revision_id, cursor_position) = match request.start() {
        InspectAssetStart::First => {
            let selected = match request.selected_revision_id() {
                Some(selected) => selected,
                None => {
                    selects.consume(1)?;
                    transaction
                        .query_row(
                        "SELECT asset_revision_id FROM asset_revisions WHERE asset_id=?1 ORDER BY sequence DESC LIMIT 1",
                        params![asset_id.to_bytes().as_slice()],
                        |row| row.get::<_, Vec<u8>>(0),
                    )
                    .optional()
                    .map_err(sqlite)?
                    .ok_or(AssetStoreError::StorageCorruption)
                        .and_then(|bytes| typed_id::<AssetRevision>(&bytes))?
                }
            };
            (selected, None)
        }
        InspectAssetStart::Continue(position) => {
            if position.library_id() != library_id || position.asset_id() != asset_id {
                return Err(AssetStoreError::Validation);
            }
            if position.asset_revision() != asset.revision() {
                return Err(AssetStoreError::Conflict);
            }
            (position.selected_revision_id(), Some(position))
        }
    };
    let revision = read_revision(&transaction, &mut selects, asset_id, selected_revision_id)?;
    let parents = read_parents(&transaction, &mut selects, asset_id, selected_revision_id)?;
    validate_representations(&transaction, &mut selects, selected_revision_id)?;
    let mut current = match cursor_position.and_then(InspectAssetPosition::member) {
        Some(member) => {
            first_resource(&transaction, &mut selects, member.0)?;
            Some(read_member(
                &transaction,
                &mut selects,
                selected_revision_id,
                member,
            )?)
        }
        None => first_member(&transaction, &mut selects, selected_revision_id)?
            .ok_or(AssetStoreError::StorageCorruption)
            .map(Some)?,
    };
    let mut phase = cursor_position
        .and_then(InspectAssetPosition::phase)
        .unwrap_or(InspectMemberPhase::NoLocationSeen);
    let mut last_location = cursor_position.and_then(InspectAssetPosition::last_location_id);
    if let (Some(position), Some(member)) = (cursor_position, current.as_ref())
        && position.blob_revision() != Some(member.blob_revision)
    {
        return Err(AssetStoreError::Conflict);
    }

    let page_size = usize::from(request.page_size());
    let mut output = Vec::with_capacity(page_size);
    let mut next = None;
    while let Some(member) = current {
        let remaining = page_size - output.len();
        let locations = read_locations(
            &transaction,
            &mut selects,
            member.blob_digest,
            last_location,
            remaining + 1,
        )?;
        if locations.is_empty() && last_location.is_none() {
            if revision.custody == RevisionCustody::Managed {
                return Err(AssetStoreError::StorageCorruption);
            }
            output.push(member.view(None));
        } else {
            for location in locations.iter().take(remaining) {
                phase = phase_after(phase, location);
                output.push(member.view(Some(location.view)));
                last_location = Some(location.view.location_id());
            }
            if locations.len() > remaining {
                next = Some(member.position(
                    library_id,
                    asset_id,
                    selected_revision_id,
                    asset.revision(),
                    phase,
                    last_location,
                )?);
                break;
            }
            if revision.custody == RevisionCustody::Managed
                && phase != InspectMemberPhase::RequiredCustodySeen
            {
                return Err(AssetStoreError::StorageCorruption);
            }
        }

        let following = next_member(&transaction, &mut selects, selected_revision_id, &member)?;
        if output.len() == page_size {
            if let Some(following) = following {
                next = Some(following.position(
                    library_id,
                    asset_id,
                    selected_revision_id,
                    asset.revision(),
                    InspectMemberPhase::NoLocationSeen,
                    None,
                )?);
            }
            break;
        }
        current = following;
        phase = InspectMemberPhase::NoLocationSeen;
        last_location = None;
    }
    if output.is_empty() && next.is_some() {
        return Err(AssetStoreError::Internal);
    }
    transaction.commit().map_err(sqlite)?;
    AssetMemberPage::__from_store(
        asset,
        selected_revision_id,
        revision.sequence,
        revision.content_kind,
        revision.custody,
        parents,
        output,
        next,
    )
}

fn resolve_materialization(
    connection: &Connection,
    request: MaterializationSelection,
) -> Result<ResolvedManagedMember, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let member = transaction
        .query_row(
            "SELECT rm.blob_digest, b.byte_length, b.lifecycle FROM assets a JOIN asset_revisions ar ON ar.asset_id=a.asset_id JOIN representations rp ON rp.asset_revision_id=ar.asset_revision_id JOIN resources rs ON rs.representation_id=rp.representation_id JOIN resource_members rm ON rm.resource_id=rs.resource_id JOIN blobs b ON b.digest=rm.blob_digest WHERE a.asset_id=?1 AND ar.asset_revision_id=?2 AND rp.representation_id=?3 AND rs.resource_id=?4 AND rm.ordinal=?5",
            params![
                request.asset_id().to_bytes().as_slice(),
                request.asset_revision_id().to_bytes().as_slice(),
                request.representation_id().to_bytes().as_slice(),
                request.resource_id().to_bytes().as_slice(),
                i64::from(request.member_ordinal()),
            ],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(sqlite)?
        .ok_or(AssetStoreError::NotFound)?;
    if member.2 != "AVAILABLE" {
        return Err(AssetStoreError::StorageCorruption);
    }
    let digest = digest(&member.0)?;
    let byte_length = u64::try_from(member.1)
        .ok()
        .filter(|length| *length <= 1_099_511_627_776)
        .ok_or(AssetStoreError::StorageCorruption)?;

    let mut statement = transaction
        .prepare(
            "SELECT location_id, backend_id, locator, custody, durability, lifecycle FROM locations WHERE blob_digest=?1 AND backend_id=?2 LIMIT 2",
        )
        .map_err(sqlite)?;
    let mut rows = statement
        .query(params![
            digest.to_bytes().as_slice(),
            request.__current_backend_id(),
        ])
        .map_err(sqlite)?;
    let location = rows
        .next()
        .map_err(sqlite)?
        .map(|row| {
            Ok::<_, rusqlite::Error>((
                row.get::<_, Vec<u8>>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .transpose()
        .map_err(sqlite)?
        .ok_or(AssetStoreError::StorageConfiguration)?;
    if rows.next().map_err(sqlite)?.is_some()
        || location.1 != request.__current_backend_id()
        || location.3 != "MANAGED"
        || location.4 != "DURABLE"
        || location.5 != "AVAILABLE"
        || location.2 != canonical_local_locator(digest)
    {
        return Err(AssetStoreError::StorageCorruption);
    }
    drop(rows);
    drop(statement);
    transaction.commit().map_err(sqlite)?;
    ResolvedManagedMember::__from_store(
        request.asset_id(),
        request.asset_revision_id(),
        request.representation_id(),
        request.resource_id(),
        request.member_ordinal(),
        digest,
        byte_length,
        typed_id::<Location>(&location.0)?,
        location.1,
        location.2,
    )
}

fn canonical_local_locator(digest: Sha256Digest) -> String {
    let digest = digest.to_string();
    format!("sha256-v1/{}/{}/{digest}.blob", &digest[..2], &digest[2..4])
}

struct RevisionRow {
    sequence: u32,
    content_kind: ContentKind,
    custody: RevisionCustody,
}

fn read_revision(
    connection: &Connection,
    selects: &mut SelectBudget,
    asset_id: Id<Asset>,
    revision_id: Id<AssetRevision>,
) -> Result<RevisionRow, AssetStoreError> {
    selects.consume(1)?;
    let row = connection
        .query_row(
            "SELECT sequence, content_kind, custody FROM asset_revisions WHERE asset_revision_id=?1 AND asset_id=?2",
            params![
                revision_id.to_bytes().as_slice(),
                asset_id.to_bytes().as_slice()
            ],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(sqlite)?
        .ok_or(AssetStoreError::NotFound)?;
    let sequence = u32::try_from(row.0)
        .ok()
        .filter(|sequence| *sequence != 0)
        .ok_or(AssetStoreError::StorageCorruption)?;
    let content_kind = ContentKind::new(row.1).map_err(|_| AssetStoreError::StorageCorruption)?;
    let custody = match row.2.as_str() {
        "MANAGED" => RevisionCustody::Managed,
        "UNMANAGED" => RevisionCustody::Unmanaged,
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    Ok(RevisionRow {
        sequence,
        content_kind,
        custody,
    })
}

fn read_parents(
    connection: &Connection,
    selects: &mut SelectBudget,
    asset_id: Id<Asset>,
    revision_id: Id<AssetRevision>,
) -> Result<Vec<Id<AssetRevision>>, AssetStoreError> {
    selects.consume(1)?;
    let mut statement = connection
        .prepare("SELECT ordinal, parent_revision_id FROM asset_revision_parents WHERE asset_id=?1 AND child_revision_id=?2 ORDER BY ordinal LIMIT 65")
        .map_err(sqlite)?;
    let mut rows = statement
        .query(params![
            asset_id.to_bytes().as_slice(),
            revision_id.to_bytes().as_slice()
        ])
        .map_err(sqlite)?;
    let mut parents = Vec::new();
    while let Some(row) = rows.next().map_err(sqlite)? {
        let ordinal = usize::try_from(row.get::<_, i64>(0).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        if ordinal != parents.len() || parents.len() == 64 {
            return Err(AssetStoreError::StorageCorruption);
        }
        parents.push(typed_id::<AssetRevision>(
            &row.get::<_, Vec<u8>>(1).map_err(sqlite)?,
        )?);
    }
    Ok(parents)
}

fn validate_representations(
    connection: &Connection,
    selects: &mut SelectBudget,
    revision_id: Id<AssetRevision>,
) -> Result<(), AssetStoreError> {
    selects.consume(1)?;
    let mut statement = connection
        .prepare(
            "SELECT representation_id FROM representations WHERE asset_revision_id=?1 ORDER BY representation_id LIMIT 65",
        )
        .map_err(sqlite)?;
    let rows = statement
        .query_map(params![revision_id.to_bytes().as_slice()], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .map_err(sqlite)?;
    let representations = rows
        .map(|row| {
            row.map_err(sqlite)
                .and_then(|bytes| typed_id::<Representation>(&bytes))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if representations.is_empty() || representations.len() > 64 {
        return Err(AssetStoreError::StorageCorruption);
    }
    Ok(())
}

fn first_resource(
    connection: &Connection,
    selects: &mut SelectBudget,
    representation_id: Id<Representation>,
) -> Result<Id<Resource>, AssetStoreError> {
    selects.consume(1)?;
    let mut statement = connection
        .prepare(
            "SELECT resource_id FROM resources WHERE representation_id=?1 ORDER BY resource_id LIMIT 65",
        )
        .map_err(sqlite)?;
    let rows = statement
        .query_map(params![representation_id.to_bytes().as_slice()], |row| {
            row.get::<_, Vec<u8>>(0)
        })
        .map_err(sqlite)?;
    let resources = rows
        .map(|row| {
            row.map_err(sqlite)
                .and_then(|bytes| typed_id::<Resource>(&bytes))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if resources.is_empty() || resources.len() > 64 {
        return Err(AssetStoreError::StorageCorruption);
    }
    Ok(resources[0])
}

#[derive(Clone)]
struct MemberRow {
    representation_id: Id<Representation>,
    representation_purpose: RepresentationPurpose,
    resource_id: Id<Resource>,
    resource_kind: ResourceKind,
    ordinal: u32,
    logical_name: LogicalName,
    blob_digest: Sha256Digest,
    byte_length: u64,
    media_type: Option<MediaType>,
    blob_revision: RevisionNo,
}

impl MemberRow {
    fn view(&self, location: Option<AssetLocationView>) -> AssetMemberView {
        AssetMemberView::__from_store(
            self.representation_id,
            self.representation_purpose.clone(),
            self.resource_id,
            self.resource_kind.clone(),
            self.ordinal,
            self.logical_name.clone(),
            self.blob_digest,
            self.byte_length,
            self.media_type.clone(),
            location,
        )
    }

    fn position(
        &self,
        library_id: [u8; 16],
        asset_id: Id<Asset>,
        selected_revision_id: Id<AssetRevision>,
        asset_revision: RevisionNo,
        phase: InspectMemberPhase,
        last_location_id: Option<Id<Location>>,
    ) -> Result<InspectAssetPosition, AssetStoreError> {
        InspectAssetPosition::new(
            library_id,
            asset_id,
            selected_revision_id,
            asset_revision,
            Some((self.representation_id, self.resource_id, self.ordinal)),
            Some(phase),
            Some(self.blob_revision),
            last_location_id,
        )
    }
}

fn read_member(
    connection: &Connection,
    selects: &mut SelectBudget,
    selected_revision_id: Id<AssetRevision>,
    key: (Id<Representation>, Id<Resource>, u32),
) -> Result<MemberRow, AssetStoreError> {
    selects.consume(1)?;
    let row = connection
        .query_row(
            "SELECT rp.purpose, rs.kind, rm.logical_name, rm.blob_digest, b.byte_length, b.media_type, b.lifecycle, b.revision FROM representations rp JOIN resources rs ON rs.representation_id=rp.representation_id JOIN resource_members rm ON rm.resource_id=rs.resource_id JOIN blobs b ON b.digest=rm.blob_digest WHERE rp.asset_revision_id=?1 AND rp.representation_id=?2 AND rs.resource_id=?3 AND rm.ordinal=?4",
            params![
                selected_revision_id.to_bytes().as_slice(),
                key.0.to_bytes().as_slice(),
                key.1.to_bytes().as_slice(),
                i64::from(key.2)
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Vec<u8>>(7)?,
                ))
            },
        )
        .optional()
        .map_err(sqlite)?
        .ok_or(AssetStoreError::Validation)?;
    if row.6 != "AVAILABLE" {
        return Err(AssetStoreError::StorageCorruption);
    }
    Ok(MemberRow {
        representation_id: key.0,
        representation_purpose: RepresentationPurpose::new(row.0)
            .map_err(|_| AssetStoreError::StorageCorruption)?,
        resource_id: key.1,
        resource_kind: ResourceKind::new(row.1).map_err(|_| AssetStoreError::StorageCorruption)?,
        ordinal: key.2,
        logical_name: LogicalName::new(row.2).map_err(|_| AssetStoreError::StorageCorruption)?,
        blob_digest: digest(&row.3)?,
        byte_length: u64::try_from(row.4)
            .ok()
            .filter(|value| *value <= 1_099_511_627_776)
            .ok_or(AssetStoreError::StorageCorruption)?,
        media_type: row
            .5
            .map(MediaType::new)
            .transpose()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
        blob_revision: parse_revision(Some(&row.7))?,
    })
}

fn first_member(
    connection: &Connection,
    selects: &mut SelectBudget,
    revision_id: Id<AssetRevision>,
) -> Result<Option<MemberRow>, AssetStoreError> {
    selects.consume(1)?;
    let representation = connection
        .query_row(
            "SELECT representation_id FROM representations WHERE asset_revision_id=?1 ORDER BY representation_id LIMIT 1",
            params![revision_id.to_bytes().as_slice()],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(sqlite)?;
    let Some(representation) = representation else {
        return Ok(None);
    };
    first_member_in_representation(connection, selects, revision_id, typed_id(&representation)?)
}

fn first_member_in_representation(
    connection: &Connection,
    selects: &mut SelectBudget,
    revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
) -> Result<Option<MemberRow>, AssetStoreError> {
    let resource = first_resource(connection, selects, representation_id)?;
    first_member_in_resource(
        connection,
        selects,
        revision_id,
        representation_id,
        resource,
    )
}

fn first_member_in_resource(
    connection: &Connection,
    selects: &mut SelectBudget,
    revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
) -> Result<Option<MemberRow>, AssetStoreError> {
    selects.consume(1)?;
    let ordinal = connection
        .query_row(
            "SELECT ordinal FROM resource_members WHERE resource_id=?1 ORDER BY ordinal LIMIT 1",
            params![resource_id.to_bytes().as_slice()],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(sqlite)?
        .ok_or(AssetStoreError::StorageCorruption)?;
    let ordinal = u32::try_from(ordinal).map_err(|_| AssetStoreError::StorageCorruption)?;
    if ordinal != 0 {
        return Err(AssetStoreError::StorageCorruption);
    }
    read_member(
        connection,
        selects,
        revision_id,
        (representation_id, resource_id, ordinal),
    )
    .map(Some)
}

fn next_member(
    connection: &Connection,
    selects: &mut SelectBudget,
    revision_id: Id<AssetRevision>,
    current: &MemberRow,
) -> Result<Option<MemberRow>, AssetStoreError> {
    selects.consume(1)?;
    if let Some(ordinal) = connection
        .query_row(
            "SELECT ordinal FROM resource_members WHERE resource_id=?1 AND ordinal>?2 ORDER BY ordinal LIMIT 1",
            params![
                current.resource_id.to_bytes().as_slice(),
                i64::from(current.ordinal)
            ],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(sqlite)?
    {
        let ordinal = u32::try_from(ordinal).map_err(|_| AssetStoreError::StorageCorruption)?;
        if ordinal != current.ordinal.checked_add(1).ok_or(AssetStoreError::StorageCorruption)? {
            return Err(AssetStoreError::StorageCorruption);
        }
        return read_member(
            connection,
            selects,
            revision_id,
            (
                current.representation_id,
                current.resource_id,
                ordinal,
            ),
        )
        .map(Some);
    }
    selects.consume(1)?;
    if let Some(resource) = connection
        .query_row(
            "SELECT resource_id FROM resources WHERE representation_id=?1 AND resource_id>?2 ORDER BY resource_id LIMIT 1",
            params![
                current.representation_id.to_bytes().as_slice(),
                current.resource_id.to_bytes().as_slice()
            ],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(sqlite)?
    {
        return first_member_in_resource(
            connection,
            selects,
            revision_id,
            current.representation_id,
            typed_id(&resource)?,
        );
    }
    selects.consume(1)?;
    if let Some(representation) = connection
        .query_row(
            "SELECT representation_id FROM representations WHERE asset_revision_id=?1 AND representation_id>?2 ORDER BY representation_id LIMIT 1",
            params![
                revision_id.to_bytes().as_slice(),
                current.representation_id.to_bytes().as_slice()
            ],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map_err(sqlite)?
    {
        return first_member_in_representation(
            connection,
            selects,
            revision_id,
            typed_id(&representation)?,
        );
    }
    Ok(None)
}

struct LocationRow {
    view: AssetLocationView,
    managed_durable: bool,
}

fn read_locations(
    connection: &Connection,
    selects: &mut SelectBudget,
    blob_digest: Sha256Digest,
    last_location_id: Option<Id<Location>>,
    limit: usize,
) -> Result<Vec<LocationRow>, AssetStoreError> {
    let mut output = Vec::with_capacity(limit);
    let limit = i64::try_from(limit).map_err(|_| AssetStoreError::Internal)?;
    if let Some(last_location_id) = last_location_id {
        selects.consume(1)?;
        let cursor = connection
            .query_row(
                "SELECT blob_digest, backend_id FROM locations WHERE location_id=?1",
                params![last_location_id.to_bytes().as_slice()],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(sqlite)?
            .ok_or(AssetStoreError::Validation)?;
        if digest(&cursor.0)? != blob_digest {
            return Err(AssetStoreError::Validation);
        }
        selects.consume(1)?;
        let mut statement = connection
            .prepare("SELECT location_id, custody, durability, lifecycle FROM locations WHERE blob_digest=?1 AND (backend_id, location_id)>(?2, ?3) ORDER BY backend_id, location_id LIMIT ?4")
            .map_err(sqlite)?;
        let rows = statement
            .query_map(
                params![
                    blob_digest.to_bytes().as_slice(),
                    cursor.1,
                    last_location_id.to_bytes().as_slice(),
                    limit
                ],
                location_row,
            )
            .map_err(sqlite)?;
        for row in rows {
            output.push(parse_location(row.map_err(sqlite)?)?);
        }
    } else {
        selects.consume(1)?;
        let mut statement = connection
            .prepare("SELECT location_id, custody, durability, lifecycle FROM locations WHERE blob_digest=?1 ORDER BY backend_id, location_id LIMIT ?2")
            .map_err(sqlite)?;
        let rows = statement
            .query_map(
                params![blob_digest.to_bytes().as_slice(), limit],
                location_row,
            )
            .map_err(sqlite)?;
        for row in rows {
            output.push(parse_location(row.map_err(sqlite)?)?);
        }
    }
    Ok(output)
}

fn location_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(Vec<u8>, String, String, String)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

fn parse_location(row: (Vec<u8>, String, String, String)) -> Result<LocationRow, AssetStoreError> {
    let custody = match row.1.as_str() {
        "MANAGED" => LocationCustody::Managed,
        "UNMANAGED" => LocationCustody::Unmanaged,
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    let durability = match row.2.as_str() {
        "DURABLE" => LocationDurability::Durable,
        "UNKNOWN" => LocationDurability::Unknown,
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    let lifecycle = match row.3.as_str() {
        "AVAILABLE" => LocationLifecycle::Available,
        "CORRUPT" => LocationLifecycle::Corrupt,
        "MISSING" => LocationLifecycle::Missing,
        "REMOVED" => LocationLifecycle::Removed,
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    Ok(LocationRow {
        view: AssetLocationView::__from_store(typed_id(&row.0)?, lifecycle, custody, durability),
        managed_durable: custody == LocationCustody::Managed
            && durability == LocationDurability::Durable,
    })
}

fn phase_after(current: InspectMemberPhase, location: &LocationRow) -> InspectMemberPhase {
    if current == InspectMemberPhase::RequiredCustodySeen || location.managed_durable {
        InspectMemberPhase::RequiredCustodySeen
    } else {
        InspectMemberPhase::LocationSeen
    }
}

fn registered_event(
    connection: &Connection,
    asset_id: Id<Asset>,
) -> Result<(u64, EventRow), AssetStoreError> {
    let mut statement = connection
        .prepare("SELECT commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, occurred_at_seconds, occurred_at_nanos FROM domain_events WHERE aggregate_kind='ASSET' AND aggregate_id=?1 AND event_type='asset.registered.v1' ORDER BY commit_sequence LIMIT 2")
        .map_err(sqlite)?;
    let mut rows = statement
        .query(params![asset_id.to_bytes().as_slice()])
        .map_err(sqlite)?;
    let row = rows
        .next()
        .map_err(sqlite)?
        .ok_or(AssetStoreError::NotFound)?;
    let sequence = valid_sequence(row.get::<_, i64>(0).map_err(sqlite)?)?;
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
    if rows.next().map_err(sqlite)?.is_some() {
        return Err(AssetStoreError::StorageCorruption);
    }
    Ok((sequence, event))
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
    if event.event_type != "asset.registered.v1"
        || event.schema_version != 1
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
    if !command_valid {
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

fn typed_id<T>(bytes: &[u8]) -> Result<Id<T>, AssetStoreError> {
    Id::from_bytes(
        bytes
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)
}

fn digest(bytes: &[u8]) -> Result<Sha256Digest, AssetStoreError> {
    Ok(Sha256Digest::from_bytes(
        bytes
            .try_into()
            .map_err(|_| AssetStoreError::StorageCorruption)?,
    ))
}

fn timestamp(seconds: i64, nanos: i64) -> Result<Timestamp, AssetStoreError> {
    Timestamp::from_unix_seconds_nanos(
        seconds,
        u32::try_from(nanos).map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)
}

fn sqlite(error: rusqlite::Error) -> AssetStoreError {
    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::OperationInterrupted) {
        AssetStoreError::OperationCancelled
    } else {
        map_store_error(map_reopen_error(error))
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
