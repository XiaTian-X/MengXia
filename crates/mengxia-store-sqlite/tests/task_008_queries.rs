use std::fs;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use mengxia_domain::{
    Asset, AssetKind, AssetRevision, ContentKind, Location, LogicalName, Representation,
    RepresentationPurpose, Resource, ResourceKind,
};
use mengxia_events::{DomainEvent, ProvenanceEvent};
use mengxia_ports::{
    ASSET_INGEST_COPY_V1, AssetQueryPort as _, AssetStoreError, AssetUnitOfWork as _, Command,
    CommandBinding, DurableBlob, ExternalClaimOutcome, ExternalIngestClaim,
    ExternalIngestCompletion, InspectAssetQuery, InspectAssetStart, ListAssetsPosition,
    ListAssetsQuery, ManagedRegistrationPlan, MaterializationSelection, MutationOutcome,
    VerificationScanPosition, VerificationStorePort as _,
};
use mengxia_store_sqlite::{ConfigSource, OpenedLibrary, ResolvedStoreConfig};
use mengxia_types::{Id, Sha256Digest, Timestamp};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("store crate belongs to the workspace")
            .join("target/task-008-query-tests")
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

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix_seconds_nanos(seconds, 123_456_789).unwrap()
}

async fn register(store: &impl mengxia_ports::AssetUnitOfWork, index: u8) -> Registered {
    let command_id = Id::<Command>::try_new().unwrap();
    let binding = CommandBinding::new(
        command_id,
        ASSET_INGEST_COPY_V1,
        Sha256Digest::from_bytes([index; 32]),
    );
    assert_eq!(
        store
            .claim_external_ingest(
                ExternalIngestClaim::new(binding, at(100 + i64::from(index))).unwrap()
            )
            .await
            .unwrap(),
        ExternalClaimOutcome::Claimed
    );
    let asset_id = Id::<Asset>::try_new().unwrap();
    let revision_id = Id::<AssetRevision>::try_new().unwrap();
    let representation_id = Id::<Representation>::try_new().unwrap();
    let resource_id = Id::<Resource>::try_new().unwrap();
    let plan = ManagedRegistrationPlan::new(
        asset_id,
        AssetKind::new("image").unwrap(),
        revision_id,
        ContentKind::new("raster").unwrap(),
        representation_id,
        RepresentationPurpose::new("original").unwrap(),
        resource_id,
        ResourceKind::new("file").unwrap(),
        LogicalName::new(format!("asset-{index}.png")).unwrap(),
        None,
        Id::<Location>::try_new().unwrap(),
    );
    let completion = ExternalIngestCompletion::new(
        binding,
        DurableBlob::__from_verified_local_adapter(
            Sha256Digest::from_bytes([0x80 | index; 32]),
            u64::from(index),
            [0x55; 32],
        ),
        plan,
        Id::<DomainEvent>::try_new().unwrap(),
        Id::<ProvenanceEvent>::try_new().unwrap(),
        at(100 + i64::from(index)),
    )
    .unwrap();
    assert!(matches!(
        store.complete_external_ingest(completion).await.unwrap(),
        MutationOutcome::Applied(_)
    ));
    Registered {
        asset_id,
        revision_id,
        representation_id,
        resource_id,
    }
}

struct Registered {
    asset_id: Id<Asset>,
    revision_id: Id<AssetRevision>,
    representation_id: Id<Representation>,
    resource_id: Id<Resource>,
}

#[tokio::test]
async fn list_assets_is_snapshot_bounded_and_uses_actual_last_examined_event() {
    let fixture = Fixture::new();
    let opened = OpenedLibrary::open_or_bootstrap(&fixture.config()).unwrap();
    let store = opened.asset_store_handle();

    let empty = store
        .list_assets(ListAssetsQuery::new(64, ListAssetsPosition::First).unwrap())
        .await
        .unwrap();
    assert_eq!(empty.snapshot_sequence(), 0);
    assert!(empty.assets().is_empty());
    assert_eq!(empty.next(), None);

    let first_registration = register(&store, 1).await;
    let second_registration = register(&store, 2).await;
    let first_id = first_registration.asset_id;
    let second_id = second_registration.asset_id;
    let first = store
        .list_assets(ListAssetsQuery::new(1, ListAssetsPosition::First).unwrap())
        .await
        .unwrap();
    assert_eq!(first.snapshot_sequence(), 2);
    assert_eq!(first.assets().len(), 1);
    assert_eq!(first.assets()[0].asset_id(), first_id);
    assert_eq!(first.assets()[0].creation_commit_sequence(), 1);
    let cursor = first
        .next()
        .expect("the second Asset remains in the snapshot");

    let second = store
        .list_assets(ListAssetsQuery::new(1, cursor).unwrap())
        .await
        .unwrap();
    assert_eq!(second.snapshot_sequence(), first.snapshot_sequence());
    assert_eq!(second.assets().len(), 1);
    assert_eq!(second.assets()[0].asset_id(), second_id);
    assert_eq!(second.assets()[0].creation_commit_sequence(), 2);
    assert_eq!(second.next(), None);

    opened.shutdown().unwrap();
}

#[test]
fn list_query_rejects_invalid_page_and_cursor_ranges() {
    assert!(ListAssetsQuery::new(0, ListAssetsPosition::First).is_err());
    assert!(ListAssetsQuery::new(65, ListAssetsPosition::First).is_err());
    assert!(ListAssetsPosition::after([1; 16], 1, 1).is_err());
    assert!(ListAssetsPosition::after([1; 16], i64::MAX as u64 + 1, 1).is_err());
}

#[tokio::test]
async fn list_assets_fails_closed_on_allocator_maximum_mismatch() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    opened.shutdown().unwrap();
    let connection = rusqlite::Connection::open(fixture.database()).unwrap();
    connection
        .execute(
            "UPDATE event_commit_sequence SET last_sequence=1 WHERE singleton=1",
            [],
        )
        .unwrap();
    drop(connection);

    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let result = opened
        .asset_store_handle()
        .list_assets(ListAssetsQuery::new(64, ListAssetsPosition::First).unwrap())
        .await;
    assert_eq!(result.unwrap_err(), AssetStoreError::StorageCorruption);
    assert!(opened.shutdown().is_err());
}

#[test]
fn list_assets_queries_use_bounded_commit_sequence_index_plans() {
    let fixture = Fixture::new();
    let opened = OpenedLibrary::open_or_bootstrap(&fixture.config()).unwrap();
    opened.shutdown().unwrap();
    let connection = rusqlite::Connection::open(fixture.database()).unwrap();
    for sql in [
        "EXPLAIN QUERY PLAN SELECT coalesce(max(commit_sequence), 0) FROM domain_events",
        "EXPLAIN QUERY PLAN SELECT 1 FROM domain_events WHERE commit_sequence=1",
        "EXPLAIN QUERY PLAN SELECT commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, occurred_at_seconds, occurred_at_nanos FROM domain_events WHERE commit_sequence > 0 AND commit_sequence <= 1 ORDER BY commit_sequence LIMIT 256",
    ] {
        let mut statement = connection.prepare(sql).unwrap();
        let details = statement
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");
        assert!(!details.contains("USE TEMP B-TREE"), "{details}");
        assert!(
            details.contains("sqlite_autoindex_domain_events_2"),
            "commit-sequence index absent: {details}"
        );
    }
}

#[test]
fn inspect_asset_queries_use_hierarchical_and_location_keyset_indexes() {
    let fixture = Fixture::new();
    let opened = OpenedLibrary::open_or_bootstrap(&fixture.config()).unwrap();
    opened.shutdown().unwrap();
    let connection = rusqlite::Connection::open(fixture.database()).unwrap();
    let plans = [
        (
            "EXPLAIN QUERY PLAN SELECT representation_id FROM representations WHERE asset_revision_id=zeroblob(16) ORDER BY representation_id LIMIT 1",
            "representations_revision_idx",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT resource_id FROM resources WHERE representation_id=zeroblob(16) AND resource_id>zeroblob(16) ORDER BY resource_id LIMIT 1",
            "resources_representation_idx",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT ordinal FROM resource_members WHERE resource_id=zeroblob(16) AND ordinal>0 ORDER BY ordinal LIMIT 1",
            "sqlite_autoindex_resource_members_1",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT location_id, custody, durability, lifecycle FROM locations WHERE blob_digest=zeroblob(32) ORDER BY backend_id, location_id LIMIT 65",
            "locations_blob_backend_idx",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT location_id, custody, durability, lifecycle FROM locations WHERE blob_digest=zeroblob(32) AND (backend_id, location_id)>('backend', zeroblob(16)) ORDER BY backend_id, location_id LIMIT 65",
            "locations_blob_backend_idx",
        ),
        (
            "EXPLAIN QUERY PLAN SELECT blob_digest, backend_id FROM locations WHERE location_id=zeroblob(16)",
            "sqlite_autoindex_locations_1",
        ),
    ];
    for (sql, required_index) in plans {
        let mut statement = connection.prepare(sql).unwrap();
        let details = statement
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
            .join("\n");
        assert!(!details.contains("USE TEMP B-TREE"), "{details}");
        assert!(
            details.contains(required_index),
            "required index {required_index} absent: {details}"
        );
    }
}

#[tokio::test]
async fn materialization_resolution_is_exact_backend_scoped_and_opaque() {
    let fixture = Fixture::new();
    let opened = OpenedLibrary::open_or_bootstrap(&fixture.config()).unwrap();
    let store = opened.asset_store_handle();
    let registration = register(&store, 1).await;
    let backend_id = format!("mengxia.local-cas.v1/{}", "55".repeat(32));
    let selection = MaterializationSelection::new(
        registration.asset_id,
        registration.revision_id,
        registration.representation_id,
        registration.resource_id,
        0,
        backend_id.clone(),
    )
    .unwrap();
    let resolved = store.resolve_materialization(selection).await.unwrap();
    assert_eq!(resolved.asset_id(), registration.asset_id);
    assert_eq!(resolved.asset_revision_id(), registration.revision_id);
    assert_eq!(resolved.representation_id(), registration.representation_id);
    assert_eq!(resolved.resource_id(), registration.resource_id);
    assert_eq!(resolved.member_ordinal(), 0);
    assert_eq!(resolved.blob_digest(), Sha256Digest::from_bytes([0x81; 32]));
    assert_eq!(resolved.byte_length(), 1);
    assert_eq!(resolved.__backend_id_for_local_adapter(), backend_id);
    assert_eq!(
        resolved.__locator_for_local_adapter(),
        format!("sha256-v1/81/81/{}.blob", "81".repeat(32))
    );

    let missing_backend = MaterializationSelection::new(
        registration.asset_id,
        registration.revision_id,
        registration.representation_id,
        registration.resource_id,
        0,
        format!("mengxia.local-cas.v1/{}", "66".repeat(32)),
    )
    .unwrap();
    assert!(matches!(
        store.resolve_materialization(missing_backend).await,
        Err(AssetStoreError::StorageConfiguration)
    ));

    let missing_member = MaterializationSelection::new(
        registration.asset_id,
        registration.revision_id,
        registration.representation_id,
        registration.resource_id,
        1,
        format!("mengxia.local-cas.v1/{}", "55".repeat(32)),
    )
    .unwrap();
    assert!(matches!(
        store.resolve_materialization(missing_member).await,
        Err(AssetStoreError::NotFound)
    ));
    opened.shutdown().unwrap();
}

#[tokio::test]
async fn verification_store_pages_capture_snapshot_and_managed_candidates() {
    let fixture = Fixture::new();
    let opened = OpenedLibrary::open_or_bootstrap(&fixture.config()).unwrap();
    let store = opened.asset_store_handle();
    register(&store, 1).await;
    let snapshot = store.capture_verification_snapshot().await.unwrap();
    assert_eq!(snapshot.snapshot_commit_sequence(), 1);

    let commands = store
        .scan_verification_page(snapshot, VerificationScanPosition::CommandsAfter(None))
        .await
        .unwrap();
    assert!(commands.findings().is_empty());
    assert!(matches!(
        commands.next(),
        VerificationScanPosition::ManagedLocationsAfter(None)
    ));
    let locations = store
        .scan_verification_page(snapshot, commands.next())
        .await
        .unwrap();
    assert!(locations.findings().is_empty());
    assert_eq!(locations.candidates().len(), 1);
    let candidate = &locations.candidates()[0];
    assert_eq!(
        candidate.blob_digest(),
        Sha256Digest::from_bytes([0x81; 32])
    );
    assert_eq!(candidate.byte_length(), 1);
    assert_eq!(candidate.blob_revision().get(), 1);
    assert!(
        candidate
            .__backend_id_for_local_adapter()
            .starts_with("mengxia.local-cas.v1/")
    );
    assert_eq!(
        candidate.__locator_for_local_adapter(),
        format!("sha256-v1/81/81/{}.blob", "81".repeat(32))
    );
    assert_eq!(locations.next(), VerificationScanPosition::Complete);
    opened.shutdown().unwrap();
}

#[tokio::test]
async fn verification_store_reports_prior_runtime_external_claim_without_reacquiring_it() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let command_id = Id::<Command>::try_new().unwrap();
    let binding = CommandBinding::new(
        command_id,
        ASSET_INGEST_COPY_V1,
        Sha256Digest::from_bytes([0x44; 32]),
    );
    assert_eq!(
        store
            .claim_external_ingest(ExternalIngestClaim::new(binding, at(44)).unwrap())
            .await
            .unwrap(),
        ExternalClaimOutcome::Claimed
    );
    opened.shutdown().unwrap();

    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let snapshot = store.capture_verification_snapshot().await.unwrap();
    let page = store
        .scan_verification_page(snapshot, VerificationScanPosition::CommandsAfter(None))
        .await
        .unwrap();
    assert_eq!(page.findings().len(), 1);
    assert_eq!(
        page.findings()[0].kind(),
        mengxia_ports::IntegrityIssueKind::CommandRecoveryRequired
    );
    assert_eq!(
        page.findings()[0].remediation(),
        mengxia_ports::IntegrityRemediation::OperatorOrRuntimeAction
    );
    opened.shutdown().unwrap();
}

#[tokio::test]
async fn inspect_asset_pages_every_location_without_exposing_backend_order() {
    let fixture = Fixture::new();
    let config = fixture.config();
    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let registration = register(&store, 1).await;
    let asset_id = registration.asset_id;
    let revision_id = registration.revision_id;
    opened.shutdown().unwrap();

    let second_location = Id::<Location>::try_new().unwrap();
    let mut connection = rusqlite::Connection::open(fixture.database()).unwrap();
    let transaction = connection.transaction().unwrap();
    transaction
        .execute(
            "UPDATE blobs SET revision=?2 WHERE digest=?1",
            rusqlite::params![
                Sha256Digest::from_bytes([0x81; 32]).to_bytes().as_slice(),
                2_u64.to_be_bytes().as_slice()
            ],
        )
        .unwrap();
    transaction
        .execute(
            "INSERT INTO locations (location_id, blob_digest, backend_id, locator, custody, durability, lifecycle, revision, verified_at_seconds, verified_at_nanos) VALUES (?1, ?2, 'other.backend.v1', 'opaque-locator', 'UNMANAGED', 'UNKNOWN', 'AVAILABLE', ?3, 101, 123456789)",
            rusqlite::params![
                second_location.to_bytes().as_slice(),
                Sha256Digest::from_bytes([0x81; 32]).to_bytes().as_slice(),
                1_u64.to_be_bytes().as_slice()
            ],
        )
        .unwrap();
    transaction.commit().unwrap();
    drop(connection);

    let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
    let store = opened.asset_store_handle();
    let first = store
        .inspect_asset(InspectAssetQuery::new(asset_id, None, 1, InspectAssetStart::First).unwrap())
        .await
        .unwrap();
    assert_eq!(first.selected_revision_id(), revision_id);
    assert_eq!(first.members().len(), 1);
    assert!(first.members()[0].location().is_some());
    let cursor = first.next().expect("second Location remains");

    let connection = rusqlite::Connection::open(fixture.database()).unwrap();
    connection
        .execute(
            "UPDATE blobs SET revision=?2 WHERE digest=?1",
            rusqlite::params![
                Sha256Digest::from_bytes([0x81; 32]).to_bytes().as_slice(),
                3_u64.to_be_bytes().as_slice()
            ],
        )
        .unwrap();
    let conflicted = store
        .inspect_asset(
            InspectAssetQuery::new(asset_id, None, 1, InspectAssetStart::Continue(cursor)).unwrap(),
        )
        .await;
    assert_eq!(conflicted.unwrap_err(), AssetStoreError::Conflict);
    connection
        .execute(
            "UPDATE blobs SET revision=?2 WHERE digest=?1",
            rusqlite::params![
                Sha256Digest::from_bytes([0x81; 32]).to_bytes().as_slice(),
                2_u64.to_be_bytes().as_slice()
            ],
        )
        .unwrap();
    drop(connection);

    let second = store
        .inspect_asset(
            InspectAssetQuery::new(asset_id, None, 1, InspectAssetStart::Continue(cursor)).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.selected_revision_id(), revision_id);
    assert_eq!(second.members().len(), 1);
    assert_eq!(
        second.members()[0].location().unwrap().location_id(),
        second_location
    );
    assert_eq!(second.next(), None);
    opened.shutdown().unwrap();
}
