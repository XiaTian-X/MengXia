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
    ASSET_INGEST_COPY_V1, AssetQueryPort as _, AssetStoreError, Command, CommandBinding,
    DurableBlob, ExternalClaimOutcome, ExternalIngestClaim, ExternalIngestCompletion,
    ListAssetsPosition, ListAssetsQuery, ManagedRegistrationPlan, MutationOutcome,
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

async fn register(store: &impl mengxia_ports::AssetUnitOfWork, index: u8) -> Id<Asset> {
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
    let plan = ManagedRegistrationPlan::new(
        asset_id,
        AssetKind::new("image").unwrap(),
        Id::<AssetRevision>::try_new().unwrap(),
        ContentKind::new("raster").unwrap(),
        Id::<Representation>::try_new().unwrap(),
        RepresentationPurpose::new("original").unwrap(),
        Id::<Resource>::try_new().unwrap(),
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
    asset_id
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

    let first_id = register(&store, 1).await;
    let second_id = register(&store, 2).await;
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
