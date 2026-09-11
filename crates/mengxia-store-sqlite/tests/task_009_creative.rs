use std::fs;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use mengxia_domain::{
    Asset, AssetLifecycle, AssetRevision, ContentKind, LogicalName, RepresentationPurpose,
    ResourceKind,
};
use mengxia_ports::{
    ASSET_RESTORE_V1, ASSET_REVISION_CREATE_V1, AssetRevisionMemberInput,
    AssetRevisionRepresentationInput, AssetRevisionResourceInput, AssetStoreError, AssetUnitOfWork,
    Command, CommandBinding, DeferredAssetLifecycleCommand, DeferredCreateAssetRevisionCommand,
    MutationOutcome, PureCommandValueSource,
};
use mengxia_store_sqlite::{ConfigSource, OpenedLibrary, ResolvedStoreConfig};
use mengxia_types::{ErrorCode, Id, RevisionNo, Sha256Digest, Timestamp};

struct Fixture(PathBuf);

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .join("target/task-009-creative-tests")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
            .expect("create fixture parent");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("secure fixture");
        Self(root)
    }

    fn config(&self) -> mengxia_store_sqlite::StoreConfig {
        self.config_with_reserve(10 * 1024 * 1024 * 1024)
    }

    fn config_with_reserve(&self, min_free_bytes: u64) -> mengxia_store_sqlite::StoreConfig {
        ResolvedStoreConfig::from_selected(
            Some(self.0.join("Library")),
            ConfigSource::Cli,
            16,
            ConfigSource::CompiledDefault,
            1,
            ConfigSource::CompiledDefault,
            100,
            ConfigSource::CompiledDefault,
        )
        .with_migration_reserve(min_free_bytes, 5)
        .validate()
        .expect("valid config")
    }
}

#[test]
fn migration_capacity_failure_follows_durable_intent_and_restart_completes() {
    let fixture = Fixture::new();
    assert_eq!(
        OpenedLibrary::open_or_bootstrap(&fixture.config_with_reserve(u64::MAX)).err(),
        Some(mengxia_store_sqlite::StoreError::Io)
    );
    let library = fixture.0.join("Library");
    assert!(library.join(".mengxia-migration-0002.intent").is_file());
    assert!(
        !library
            .join(".library.sqlite3.pre-0002.snapshot.staging")
            .exists()
    );
    assert!(!library.join(".library.sqlite3.pre-0002.snapshot").exists());

    OpenedLibrary::open_or_bootstrap(&fixture.config())
        .unwrap()
        .shutdown()
        .unwrap();
    assert!(library.join(".mengxia-migration-0002.intent").is_file());
    assert!(library.join(".library.sqlite3.pre-0002.snapshot").is_file());
    assert!(
        !library
            .join(".library.sqlite3.pre-0002.snapshot.staging")
            .exists()
    );
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct CountingValues {
    ids: AtomicUsize,
    clocks: AtomicUsize,
    fail_if_used: bool,
}

impl CountingValues {
    const fn active() -> Self {
        Self {
            ids: AtomicUsize::new(0),
            clocks: AtomicUsize::new(0),
            fail_if_used: false,
        }
    }

    const fn forbidden() -> Self {
        Self {
            ids: AtomicUsize::new(0),
            clocks: AtomicUsize::new(0),
            fail_if_used: true,
        }
    }
}

impl PureCommandValueSource for CountingValues {
    fn next_uuid_v7(&self) -> Result<[u8; 16], AssetStoreError> {
        if self.fail_if_used {
            return Err(AssetStoreError::Internal);
        }
        let index = self.ids.fetch_add(1, Ordering::SeqCst);
        let mut value = [
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x01,
        ];
        value[15] = u8::try_from(index + 1).expect("bounded fixture IDs");
        Ok(value)
    }

    fn now(&self) -> Result<Timestamp, AssetStoreError> {
        if self.fail_if_used {
            return Err(AssetStoreError::Internal);
        }
        self.clocks.fetch_add(1, Ordering::SeqCst);
        Timestamp::from_unix_seconds_nanos(1_700_000_000, 7).map_err(|_| AssetStoreError::Internal)
    }

    fn checkpoint(&self) -> Result<(), AssetStoreError> {
        if self.fail_if_used {
            Err(AssetStoreError::Internal)
        } else {
            Ok(())
        }
    }
}

fn fixed_id<T>(last: u8) -> Id<T> {
    Id::from_bytes([
        0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        last,
    ])
    .expect("valid UUIDv7")
}

fn revision_request(
    binding: CommandBinding,
    source: Arc<CountingValues>,
) -> DeferredCreateAssetRevisionCommand {
    let resource = AssetRevisionResourceInput::new(
        ResourceKind::new("file").unwrap(),
        vec![AssetRevisionMemberInput::new(
            LogicalName::new("frame.png").unwrap(),
            Sha256Digest::from_bytes([0x81; 32]),
        )],
    )
    .unwrap();
    let representation = AssetRevisionRepresentationInput::new(
        RepresentationPurpose::new("original").unwrap(),
        vec![resource],
    )
    .unwrap();
    DeferredCreateAssetRevisionCommand::new(
        binding,
        fixed_id::<Asset>(0x20),
        RevisionNo::new(1),
        vec![fixed_id::<AssetRevision>(0x21)],
        ContentKind::new("raster").unwrap(),
        vec![representation],
        source,
    )
    .unwrap()
}

#[tokio::test]
async fn deferred_asset_commands_sample_only_after_absent_and_never_on_replay() {
    let fixture = Fixture::new();
    let opened = OpenedLibrary::open_or_bootstrap(&fixture.config()).unwrap();
    let store = opened.asset_store_handle();

    let revision_binding = CommandBinding::new(
        fixed_id::<Command>(0x10),
        ASSET_REVISION_CREATE_V1,
        Sha256Digest::from_bytes([0x31; 32]),
    );
    let revision_values = Arc::new(CountingValues::active());
    let first = store
        .execute_deferred_create_revision(revision_request(
            revision_binding,
            Arc::clone(&revision_values),
        ))
        .await
        .unwrap();
    assert_eq!(
        first,
        MutationOutcome::TerminalRejected {
            safe_error_code: ErrorCode::NotFound
        }
    );
    assert_eq!(revision_values.ids.load(Ordering::SeqCst), 5);
    assert_eq!(revision_values.clocks.load(Ordering::SeqCst), 1);

    let forbidden = Arc::new(CountingValues::forbidden());
    let replay = store
        .execute_deferred_create_revision(revision_request(
            revision_binding,
            Arc::clone(&forbidden),
        ))
        .await
        .unwrap();
    assert_eq!(replay, first);
    assert_eq!(forbidden.ids.load(Ordering::SeqCst), 0);
    assert_eq!(forbidden.clocks.load(Ordering::SeqCst), 0);

    let lifecycle_binding = CommandBinding::new(
        fixed_id::<Command>(0x11),
        ASSET_RESTORE_V1,
        Sha256Digest::from_bytes([0x32; 32]),
    );
    let lifecycle_values = Arc::new(CountingValues::active());
    let lifecycle = DeferredAssetLifecycleCommand::new(
        lifecycle_binding,
        fixed_id::<Asset>(0x22),
        RevisionNo::new(1),
        AssetLifecycle::Active,
        lifecycle_values.clone(),
    )
    .unwrap();
    let first = store
        .execute_deferred_asset_lifecycle(lifecycle)
        .await
        .unwrap();
    assert_eq!(
        first,
        MutationOutcome::TerminalRejected {
            safe_error_code: ErrorCode::NotFound
        }
    );
    assert_eq!(lifecycle_values.ids.load(Ordering::SeqCst), 1);
    assert_eq!(lifecycle_values.clocks.load(Ordering::SeqCst), 1);

    let forbidden = Arc::new(CountingValues::forbidden());
    let replay = store
        .execute_deferred_asset_lifecycle(
            DeferredAssetLifecycleCommand::new(
                lifecycle_binding,
                fixed_id::<Asset>(0x22),
                RevisionNo::new(1),
                AssetLifecycle::Active,
                forbidden.clone(),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay, first);
    assert_eq!(forbidden.ids.load(Ordering::SeqCst), 0);
    assert_eq!(forbidden.clocks.load(Ordering::SeqCst), 0);

    opened.shutdown().unwrap();
}
