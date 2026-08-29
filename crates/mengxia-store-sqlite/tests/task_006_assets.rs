use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use mengxia_domain::{
    Asset, AssetGraph, AssetKind, AssetRevision, ContentKind, CreateAssetRevisionValues, Location,
    LogicalName, MediaType, RegisterManagedAssetValues, Representation, RepresentationPurpose,
    Resource, ResourceKind, RevisionMember, RevisionRepresentation, RevisionResource,
};
use mengxia_events::{DomainEvent, ProvenanceEvent};
use mengxia_ports::{
    ASSET_INGEST_COPY_V1, ASSET_REVISION_CREATE_V1, AssetUnitOfWork, BLOB_LOCATION_RECORD_V1,
    Command, CommandBinding, CommandResult, CreateAssetRevisionCommand, DurableBlob,
    ExternalClaimOutcome, ExternalDisposition, ExternalDispositionOutcome, ExternalIngestClaim,
    ExternalIngestCompletion, ExternalIngestDisposition, ManagedRegistrationPlan, MutationOutcome,
    RecordManagedLocationCommand,
};
use mengxia_store_sqlite::{ConfigSource, OpenedLibrary, ResolvedStoreConfig};
use mengxia_types::{ErrorCode, Id, RevisionNo, Sha256Digest, Timestamp};
use rusqlite::Connection;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("target/task-006-asset-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
            .unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn config(&self) -> mengxia_store_sqlite::StoreConfig {
        ResolvedStoreConfig::from_selected(
            Some(self.root.join("Library")),
            ConfigSource::Cli,
            16,
            ConfigSource::CompiledDefault,
            1,
            ConfigSource::CompiledDefault,
            100,
            ConfigSource::CompiledDefault,
        )
        .validate()
        .unwrap()
    }

    fn database(&self) -> PathBuf {
        self.root.join("Library/library.sqlite3")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone, Copy)]
struct RegistrationIds {
    asset: Id<Asset>,
    revision: Id<AssetRevision>,
    representation: Id<Representation>,
    resource: Id<Resource>,
    location: Id<Location>,
    domain_event: Id<DomainEvent>,
    provenance_event: Id<ProvenanceEvent>,
}

fn ids() -> RegistrationIds {
    RegistrationIds {
        asset: Id::try_new().unwrap(),
        revision: Id::try_new().unwrap(),
        representation: Id::try_new().unwrap(),
        resource: Id::try_new().unwrap(),
        location: Id::try_new().unwrap(),
        domain_event: Id::try_new().unwrap(),
        provenance_event: Id::try_new().unwrap(),
    }
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix_seconds_nanos(seconds, 123_456_789).unwrap()
}

fn binding(command: Id<Command>, digest_byte: u8) -> CommandBinding {
    CommandBinding::new(
        command,
        ASSET_INGEST_COPY_V1,
        Sha256Digest::from_bytes([digest_byte; 32]),
    )
}

fn operation_binding(
    command: Id<Command>,
    operation: mengxia_ports::OperationId,
    digest_byte: u8,
) -> CommandBinding {
    CommandBinding::new(
        command,
        operation,
        Sha256Digest::from_bytes([digest_byte; 32]),
    )
}

fn completion(
    binding: CommandBinding,
    ids: RegistrationIds,
    blob_digest: Sha256Digest,
    backend: [u8; 32],
    completed_at: Timestamp,
) -> ExternalIngestCompletion {
    let blob = DurableBlob::__from_verified_local_adapter(blob_digest, 8192, backend);
    let plan = ManagedRegistrationPlan::new(
        ids.asset,
        AssetKind::new("image").unwrap(),
        ids.revision,
        ContentKind::new("raster").unwrap(),
        ids.representation,
        RepresentationPurpose::new("original").unwrap(),
        ids.resource,
        ResourceKind::new("file").unwrap(),
        LogicalName::new("original.png").unwrap(),
        Some(MediaType::new("image/png").unwrap()),
        ids.location,
    );
    ExternalIngestCompletion::new(
        binding,
        blob,
        plan,
        ids.domain_event,
        ids.provenance_event,
        completed_at,
    )
    .unwrap()
}

fn revision_command(
    binding: CommandBinding,
    registration_ids: RegistrationIds,
    blob_digest: Sha256Digest,
    domain_event_id: Id<DomainEvent>,
    provenance_event_id: Id<ProvenanceEvent>,
    operation_at: Timestamp,
) -> CreateAssetRevisionCommand {
    let initial_graph = AssetGraph::register_managed(RegisterManagedAssetValues {
        asset_id: registration_ids.asset,
        asset_kind: AssetKind::new("image").unwrap(),
        asset_revision_id: registration_ids.revision,
        content_kind: ContentKind::new("raster").unwrap(),
        representation_id: registration_ids.representation,
        representation_purpose: RepresentationPurpose::new("original").unwrap(),
        resource_id: registration_ids.resource,
        resource_kind: ResourceKind::new("file").unwrap(),
        logical_name: LogicalName::new("original.png").unwrap(),
        media_type: Some(MediaType::new("image/png").unwrap()),
        blob_digest,
        created_at: at(31),
    })
    .unwrap();
    let revision = initial_graph
        .asset()
        .create_revision(CreateAssetRevisionValues {
            expected_revision: RevisionNo::new(1),
            revision_id: Id::try_new().unwrap(),
            parent_revision_ids: vec![registration_ids.revision],
            content_kind: ContentKind::new("raster").unwrap(),
            representations: vec![
                RevisionRepresentation::new(
                    Id::try_new().unwrap(),
                    RepresentationPurpose::new("edited").unwrap(),
                    vec![
                        RevisionResource::new(
                            Id::try_new().unwrap(),
                            ResourceKind::new("file").unwrap(),
                            vec![RevisionMember::new(
                                LogicalName::new("edit.png").unwrap(),
                                blob_digest,
                            )],
                        )
                        .unwrap(),
                    ],
                )
                .unwrap(),
            ],
            created_at: operation_at,
        })
        .unwrap();
    CreateAssetRevisionCommand::new(
        binding,
        revision,
        domain_event_id,
        provenance_event_id,
        operation_at,
    )
    .unwrap()
}

async fn registered_fixture() -> (
    Fixture,
    mengxia_store_sqlite::StoreConfig,
    RegistrationIds,
    Sha256Digest,
) {
    let fixture = Fixture::new();
    let config = fixture.config();
    let registration_ids = ids();
    let blob_digest = Sha256Digest::from_bytes([0x79; 32]);
    let ingest_binding = binding(Id::try_new().unwrap(), 0x69);
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(ingest_binding, at(60)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Claimed
    );
    assert!(matches!(
        store
            .complete_external_ingest(completion(
                ingest_binding,
                registration_ids,
                blob_digest,
                [1; 32],
                at(61),
            ))
            .await
            .unwrap(),
        MutationOutcome::Applied(CommandResult::ManagedRegistration(_))
    ));
    drop(store);
    opened.shutdown().unwrap();
    (fixture, config, registration_ids, blob_digest)
}

#[tokio::test]
async fn external_registration_is_atomic_shared_and_exactly_replayable() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let blob_digest = Sha256Digest::from_bytes([0x41; 32]);
    let command_a = binding(Id::try_new().unwrap(), 0x11);
    let command_b = binding(Id::try_new().unwrap(), 0x12);
    let ids_a = ids();
    let ids_b = ids();

    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(command_a, at(10)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Claimed
    );
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(command_a, at(11)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::InProgress
    );
    assert_eq!(
        store
            .claim_external_ingest(
                ExternalIngestClaim::new(binding(command_a.command_id(), 0xfe), at(11),).unwrap()
            )
            .await,
        Err(mengxia_ports::AssetStoreError::Conflict),
        "a command ID collision must not disclose or reuse another request binding"
    );
    let first = store
        .complete_external_ingest(completion(command_a, ids_a, blob_digest, [1; 32], at(12)))
        .await
        .unwrap();
    let MutationOutcome::Applied(CommandResult::ManagedRegistration(first_result)) = first else {
        panic!("first registration must apply");
    };
    assert_eq!(first_result.asset_id(), ids_a.asset);
    assert_eq!(first_result.location_id(), ids_a.location);

    let replay = store
        .complete_external_ingest(completion(command_a, ids_a, blob_digest, [1; 32], at(13)))
        .await
        .unwrap();
    assert_eq!(
        replay,
        MutationOutcome::Replay(CommandResult::ManagedRegistration(first_result))
    );

    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(command_b, at(14)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Claimed
    );
    let second = store
        .complete_external_ingest(completion(command_b, ids_b, blob_digest, [1; 32], at(15)))
        .await
        .unwrap();
    let MutationOutcome::Applied(CommandResult::ManagedRegistration(second_result)) = second else {
        panic!("second registration must apply");
    };
    assert_ne!(first_result.asset_id(), second_result.asset_id());
    assert_eq!(first_result.blob_digest(), second_result.blob_digest());
    assert_eq!(first_result.location_id(), second_result.location_id());

    drop(store);
    opened.shutdown().unwrap();

    let connection = Connection::open(fixture.database()).unwrap();
    for (table, expected) in [
        ("assets", 2_i64),
        ("asset_revisions", 2),
        ("representations", 2),
        ("resources", 2),
        ("resource_members", 2),
        ("blobs", 1),
        ("locations", 1),
        ("commands", 2),
        ("domain_events", 2),
        ("provenance_events", 2),
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, expected, "table {table}");
    }
    assert!(
        connection
            .execute("UPDATE domain_events SET schema_version=1", [])
            .is_err(),
        "domain events must remain append-only"
    );
    assert!(
        connection
            .execute("DELETE FROM provenance_events", [])
            .is_err(),
        "provenance events must remain append-only"
    );
    drop(connection);

    let reopened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = reopened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(command_a, at(16)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Replay(CommandResult::ManagedRegistration(first_result))
    );
    drop(store);
    reopened.shutdown().unwrap();
}

#[tokio::test]
async fn stale_claim_requires_recovery_and_terminal_disposition_replays_after_restart() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let stale_binding = binding(Id::try_new().unwrap(), 0x21);
    let rejected_binding = binding(Id::try_new().unwrap(), 0x22);

    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(stale_binding, at(20)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Claimed
    );
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(rejected_binding, at(21)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Claimed
    );
    let disposition = ExternalIngestDisposition::new(
        rejected_binding,
        ExternalDisposition::TerminalRejected(ErrorCode::OperationCancelled),
        at(22),
    )
    .unwrap();
    assert_eq!(
        store.finish_external_ingest(disposition).await.unwrap(),
        ExternalDispositionOutcome::Stored
    );
    drop(store);
    opened.shutdown().unwrap();

    let reopened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = reopened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(stale_binding, at(23)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::RecoveryRequired {
            safe_error_code: ErrorCode::StorageConfigurationError,
        }
    );
    assert_eq!(
        store
            .finish_external_ingest(
                ExternalIngestDisposition::new(
                    rejected_binding,
                    ExternalDisposition::TerminalRejected(ErrorCode::OperationCancelled),
                    at(24),
                )
                .unwrap(),
            )
            .await
            .unwrap(),
        ExternalDispositionOutcome::Replay {
            safe_error_code: ErrorCode::OperationCancelled,
        }
    );
    drop(store);
    reopened.shutdown().unwrap();
}

#[tokio::test]
async fn concurrent_duplicate_claim_has_one_durable_owner() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let binding = binding(Id::try_new().unwrap(), 0x29);
    let first = store.claim_external_ingest(ExternalIngestClaim::new(binding, at(25)).unwrap());
    let second = store.claim_external_ingest(ExternalIngestClaim::new(binding, at(25)).unwrap());
    let (first, second) = tokio::join!(first, second);
    let outcomes = [first.unwrap(), second.unwrap()];
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == ExternalClaimOutcome::Claimed)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|outcome| **outcome == ExternalClaimOutcome::InProgress)
            .count(),
        1
    );
    drop(store);
    opened.shutdown().unwrap();
}

#[tokio::test]
async fn concurrent_pure_duplicate_mutates_once_and_stale_revision_rejects_replayably() {
    let (_fixture, config, _, blob_digest) = registered_fixture().await;
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let duplicate_binding =
        operation_binding(Id::try_new().unwrap(), BLOB_LOCATION_RECORD_V1, 0x2a);
    let location_id = Id::<Location>::try_new().unwrap();
    let event_id = Id::<DomainEvent>::try_new().unwrap();
    let duplicate = || {
        RecordManagedLocationCommand::new(
            duplicate_binding,
            DurableBlob::__from_verified_local_adapter(blob_digest, 8192, [2; 32]),
            location_id,
            RevisionNo::new(1),
            event_id,
            at(26),
        )
        .unwrap()
    };
    let (first, second) = tokio::join!(
        store.execute_record_location(duplicate()),
        store.execute_record_location(duplicate())
    );
    let outcomes = [first.unwrap(), second.unwrap()];
    let applied = outcomes
        .iter()
        .find_map(|outcome| match outcome {
            MutationOutcome::Applied(CommandResult::Location(result)) => Some(*result),
            _ => None,
        })
        .expect("one pure command applies");
    assert!(
        outcomes.iter().any(|outcome| {
            *outcome == MutationOutcome::Replay(CommandResult::Location(applied))
        })
    );

    let stale_binding = operation_binding(Id::try_new().unwrap(), BLOB_LOCATION_RECORD_V1, 0x2b);
    let stale = || {
        RecordManagedLocationCommand::new(
            stale_binding,
            DurableBlob::__from_verified_local_adapter(blob_digest, 8192, [3; 32]),
            Id::try_new().unwrap(),
            RevisionNo::new(1),
            Id::try_new().unwrap(),
            at(27),
        )
        .unwrap()
    };
    let rejected = MutationOutcome::TerminalRejected {
        safe_error_code: ErrorCode::Conflict,
    };
    assert_eq!(
        store.execute_record_location(stale()).await.unwrap(),
        rejected
    );
    assert_eq!(
        store.execute_record_location(stale()).await.unwrap(),
        rejected
    );
    drop(store);
    opened.shutdown().unwrap();
}

#[tokio::test]
async fn different_command_reusing_location_descriptor_is_terminal_conflict_without_domain_change()
{
    let (fixture, config, _, blob_digest) = registered_fixture().await;
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let conflicting_binding =
        operation_binding(Id::try_new().unwrap(), BLOB_LOCATION_RECORD_V1, 0x2c);
    let conflicting = || {
        RecordManagedLocationCommand::new(
            conflicting_binding,
            DurableBlob::__from_verified_local_adapter(blob_digest, 8192, [1; 32]),
            Id::try_new().unwrap(),
            RevisionNo::new(1),
            Id::try_new().unwrap(),
            at(28),
        )
        .unwrap()
    };
    let rejected = MutationOutcome::TerminalRejected {
        safe_error_code: ErrorCode::Conflict,
    };
    assert_eq!(
        store.execute_record_location(conflicting()).await.unwrap(),
        rejected
    );
    assert_eq!(
        store.execute_record_location(conflicting()).await.unwrap(),
        rejected
    );
    drop(store);
    opened.shutdown().unwrap();

    let connection = Connection::open(fixture.database()).unwrap();
    let state: (i64, i64, i64, Vec<u8>, String) = connection
        .query_row(
            "SELECT (SELECT count(*) FROM locations), (SELECT count(*) FROM domain_events), (SELECT last_sequence FROM event_commit_sequence WHERE singleton=1), revision, (SELECT state FROM commands WHERE command_id=?2) FROM blobs WHERE digest=?1",
            rusqlite::params![blob_digest.to_bytes().as_slice(), conflicting_binding.command_id().to_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        state,
        (
            1,
            1,
            1,
            1_u64.to_be_bytes().to_vec(),
            "TERMINAL_REJECTED".to_owned()
        )
    );
}

#[tokio::test]
async fn location_and_creative_revision_use_expected_revision_and_atomic_events() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let blob_digest = Sha256Digest::from_bytes([0x51; 32]);
    let ingest_binding = binding(Id::try_new().unwrap(), 0x31);
    let registration_ids = ids();
    store
        .claim_external_ingest(ExternalIngestClaim::new(ingest_binding, at(30)).unwrap())
        .await
        .unwrap();
    let registered = store
        .complete_external_ingest(completion(
            ingest_binding,
            registration_ids,
            blob_digest,
            [1; 32],
            at(31),
        ))
        .await
        .unwrap();
    let MutationOutcome::Applied(CommandResult::ManagedRegistration(registered)) = registered
    else {
        panic!("registration must apply");
    };

    let location_binding = operation_binding(Id::try_new().unwrap(), BLOB_LOCATION_RECORD_V1, 0x32);
    let second_location = Id::<Location>::try_new().unwrap();
    let location_command = || {
        RecordManagedLocationCommand::new(
            location_binding,
            DurableBlob::__from_verified_local_adapter(blob_digest, 8192, [2; 32]),
            second_location,
            RevisionNo::new(1),
            Id::try_new().unwrap(),
            at(32),
        )
        .unwrap()
    };
    let location = store
        .execute_record_location(location_command())
        .await
        .unwrap();
    let MutationOutcome::Applied(CommandResult::Location(location_result)) = location else {
        panic!("location mutation must apply");
    };
    assert_eq!(location_result.location_id(), second_location);
    assert_eq!(location_result.revision(), RevisionNo::new(2));
    assert_eq!(
        store
            .execute_record_location(location_command())
            .await
            .unwrap(),
        MutationOutcome::Replay(CommandResult::Location(location_result))
    );
    let later_location_binding =
        operation_binding(Id::try_new().unwrap(), BLOB_LOCATION_RECORD_V1, 0x34);
    let later_location = store
        .execute_record_location(
            RecordManagedLocationCommand::new(
                later_location_binding,
                DurableBlob::__from_verified_local_adapter(blob_digest, 8192, [3; 32]),
                Id::try_new().unwrap(),
                RevisionNo::new(2),
                Id::try_new().unwrap(),
                at(33),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let MutationOutcome::Applied(CommandResult::Location(later_location)) = later_location else {
        panic!("later location mutation must apply");
    };
    assert_eq!(later_location.revision(), RevisionNo::new(3));
    assert_eq!(
        store
            .execute_record_location(location_command())
            .await
            .unwrap(),
        MutationOutcome::Replay(CommandResult::Location(location_result)),
        "replay must return the immutable original result, not current Blob revision"
    );
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(ingest_binding, at(33)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Replay(CommandResult::ManagedRegistration(registered))
    );
    assert_ne!(registered.location_id(), second_location);

    let initial_graph = AssetGraph::register_managed(RegisterManagedAssetValues {
        asset_id: registration_ids.asset,
        asset_kind: AssetKind::new("image").unwrap(),
        asset_revision_id: registration_ids.revision,
        content_kind: ContentKind::new("raster").unwrap(),
        representation_id: registration_ids.representation,
        representation_purpose: RepresentationPurpose::new("original").unwrap(),
        resource_id: registration_ids.resource,
        resource_kind: ResourceKind::new("file").unwrap(),
        logical_name: LogicalName::new("original.png").unwrap(),
        media_type: Some(MediaType::new("image/png").unwrap()),
        blob_digest,
        created_at: at(31),
    })
    .unwrap();
    let new_revision_id = Id::<AssetRevision>::try_new().unwrap();
    let new_representation_id = Id::<Representation>::try_new().unwrap();
    let new_resource_id = Id::<Resource>::try_new().unwrap();
    let make_revision = || {
        initial_graph
            .asset()
            .create_revision(CreateAssetRevisionValues {
                expected_revision: RevisionNo::new(1),
                revision_id: new_revision_id,
                parent_revision_ids: vec![registration_ids.revision],
                content_kind: ContentKind::new("raster").unwrap(),
                representations: vec![
                    RevisionRepresentation::new(
                        new_representation_id,
                        RepresentationPurpose::new("edited").unwrap(),
                        vec![
                            RevisionResource::new(
                                new_resource_id,
                                ResourceKind::new("file").unwrap(),
                                vec![RevisionMember::new(
                                    LogicalName::new("edit.png").unwrap(),
                                    blob_digest,
                                )],
                            )
                            .unwrap(),
                        ],
                    )
                    .unwrap(),
                ],
                created_at: at(34),
            })
            .unwrap()
    };
    let revision_binding =
        operation_binding(Id::try_new().unwrap(), ASSET_REVISION_CREATE_V1, 0x33);
    let revision_command = || {
        CreateAssetRevisionCommand::new(
            revision_binding,
            make_revision(),
            Id::try_new().unwrap(),
            Id::try_new().unwrap(),
            at(34),
        )
        .unwrap()
    };
    let cross_operation_collision = CreateAssetRevisionCommand::new(
        operation_binding(ingest_binding.command_id(), ASSET_REVISION_CREATE_V1, 0x35),
        make_revision(),
        Id::try_new().unwrap(),
        Id::try_new().unwrap(),
        at(34),
    )
    .unwrap();
    assert_eq!(
        store
            .execute_create_revision(cross_operation_collision)
            .await,
        Err(mengxia_ports::AssetStoreError::Conflict),
        "operation mismatch on an existing command ID is a nondisclosing collision"
    );
    let revision = store
        .execute_create_revision(revision_command())
        .await
        .unwrap();
    let MutationOutcome::Applied(CommandResult::AssetRevision(revision_result)) = revision else {
        panic!("creative revision must apply");
    };
    assert_eq!(revision_result.asset_id(), registration_ids.asset);
    assert_eq!(revision_result.revision(), RevisionNo::new(2));
    assert_eq!(
        store
            .execute_create_revision(revision_command())
            .await
            .unwrap(),
        MutationOutcome::Replay(CommandResult::AssetRevision(revision_result))
    );

    drop(store);
    opened.shutdown().unwrap();
    let connection = Connection::open(fixture.database()).unwrap();
    let counts: (i64, i64, i64, i64) = connection
        .query_row(
            "SELECT (SELECT count(*) FROM asset_revisions), (SELECT count(*) FROM locations), (SELECT count(*) FROM domain_events), (SELECT count(*) FROM provenance_events)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(counts, (2, 3, 4, 2));
}

#[test]
fn current_schema_tamper_and_newer_prefixes_fail_closed_by_class() {
    let tampered = Fixture::new();
    let config = tampered.config();
    OpenedLibrary::open_or_bootstrap(&config)
        .unwrap()
        .shutdown()
        .unwrap();
    let connection = Connection::open(tampered.database()).unwrap();
    connection
        .execute_batch("CREATE VIEW forbidden_extra_view AS SELECT asset_id FROM assets")
        .unwrap();
    drop(connection);
    assert_eq!(
        OpenedLibrary::open_or_bootstrap(&config).err(),
        Some(mengxia_store_sqlite::StoreError::Corruption)
    );

    let newer = Fixture::new();
    let config = newer.config();
    OpenedLibrary::open_or_bootstrap(&config)
        .unwrap()
        .shutdown()
        .unwrap();
    let connection = Connection::open(newer.database()).unwrap();
    connection
        .execute("INSERT INTO schema_migrations (migration_sequence, migration_name, sha256, applied_at_seconds, applied_at_nanos) VALUES (2, '0002_future', zeroblob(32), 1700000000, 0)", [])
        .unwrap();
    drop(connection);
    assert_eq!(
        OpenedLibrary::open_or_bootstrap(&config).err(),
        Some(mengxia_store_sqlite::StoreError::Configuration)
    );

    let malformed = Fixture::new();
    let config = malformed.config();
    OpenedLibrary::open_or_bootstrap(&config)
        .unwrap()
        .shutdown()
        .unwrap();
    let connection = Connection::open(malformed.database()).unwrap();
    connection
        .execute("INSERT INTO schema_migrations (migration_sequence, migration_name, sha256, applied_at_seconds, applied_at_nanos) VALUES (2, '../future', zeroblob(32), 1700000000, 0)", [])
        .unwrap();
    drop(connection);
    assert_eq!(
        OpenedLibrary::open_or_bootstrap(&config).err(),
        Some(mengxia_store_sqlite::StoreError::Corruption)
    );

    let invalid_timestamp = Fixture::new();
    let config = invalid_timestamp.config();
    OpenedLibrary::open_or_bootstrap(&config)
        .unwrap()
        .shutdown()
        .unwrap();
    let connection = Connection::open(invalid_timestamp.database()).unwrap();
    connection
        .execute(
            "UPDATE schema_migrations SET applied_at_seconds=253402300800 WHERE migration_sequence=1",
            [],
        )
        .unwrap();
    drop(connection);
    assert_eq!(
        OpenedLibrary::open_or_bootstrap(&config).err(),
        Some(mengxia_store_sqlite::StoreError::Corruption)
    );
}

#[tokio::test]
async fn event_sequence_exhaustion_commits_only_replayable_terminal_rejection() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let blob_digest = Sha256Digest::from_bytes([0x61; 32]);
    let ingest_binding = binding(Id::try_new().unwrap(), 0x41);
    let registration_ids = ids();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    store
        .claim_external_ingest(ExternalIngestClaim::new(ingest_binding, at(40)).unwrap())
        .await
        .unwrap();
    store
        .complete_external_ingest(completion(
            ingest_binding,
            registration_ids,
            blob_digest,
            [1; 32],
            at(41),
        ))
        .await
        .unwrap();
    drop(store);
    opened.shutdown().unwrap();

    let connection = Connection::open(fixture.database()).unwrap();
    connection
        .execute(
            "UPDATE event_commit_sequence SET last_sequence=9223372036854775807 WHERE singleton=1",
            [],
        )
        .unwrap();
    drop(connection);

    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let location_binding = operation_binding(Id::try_new().unwrap(), BLOB_LOCATION_RECORD_V1, 0x42);
    let candidate = Id::<Location>::try_new().unwrap();
    let command = || {
        RecordManagedLocationCommand::new(
            location_binding,
            DurableBlob::__from_verified_local_adapter(blob_digest, 8192, [2; 32]),
            candidate,
            RevisionNo::new(1),
            Id::try_new().unwrap(),
            at(42),
        )
        .unwrap()
    };
    let expected = MutationOutcome::TerminalRejected {
        safe_error_code: ErrorCode::RevisionExhausted,
    };
    assert_eq!(
        store.execute_record_location(command()).await.unwrap(),
        expected
    );
    assert_eq!(
        store.execute_record_location(command()).await.unwrap(),
        expected
    );
    drop(store);
    opened.shutdown().unwrap();

    let connection = Connection::open(fixture.database()).unwrap();
    let counts: (i64, i64, i64, Vec<u8>) = connection
        .query_row(
            "SELECT (SELECT count(*) FROM locations), (SELECT count(*) FROM domain_events), (SELECT count(*) FROM commands), (SELECT revision FROM blobs WHERE digest=?1)",
            [blob_digest.to_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(counts.0, 1);
    assert_eq!(counts.1, 1);
    assert_eq!(counts.2, 2);
    assert_eq!(counts.3, 1_u64.to_be_bytes());
}

#[tokio::test]
async fn pure_statement_failure_rolls_back_command_state_and_events_and_fails_runtime() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let blob_digest = Sha256Digest::from_bytes([0x71; 32]);
    let ingest_binding = binding(Id::try_new().unwrap(), 0x51);
    let registration_ids = ids();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    store
        .claim_external_ingest(ExternalIngestClaim::new(ingest_binding, at(50)).unwrap())
        .await
        .unwrap();
    store
        .complete_external_ingest(completion(
            ingest_binding,
            registration_ids,
            blob_digest,
            [1; 32],
            at(51),
        ))
        .await
        .unwrap();

    let command = revision_command(
        operation_binding(Id::try_new().unwrap(), ASSET_REVISION_CREATE_V1, 0x52),
        registration_ids,
        blob_digest,
        registration_ids.domain_event,
        Id::try_new().unwrap(),
        at(52),
    );
    assert_eq!(
        store.execute_create_revision(command).await,
        Err(mengxia_ports::AssetStoreError::Internal)
    );
    drop(store);
    assert_eq!(
        opened.shutdown(),
        Err(mengxia_store_sqlite::StoreError::Internal)
    );

    let connection = Connection::open(fixture.database()).unwrap();
    let state: (Vec<u8>, i64, i64, i64, i64) = connection
        .query_row(
            "SELECT revision, (SELECT count(*) FROM asset_revisions), (SELECT count(*) FROM commands), (SELECT count(*) FROM domain_events), (SELECT count(*) FROM provenance_events) FROM assets WHERE asset_id=?1",
            [registration_ids.asset.to_bytes().as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .unwrap();
    assert_eq!(state, (1_u64.to_be_bytes().to_vec(), 1, 1, 1, 1));
}

#[tokio::test]
async fn materialized_row_corruption_is_not_downgraded_to_not_found_or_conflict() {
    let (sequence_fixture, config, registration_ids, blob_digest) = registered_fixture().await;
    let connection = Connection::open(sequence_fixture.database()).unwrap();
    connection
        .execute(
            "UPDATE asset_revisions SET sequence=2 WHERE asset_revision_id=?1",
            [registration_ids.revision.to_bytes().as_slice()],
        )
        .unwrap();
    drop(connection);
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .execute_create_revision(revision_command(
                operation_binding(Id::try_new().unwrap(), ASSET_REVISION_CREATE_V1, 0x6a),
                registration_ids,
                blob_digest,
                Id::try_new().unwrap(),
                Id::try_new().unwrap(),
                at(62),
            ))
            .await,
        Err(mengxia_ports::AssetStoreError::StorageCorruption)
    );
    assert_eq!(
        store
            .execute_create_revision(revision_command(
                operation_binding(Id::try_new().unwrap(), ASSET_REVISION_CREATE_V1, 0x6d),
                registration_ids,
                blob_digest,
                Id::try_new().unwrap(),
                Id::try_new().unwrap(),
                at(62),
            ))
            .await,
        Err(mengxia_ports::AssetStoreError::Internal),
        "corruption must close later admission"
    );
    drop(store);
    assert_eq!(
        opened.shutdown(),
        Err(mengxia_store_sqlite::StoreError::Internal)
    );

    let (custody_fixture, config, registration_ids, blob_digest) = registered_fixture().await;
    let connection = Connection::open(custody_fixture.database()).unwrap();
    connection
        .execute(
            "UPDATE locations SET custody='UNMANAGED' WHERE blob_digest=?1",
            [blob_digest.to_bytes().as_slice()],
        )
        .unwrap();
    drop(connection);
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .execute_create_revision(revision_command(
                operation_binding(Id::try_new().unwrap(), ASSET_REVISION_CREATE_V1, 0x6b),
                registration_ids,
                blob_digest,
                Id::try_new().unwrap(),
                Id::try_new().unwrap(),
                at(63),
            ))
            .await,
        Err(mengxia_ports::AssetStoreError::StorageCorruption)
    );
    drop(store);
    assert_eq!(
        opened.shutdown(),
        Err(mengxia_store_sqlite::StoreError::Internal)
    );

    let (timestamp_fixture, config, _, blob_digest) = registered_fixture().await;
    let connection = Connection::open(timestamp_fixture.database()).unwrap();
    connection
        .execute(
            "UPDATE blobs SET verified_at_seconds=253402300800 WHERE digest=?1",
            [blob_digest.to_bytes().as_slice()],
        )
        .unwrap();
    drop(connection);
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .execute_record_location(
                RecordManagedLocationCommand::new(
                    operation_binding(Id::try_new().unwrap(), BLOB_LOCATION_RECORD_V1, 0x6c),
                    DurableBlob::__from_verified_local_adapter(blob_digest, 8192, [1; 32]),
                    Id::try_new().unwrap(),
                    RevisionNo::new(1),
                    Id::try_new().unwrap(),
                    at(64),
                )
                .unwrap(),
            )
            .await,
        Err(mengxia_ports::AssetStoreError::StorageCorruption)
    );
    drop(store);
    assert_eq!(
        opened.shutdown(),
        Err(mengxia_store_sqlite::StoreError::Internal)
    );
}

async fn assert_completed_command_tamper_fails_closed(update: &str) {
    let (fixture, config, _, _) = registered_fixture().await;
    let connection = Connection::open(fixture.database()).unwrap();
    let command_bytes: Vec<u8> = connection
        .query_row("SELECT command_id FROM commands", [], |row| row.get(0))
        .unwrap();
    connection.execute_batch(update).unwrap();
    drop(connection);
    let command = Id::<Command>::from_bytes(command_bytes.try_into().unwrap()).unwrap();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(
                ExternalIngestClaim::new(binding(command, 0x69), at(70)).unwrap()
            )
            .await,
        Err(mengxia_ports::AssetStoreError::StorageCorruption)
    );
    drop(store);
    assert_eq!(
        opened.shutdown(),
        Err(mengxia_store_sqlite::StoreError::Internal)
    );
}

#[tokio::test]
async fn command_record_typed_mapping_and_operation_matrix_fail_closed() {
    assert_completed_command_tamper_fails_closed(
        "UPDATE commands SET store_runtime_id=zeroblob(16)",
    )
    .await;
    assert_completed_command_tamper_fails_closed(
        "UPDATE commands SET result_kind='LOCATION', result_id=result_location_id, result_location_id=NULL",
    )
    .await;
    assert_completed_command_tamper_fails_closed("UPDATE commands SET result_id=zeroblob(16)")
        .await;
    assert_completed_command_tamper_fails_closed(
        "UPDATE commands SET updated_at_seconds=253402300800",
    )
    .await;

    let (pure_fixture, pure_config, registration_ids, blob_digest) = registered_fixture().await;
    let connection = Connection::open(pure_fixture.database()).unwrap();
    let command_bytes: Vec<u8> = connection
        .query_row("SELECT command_id FROM commands", [], |row| row.get(0))
        .unwrap();
    connection
        .execute(
            "UPDATE commands SET operation_id='asset.revision.create.v1', state='CLAIMED', result_kind=NULL, result_id=NULL, result_location_id=NULL",
            [],
        )
        .unwrap();
    drop(connection);
    let command = Id::<Command>::from_bytes(command_bytes.try_into().unwrap()).unwrap();
    let opened = OpenedLibrary::open_or_bootstrap(&pure_config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .execute_create_revision(revision_command(
                operation_binding(command, ASSET_REVISION_CREATE_V1, 0x69),
                registration_ids,
                blob_digest,
                Id::try_new().unwrap(),
                Id::try_new().unwrap(),
                at(70),
            ))
            .await,
        Err(mengxia_ports::AssetStoreError::StorageCorruption)
    );
    drop(store);
    assert_eq!(
        opened.shutdown(),
        Err(mengxia_store_sqlite::StoreError::Internal)
    );

    let (fixture, config, _, _) = registered_fixture().await;
    let connection = Connection::open(fixture.database()).unwrap();
    let command_bytes: Vec<u8> = connection
        .query_row("SELECT command_id FROM commands", [], |row| row.get(0))
        .unwrap();
    connection
        .execute("UPDATE commands SET store_runtime_id=zeroblob(16)", [])
        .unwrap();
    drop(connection);
    let command = Id::<Command>::from_bytes(command_bytes.try_into().unwrap()).unwrap();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(
                ExternalIngestClaim::new(binding(command, 0x70), at(70)).unwrap()
            )
            .await,
        Err(mengxia_ports::AssetStoreError::Conflict),
        "binding mismatch takes precedence without disclosing malformed stored outcome state"
    );
    drop(store);
    opened.shutdown().unwrap();
}

#[tokio::test]
async fn external_terminal_code_outside_operation_allowlist_is_corruption() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let command_binding = binding(Id::try_new().unwrap(), 0x73);
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(command_binding, at(71)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Claimed
    );
    assert_eq!(
        store
            .finish_external_ingest(
                ExternalIngestDisposition::new(
                    command_binding,
                    ExternalDisposition::TerminalRejected(ErrorCode::OperationCancelled),
                    at(72),
                )
                .unwrap()
            )
            .await
            .unwrap(),
        ExternalDispositionOutcome::Stored
    );
    drop(store);
    opened.shutdown().unwrap();
    let connection = Connection::open(fixture.database()).unwrap();
    connection
        .execute("UPDATE commands SET safe_error_code='NOT_FOUND'", [])
        .unwrap();
    drop(connection);

    let reopened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = reopened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(command_binding, at(73)).unwrap())
            .await,
        Err(mengxia_ports::AssetStoreError::StorageCorruption)
    );
    drop(store);
    assert_eq!(
        reopened.shutdown(),
        Err(mengxia_store_sqlite::StoreError::Internal)
    );
}

#[tokio::test]
async fn registration_replay_rejects_non_exact_materialized_graph() {
    let (fixture, config, registration_ids, _) = registered_fixture().await;
    let connection = Connection::open(fixture.database()).unwrap();
    let command_bytes: Vec<u8> = connection
        .query_row("SELECT command_id FROM commands", [], |row| row.get(0))
        .unwrap();
    connection
        .execute(
            "INSERT INTO representations (representation_id, asset_revision_id, purpose) VALUES (?1, ?2, 'thumbnail')",
            rusqlite::params![Id::<Representation>::try_new().unwrap().to_bytes().as_slice(), registration_ids.revision.to_bytes().as_slice()],
        )
        .unwrap();
    drop(connection);
    let command = Id::<Command>::from_bytes(command_bytes.try_into().unwrap()).unwrap();

    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    assert_eq!(
        store
            .claim_external_ingest(
                ExternalIngestClaim::new(binding(command, 0x69), at(74)).unwrap()
            )
            .await,
        Err(mengxia_ports::AssetStoreError::StorageCorruption)
    );
    drop(store);
    assert_eq!(
        opened.shutdown(),
        Err(mengxia_store_sqlite::StoreError::Internal)
    );
}
