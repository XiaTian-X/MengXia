use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

use mengxia_ports::{
    BlobSourceError, BlobStorage, BlobStorageError, IngestControl, IngestDirective, IngestOutcome,
};
use mengxia_storage_local::{
    BlobConfigSource, BlobIngestState, LocalBlobStorage, ResolvedBlobStorageConfig,
};
use mengxia_store_sqlite::{ConfigSource, OpenedLibrary, ResolvedStoreConfig, StoreError};
use mengxia_types::Sha256Digest;
use sha2::{Digest as _, Sha256};

static NEXT: AtomicU64 = AtomicU64::new(0);
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

fn raw(value: impl ToString) -> Option<String> {
    Some(value.to_string())
}

struct Continue;

impl IngestControl for Continue {
    fn checkpoint(&self) -> IngestDirective {
        IngestDirective::Continue
    }
}

struct StopAt {
    target: u64,
    count: AtomicU64,
    barrier: Option<Arc<Barrier>>,
    stop: mengxia_ports::IngestStop,
}

impl IngestControl for StopAt {
    fn checkpoint(&self) -> IngestDirective {
        let count = self.count.fetch_add(1, Ordering::SeqCst) + 1;
        if count == self.target {
            if let Some(barrier) = &self.barrier {
                barrier.wait();
            }
            return IngestDirective::Stop(self.stop);
        }
        IngestDirective::Continue
    }
}

struct PanicAt {
    target: u64,
    count: AtomicU64,
}

struct PauseAt {
    target: u64,
    count: AtomicU64,
    barrier: Arc<Barrier>,
}

struct SynchronizeAt {
    target: u64,
    count: AtomicU64,
    barrier: Arc<Barrier>,
}

impl IngestControl for SynchronizeAt {
    fn checkpoint(&self) -> IngestDirective {
        if self.count.fetch_add(1, Ordering::SeqCst) + 1 == self.target {
            self.barrier.wait();
        }
        IngestDirective::Continue
    }
}

impl IngestControl for PauseAt {
    fn checkpoint(&self) -> IngestDirective {
        if self.count.fetch_add(1, Ordering::SeqCst) + 1 == self.target {
            self.barrier.wait();
            self.barrier.wait();
        }
        IngestDirective::Continue
    }
}

impl IngestControl for PanicAt {
    fn checkpoint(&self) -> IngestDirective {
        let count = self.count.fetch_add(1, Ordering::SeqCst) + 1;
        assert_ne!(count, self.target, "injected control panic");
        IngestDirective::Continue
    }
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap()
            .join("target/task-005-local-cas")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
            .unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn library_root(&self) -> PathBuf {
        self.root.join("Library")
    }
    fn blob_root(&self) -> PathBuf {
        self.library_root().join("storage")
    }

    fn store_config(&self) -> mengxia_store_sqlite::StoreConfig {
        ResolvedStoreConfig::from_selected(
            Some(self.library_root()),
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

    fn blob_config(&self) -> mengxia_storage_local::BlobStorageConfig {
        let source = BlobConfigSource::CompiledDefault;
        ResolvedBlobStorageConfig::from_selected(
            Some(self.library_root()),
            BlobConfigSource::Cli,
            Some(self.blob_root()),
            source,
            raw(2),
            source,
            raw(2),
            source,
            raw(2),
            source,
            raw(MIB as usize),
            source,
            raw(GIB),
            source,
            raw(2 * GIB),
            source,
            raw(10 * GIB),
            source,
            raw(5),
            source,
        )
        .validate()
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn start(fixture: &Fixture) -> (OpenedLibrary, LocalBlobStorage) {
    let store = OpenedLibrary::open_or_bootstrap(&fixture.store_config()).unwrap();
    let config = fixture.blob_config();
    let authority = store
        .authorize_blob_root(config.blob_root_request())
        .unwrap();
    let (storage, report) = LocalBlobStorage::start(config, authority).unwrap();
    assert_eq!(report.ingest_state(), BlobIngestState::Ready);
    assert_eq!(report.staging_orphan_count(), 0);
    assert_eq!(report.backend_id().len(), 85);
    (store, storage)
}

#[test]
fn streams_hashes_promotes_and_deduplicates_without_exposing_a_root() {
    let fixture = Fixture::new();
    let source_path = fixture.root.join("source.bin");
    let bytes = b"MengXia TASK-005 local CAS";
    fs::write(&source_path, bytes).unwrap();
    let expected = Sha256Digest::from_bytes(Sha256::digest(bytes).into());
    let (store, storage) = start(&fixture);

    for _ in 0..2 {
        let source = storage.open_source(&source_path).unwrap();
        let outcome = storage
            .ingest(source, Some(expected), Arc::new(Continue))
            .unwrap();
        let IngestOutcome::Stored(blob) = outcome else {
            panic!("stored outcome");
        };
        assert_eq!(blob.digest(), expected);
        assert_eq!(blob.byte_length(), bytes.len() as u64);
        assert_eq!(blob.location().backend_id().len(), 85);
        assert_eq!(blob.location().locator().len(), 85);
    }
    let digest = expected.to_string();
    assert_eq!(
        fs::read(
            fixture
                .blob_root()
                .join("sha256-v1")
                .join(&digest[..2])
                .join(&digest[2..4])
                .join(format!("{digest}.blob"))
        )
        .unwrap(),
        bytes
    );
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
}

#[test]
fn source_mutation_and_expected_digest_mismatch_fail_without_publish() {
    let fixture = Fixture::new();
    let source_path = fixture.root.join("source.bin");
    fs::write(&source_path, b"before").unwrap();
    let (store, storage) = start(&fixture);
    let opened = storage.open_source(&source_path).unwrap();
    fs::write(&source_path, b"changed-length").unwrap();
    assert_eq!(
        storage.ingest(opened, None, Arc::new(Continue)).err(),
        Some(BlobStorageError::SourceModified)
    );

    let opened = storage.open_source(&source_path).unwrap();
    assert_eq!(
        storage
            .ingest(
                opened,
                Some(Sha256Digest::from_bytes([0; 32])),
                Arc::new(Continue)
            )
            .err(),
        Some(BlobStorageError::Corruption)
    );
    let replacement = fixture.root.join("replacement.bin");
    fs::write(&replacement, b"same-size-byte").unwrap();
    let opened = storage.open_source(&source_path).unwrap();
    fs::rename(&replacement, &source_path).unwrap();
    assert_eq!(
        storage.ingest(opened, None, Arc::new(Continue)).err(),
        Some(BlobStorageError::SourceModified)
    );
    assert!(
        fs::read_dir(fixture.blob_root().join(".staging-v1"))
            .unwrap()
            .next()
            .is_none()
    );
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
}

#[test]
fn blob_authority_keeps_library_lock_after_sqlite_shutdown() {
    let fixture = Fixture::new();
    let config = fixture.store_config();
    let (store, storage) = start(&fixture);
    store.shutdown().unwrap();
    assert_eq!(
        OpenedLibrary::open_or_bootstrap(&config).err(),
        Some(StoreError::Conflict)
    );
    storage.shutdown().unwrap();
    OpenedLibrary::open_or_bootstrap(&config)
        .unwrap()
        .shutdown()
        .unwrap();
}

#[test]
fn zero_byte_hard_link_and_internal_source_boundaries_are_exact() {
    let fixture = Fixture::new();
    let empty = fixture.root.join("empty");
    let hard_link = fixture.root.join("empty-link");
    fs::write(&empty, []).unwrap();
    fs::hard_link(&empty, &hard_link).unwrap();
    let (store, storage) = start(&fixture);
    let outcome = storage
        .ingest(
            storage.open_source(&hard_link).unwrap(),
            None,
            Arc::new(Continue),
        )
        .unwrap();
    let IngestOutcome::Stored(blob) = outcome else {
        panic!("stored");
    };
    assert_eq!(blob.byte_length(), 0);
    assert_eq!(
        blob.digest().to_string(),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert!(
        storage
            .open_source(&fixture.library_root().join("library.sqlite3"))
            .is_err()
    );
    let nonunicode = fixture.root.join(std::ffi::OsString::from_vec(vec![
        b'n', b'o', b'n', b'-', 0xff,
    ]));
    // APFS on this host rejects creating the invalid UTF-8 name with EILSEQ;
    // the source validator still treats the bytes as an OS lookup, not as a
    // configuration/Unicode validation failure.
    assert_eq!(
        storage.open_source(&nonunicode).err(),
        Some(BlobSourceError::Io)
    );
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
}

#[test]
fn cooperative_stop_and_control_panic_cleanup_without_poisoning_runtime() {
    let fixture = Fixture::new();
    let source_path = fixture.root.join("source.bin");
    fs::write(&source_path, vec![7_u8; MIB as usize + 17]).unwrap();
    let (store, storage) = start(&fixture);
    let stopped = storage
        .ingest(
            storage.open_source(&source_path).unwrap(),
            None,
            Arc::new(StopAt {
                target: 4,
                count: AtomicU64::new(0),
                barrier: None,
                stop: mengxia_ports::IngestStop::Cancelled,
            }),
        )
        .unwrap();
    assert!(matches!(
        stopped,
        IngestOutcome::Stopped(mengxia_ports::IngestStop::Cancelled)
    ));
    assert!(
        fs::read_dir(fixture.blob_root().join(".staging-v1"))
            .unwrap()
            .next()
            .is_none()
    );

    for target in [2, 4] {
        let panic_result = storage.ingest(
            storage.open_source(&source_path).unwrap(),
            None,
            Arc::new(PanicAt {
                target,
                count: AtomicU64::new(0),
            }),
        );
        assert_eq!(panic_result.err(), Some(BlobStorageError::Internal));
        assert!(
            fs::read_dir(fixture.blob_root().join(".staging-v1"))
                .unwrap()
                .next()
                .is_none()
        );
    }

    // This two-chunk stream has fifteen cooperative checkpoints before
    // publication. Exercise both stop classes at every boundary. Target 16 is
    // intentionally observed only after commit and therefore cannot turn a
    // durable success into a false rollback.
    for stop in [
        mengxia_ports::IngestStop::Cancelled,
        mengxia_ports::IngestStop::DeadlineReached,
    ] {
        for target in 1..=15 {
            let result = storage
                .ingest(
                    storage.open_source(&source_path).unwrap(),
                    None,
                    Arc::new(StopAt {
                        target,
                        count: AtomicU64::new(0),
                        barrier: None,
                        stop,
                    }),
                )
                .unwrap();
            assert!(matches!(result, IngestOutcome::Stopped(actual) if actual == stop));
            assert!(
                fs::read_dir(fixture.blob_root().join(".staging-v1"))
                    .unwrap()
                    .next()
                    .is_none()
            );
        }
        let post_promote = storage
            .ingest(
                storage.open_source(&source_path).unwrap(),
                None,
                Arc::new(StopAt {
                    target: 16,
                    count: AtomicU64::new(0),
                    barrier: None,
                    stop,
                }),
            )
            .unwrap();
        assert!(matches!(post_promote, IngestOutcome::Stored(_)));
    }
    let retry = storage.ingest(
        storage.open_source(&source_path).unwrap(),
        None,
        Arc::new(Continue),
    );
    assert!(matches!(retry, Ok(IngestOutcome::Stored(_))));
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
}

#[test]
fn restart_reports_valid_staging_orphan_without_deleting_it() {
    let fixture = Fixture::new();
    let store_config = fixture.store_config();
    let (store, storage) = start(&fixture);
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
    let orphan = fixture
        .blob_root()
        .join(".staging-v1/.ingest-00000000000000000000000000000000.part");
    fs::write(&orphan, b"orphan").unwrap();
    fs::set_permissions(&orphan, fs::Permissions::from_mode(0o600)).unwrap();

    let store = OpenedLibrary::open_or_bootstrap(&store_config).unwrap();
    let config = fixture.blob_config();
    let authority = store
        .authorize_blob_root(config.blob_root_request())
        .unwrap();
    let (storage, report) = LocalBlobStorage::start(config, authority).unwrap();
    assert_eq!(
        report.ingest_state(),
        BlobIngestState::OrphanReconciliationRequired
    );
    assert_eq!(report.staging_orphan_count(), 1);
    assert_eq!(report.staging_orphan_bytes(), 6);
    assert!(orphan.exists());
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
}

#[test]
fn atomic_admission_returns_backpressure_without_a_second_staging_file() {
    let fixture = Fixture::new();
    let first_path = fixture.root.join("first.bin");
    let second_path = fixture.root.join("second.bin");
    fs::write(&first_path, vec![1_u8; MIB as usize + 1]).unwrap();
    fs::write(&second_path, b"second").unwrap();
    let store = OpenedLibrary::open_or_bootstrap(&fixture.store_config()).unwrap();
    let source = BlobConfigSource::CompiledDefault;
    let config = ResolvedBlobStorageConfig::from_selected(
        Some(fixture.library_root()),
        source,
        Some(fixture.blob_root()),
        source,
        raw(1),
        source,
        raw(1),
        source,
        raw(1),
        source,
        raw(MIB as usize),
        source,
        raw(GIB),
        source,
        raw(2 * GIB),
        source,
        raw(10 * GIB),
        source,
        raw(5),
        source,
    )
    .validate()
    .unwrap();
    let authority = store
        .authorize_blob_root(config.blob_root_request())
        .unwrap();
    let (storage, _) = LocalBlobStorage::start(config, authority).unwrap();
    let storage = Arc::new(storage);
    let first = storage.open_source(&first_path).unwrap();
    let second = storage.open_source(&second_path).unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let worker_storage = Arc::clone(&storage);
    let worker_barrier = Arc::clone(&barrier);
    let worker = std::thread::spawn(move || {
        worker_storage.ingest(
            first,
            None,
            Arc::new(PauseAt {
                target: 3,
                count: AtomicU64::new(0),
                barrier: worker_barrier,
            }),
        )
    });
    barrier.wait();
    assert_eq!(
        storage.ingest(second, None, Arc::new(Continue)).err(),
        Some(BlobStorageError::Backpressure)
    );
    assert!(
        fs::read_dir(fixture.blob_root().join(".staging-v1"))
            .unwrap()
            .next()
            .is_none()
    );
    barrier.wait();
    assert!(matches!(
        worker.join().unwrap(),
        Ok(IngestOutcome::Stored(_))
    ));
    Arc::try_unwrap(storage)
        .ok()
        .expect("joined caller")
        .shutdown()
        .unwrap();
    store.shutdown().unwrap();
}

#[test]
fn task_004_reopen_rejects_an_unsafe_storage_directory_without_mutation() {
    let fixture = Fixture::new();
    let config = fixture.store_config();
    let (store, storage) = start(&fixture);
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
    fs::set_permissions(fixture.blob_root(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        OpenedLibrary::open_or_bootstrap(&config).err(),
        Some(StoreError::Configuration)
    );
    assert!(fixture.blob_root().exists());
}

#[test]
fn source_symlinks_and_invalid_orphan_names_fail_closed() {
    let fixture = Fixture::new();
    let source_path = fixture.root.join("source.bin");
    let symlink_path = fixture.root.join("source-link.bin");
    fs::write(&source_path, b"source").unwrap();
    std::os::unix::fs::symlink(&source_path, &symlink_path).unwrap();
    let (store, storage) = start(&fixture);
    assert!(storage.open_source(&symlink_path).is_err());
    storage.shutdown().unwrap();
    store.shutdown().unwrap();

    let invalid = fixture.blob_root().join(".staging-v1/not-an-ingest-name");
    fs::write(&invalid, b"preserve").unwrap();
    fs::set_permissions(&invalid, fs::Permissions::from_mode(0o600)).unwrap();
    let store = OpenedLibrary::open_or_bootstrap(&fixture.store_config()).unwrap();
    let config = fixture.blob_config();
    let authority = store
        .authorize_blob_root(config.blob_root_request())
        .unwrap();
    assert_eq!(
        LocalBlobStorage::start(config, authority).err(),
        Some(BlobStorageError::Configuration)
    );
    assert!(invalid.exists());
    store.shutdown().unwrap();
}

#[test]
fn hostile_existing_canonical_is_preserved_with_staging_evidence() {
    let fixture = Fixture::new();
    let source_path = fixture.root.join("source.bin");
    let bytes = b"canonical integrity";
    fs::write(&source_path, bytes).unwrap();
    let digest = Sha256Digest::from_bytes(Sha256::digest(bytes).into());
    let (store, storage) = start(&fixture);
    let source = storage.open_source(&source_path).unwrap();
    assert!(matches!(
        storage.ingest(source, Some(digest), Arc::new(Continue)),
        Ok(IngestOutcome::Stored(_))
    ));
    let hex = digest.to_string();
    let canonical = fixture
        .blob_root()
        .join("sha256-v1")
        .join(&hex[..2])
        .join(&hex[2..4])
        .join(format!("{hex}.blob"));
    fs::write(&canonical, b"hostile replacement bytes").unwrap();
    fs::set_permissions(&canonical, fs::Permissions::from_mode(0o600)).unwrap();
    let source = storage.open_source(&source_path).unwrap();
    assert_eq!(
        storage
            .ingest(source, Some(digest), Arc::new(Continue))
            .err(),
        Some(BlobStorageError::Corruption)
    );
    assert_eq!(fs::read(&canonical).unwrap(), b"hostile replacement bytes");
    assert_eq!(
        fs::read_dir(fixture.blob_root().join(".staging-v1"))
            .unwrap()
            .count(),
        1
    );
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
}

#[test]
fn mismatched_config_and_authority_fail_before_mutating_requested_root() {
    let fixture = Fixture::new();
    let store = OpenedLibrary::open_or_bootstrap(&fixture.store_config()).unwrap();
    let first = fixture.blob_config();
    let authority = store
        .authorize_blob_root(first.blob_root_request())
        .unwrap();
    let other_root = fixture.root.join("other-storage");
    let source = BlobConfigSource::CompiledDefault;
    let other = ResolvedBlobStorageConfig::from_selected(
        Some(fixture.library_root()),
        source,
        Some(other_root.clone()),
        source,
        raw(1),
        source,
        raw(1),
        source,
        raw(1),
        source,
        raw(MIB as usize),
        source,
        raw(GIB),
        source,
        raw(2 * GIB),
        source,
        raw(10 * GIB),
        source,
        raw(5),
        source,
    )
    .validate()
    .unwrap();
    assert_eq!(
        LocalBlobStorage::start(other, authority).err(),
        Some(BlobStorageError::Configuration)
    );
    assert!(!other_root.exists());
    store.shutdown().unwrap();
}

#[test]
fn backend_identity_tracks_root_inode_not_configured_path_text() {
    let fixture = Fixture::new();
    let store = OpenedLibrary::open_or_bootstrap(&fixture.store_config()).unwrap();
    let source = BlobConfigSource::CompiledDefault;
    let make_config = |root: PathBuf| {
        ResolvedBlobStorageConfig::from_selected(
            Some(fixture.library_root()),
            source,
            Some(root),
            source,
            raw(1),
            source,
            raw(1),
            source,
            raw(1),
            source,
            raw(MIB as usize),
            source,
            raw(GIB),
            source,
            raw(2 * GIB),
            source,
            raw(10 * GIB),
            source,
            raw(5),
            source,
        )
        .validate()
        .unwrap()
    };
    let original = fixture.root.join("external-cas");
    let first_config = make_config(original.clone());
    let first_authority = store
        .authorize_blob_root(first_config.blob_root_request())
        .unwrap();
    let (first, first_report) = LocalBlobStorage::start(first_config, first_authority).unwrap();
    let first_id = first_report.backend_id().to_owned();
    first.shutdown().unwrap();

    let moved = fixture.root.join("renamed-cas");
    fs::rename(&original, &moved).unwrap();
    let moved_config = make_config(moved.clone());
    let moved_authority = store
        .authorize_blob_root(moved_config.blob_root_request())
        .unwrap();
    let (moved_storage, moved_report) =
        LocalBlobStorage::start(moved_config, moved_authority).unwrap();
    assert_eq!(moved_report.backend_id(), first_id);
    moved_storage.shutdown().unwrap();

    fs::rename(&moved, &original).unwrap();
    let replacement_config = make_config(moved);
    let replacement_authority = store
        .authorize_blob_root(replacement_config.blob_root_request())
        .unwrap();
    let (replacement, replacement_report) =
        LocalBlobStorage::start(replacement_config, replacement_authority).unwrap();
    assert_ne!(replacement_report.backend_id(), first_id);
    replacement.shutdown().unwrap();
    store.shutdown().unwrap();
}

#[test]
fn concurrent_identical_ingests_publish_once_and_both_return_the_same_durable_blob() {
    let fixture = Fixture::new();
    let first_path = fixture.root.join("first.bin");
    let second_path = fixture.root.join("second.bin");
    let bytes = vec![0x5a_u8; MIB as usize + 73];
    fs::write(&first_path, &bytes).unwrap();
    fs::write(&second_path, &bytes).unwrap();
    let expected = Sha256Digest::from_bytes(Sha256::digest(&bytes).into());
    let (store, storage) = start(&fixture);
    let first = storage.open_source(&first_path).unwrap();
    let second = storage.open_source(&second_path).unwrap();
    let storage = Arc::new(storage);
    let barrier = Arc::new(Barrier::new(2));

    let spawn_ingest = |source| {
        let storage = Arc::clone(&storage);
        let barrier = Arc::clone(&barrier);
        std::thread::spawn(move || {
            storage.ingest(
                source,
                Some(expected),
                Arc::new(SynchronizeAt {
                    // Both jobs have fully copied and hashed their staging inode,
                    // but neither has begun the no-clobber promote.
                    target: 19,
                    count: AtomicU64::new(0),
                    barrier,
                }),
            )
        })
    };
    let first_worker = spawn_ingest(first);
    let second_worker = spawn_ingest(second);
    let first = first_worker.join().unwrap().unwrap();
    let second = second_worker.join().unwrap().unwrap();

    let (IngestOutcome::Stored(first), IngestOutcome::Stored(second)) = (first, second) else {
        panic!("both concurrent operations must return durable storage results");
    };
    assert_eq!(first.digest(), expected);
    assert_eq!(second.digest(), expected);
    assert_eq!(
        first.location().backend_id(),
        second.location().backend_id()
    );
    assert_eq!(first.location().locator(), second.location().locator());
    assert_eq!(
        fs::read_dir(fixture.blob_root().join(".staging-v1"))
            .unwrap()
            .count(),
        0
    );
    let digest = expected.to_string();
    let shard = fixture
        .blob_root()
        .join("sha256-v1")
        .join(&digest[..2])
        .join(&digest[2..4]);
    assert_eq!(fs::read_dir(shard).unwrap().count(), 1);

    Arc::try_unwrap(storage)
        .ok()
        .expect("joined callers")
        .shutdown()
        .unwrap();
    store.shutdown().unwrap();
}

#[test]
fn staging_orphan_enumeration_accepts_4096_and_rejects_4097_without_deletion() {
    let fixture = Fixture::new();
    let store_config = fixture.store_config();
    let (store, storage) = start(&fixture);
    storage.shutdown().unwrap();
    store.shutdown().unwrap();
    let staging = fixture.blob_root().join(".staging-v1");
    for index in 0_u16..4096 {
        let orphan = staging.join(format!(".ingest-{index:032x}.part"));
        fs::write(&orphan, []).unwrap();
        fs::set_permissions(orphan, fs::Permissions::from_mode(0o600)).unwrap();
    }

    let store = OpenedLibrary::open_or_bootstrap(&store_config).unwrap();
    let config = fixture.blob_config();
    let authority = store
        .authorize_blob_root(config.blob_root_request())
        .unwrap();
    let (storage, report) = LocalBlobStorage::start(config, authority).unwrap();
    assert_eq!(report.staging_orphan_count(), 4096);
    assert_eq!(report.staging_orphan_bytes(), 0);
    assert_eq!(
        report.ingest_state(),
        BlobIngestState::OrphanReconciliationRequired
    );
    storage.shutdown().unwrap();
    store.shutdown().unwrap();

    let overflow = staging.join(".ingest-00000000000000000000000000010000.part");
    fs::write(&overflow, []).unwrap();
    fs::set_permissions(&overflow, fs::Permissions::from_mode(0o600)).unwrap();
    let store = OpenedLibrary::open_or_bootstrap(&store_config).unwrap();
    let config = fixture.blob_config();
    let authority = store
        .authorize_blob_root(config.blob_root_request())
        .unwrap();
    assert_eq!(
        LocalBlobStorage::start(config, authority).err(),
        Some(BlobStorageError::Configuration)
    );
    assert_eq!(fs::read_dir(&staging).unwrap().count(), 4097);
    store.shutdown().unwrap();
}
