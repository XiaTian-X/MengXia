// TASK005_FORMAL_MATRIX_COMPLETE: YES

use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use mengxia_ports::{BlobStorage, IngestControl, IngestDirective};
use mengxia_storage_local::{
    BlobConfigSource, BlobIngestState, LocalBlobStorage, ResolvedBlobStorageConfig,
};
use mengxia_store_sqlite::{ConfigSource, OpenedLibrary, ResolvedStoreConfig};

static NEXT: AtomicU64 = AtomicU64::new(0);
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

struct PauseAt {
    target: u64,
    count: AtomicU64,
    ready: PathBuf,
}

impl IngestControl for PauseAt {
    fn checkpoint(&self) -> IngestDirective {
        let count = self.count.fetch_add(1, Ordering::SeqCst) + 1;
        if count == self.target {
            let ready = fs::File::create(&self.ready).expect("create stream crash acknowledgement");
            ready.sync_all().expect("sync stream crash acknowledgement");
            loop {
                std::thread::park();
            }
        }
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
            .join("target/task-005-recovery")
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

    fn library(&self) -> PathBuf {
        self.root.join("Library")
    }

    fn blob(&self) -> PathBuf {
        self.library().join("storage")
    }

    fn source(&self) -> PathBuf {
        self.root.join("source.bin")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn store_config(library: PathBuf) -> mengxia_store_sqlite::StoreConfig {
    ResolvedStoreConfig::from_selected(
        Some(library),
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

fn raw(value: impl ToString) -> Option<String> {
    Some(value.to_string())
}

fn blob_config(library: PathBuf, blob: PathBuf) -> mengxia_storage_local::BlobStorageConfig {
    let source = BlobConfigSource::CompiledDefault;
    ResolvedBlobStorageConfig::from_selected(
        Some(library),
        BlobConfigSource::Cli,
        Some(blob),
        source,
        raw(1),
        source,
        raw(1),
        source,
        raw(1),
        source,
        raw(MIB),
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

#[test]
fn exact_stream_sigkill_boundaries_restart_as_reported_orphans() {
    let source_length = 2 * MIB + 17;
    let cases = [
        (11_u8, 2_u64, 0_u64),
        (14, 8, MIB),
        (15, 12, 2 * MIB),
        (16, 16, source_length),
        (17, 17, source_length),
        (18, 18, source_length),
    ];
    for (kill_id, checkpoint, expected_orphan_bytes) in cases {
        let fixture = Fixture::new();
        fs::write(fixture.source(), vec![0x5a_u8; source_length as usize]).unwrap();
        let ready = fixture.root.join("ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("stream_sigkill_child_entrypoint")
            .arg("--exact")
            .arg("--nocapture")
            .env("MENGXIA_TASK005_STREAM_CHECKPOINT", checkpoint.to_string())
            .env("MENGXIA_TASK005_STREAM_ROOT", &fixture.root)
            .env("MENGXIA_TASK005_STREAM_READY", &ready)
            .spawn()
            .expect("spawn TASK-005 stream crash child");
        wait_for_ready(&mut child, &ready);
        child.kill().expect("SIGKILL TASK-005 stream crash child");
        let _ = child.wait().expect("reap TASK-005 stream crash child");

        let staging = fixture.blob().join(".staging-v1");
        let observed: Vec<_> = fs::read_dir(&staging).unwrap().collect();
        assert_eq!(
            observed.len(),
            usize::from(expected_orphan_bytes != 0),
            "KILL-005-{kill_id:03}"
        );
        if let Some(entry) = observed.first() {
            assert_eq!(
                entry.as_ref().unwrap().metadata().unwrap().len(),
                expected_orphan_bytes,
                "KILL-005-{kill_id:03}"
            );
        }
        assert_eq!(
            fs::read_dir(fixture.blob().join("sha256-v1"))
                .unwrap()
                .count(),
            0,
            "KILL-005-{kill_id:03} must not publish"
        );

        let store = OpenedLibrary::open_or_bootstrap(&store_config(fixture.library()))
            .unwrap_or_else(|error| {
                let mut names: Vec<_> = fs::read_dir(fixture.library())
                    .unwrap()
                    .map(|entry| entry.unwrap().file_name())
                    .collect();
                names.sort_unstable();
                panic!("KILL-005-{kill_id:03} reopen failed with {error}; namespace: {names:?}")
            });
        let config = blob_config(fixture.library(), fixture.blob());
        let authority = store
            .authorize_blob_root(config.blob_root_request())
            .unwrap();
        let (storage, report) = LocalBlobStorage::start(config, authority).unwrap();
        assert_eq!(
            report.staging_orphan_count(),
            u16::from(expected_orphan_bytes != 0),
            "KILL-005-{kill_id:03}"
        );
        assert_eq!(report.staging_orphan_bytes(), expected_orphan_bytes);
        assert_eq!(
            report.ingest_state(),
            if expected_orphan_bytes == 0 {
                BlobIngestState::Ready
            } else {
                BlobIngestState::OrphanReconciliationRequired
            }
        );
        storage.shutdown().unwrap();
        store.shutdown().unwrap();
    }
}

#[test]
fn stream_sigkill_child_entrypoint() {
    let Some(checkpoint) = std::env::var_os("MENGXIA_TASK005_STREAM_CHECKPOINT") else {
        return;
    };
    let checkpoint = checkpoint
        .to_str()
        .expect("ASCII stream checkpoint")
        .parse::<u64>()
        .expect("numeric stream checkpoint");
    let root = PathBuf::from(
        std::env::var_os("MENGXIA_TASK005_STREAM_ROOT").expect("stream fixture root"),
    );
    let ready =
        PathBuf::from(std::env::var_os("MENGXIA_TASK005_STREAM_READY").expect("stream ready path"));
    let library = root.join("Library");
    let blob = library.join("storage");
    let source = root.join("source.bin");
    let store = OpenedLibrary::open_or_bootstrap(&store_config(library.clone())).unwrap();
    let config = blob_config(library, blob);
    let authority = store
        .authorize_blob_root(config.blob_root_request())
        .unwrap();
    let (storage, _) = LocalBlobStorage::start(config, authority).unwrap();
    let opened = storage.open_source(&source).unwrap();
    let result = storage.ingest(
        opened,
        None,
        Arc::new(PauseAt {
            target: checkpoint,
            count: AtomicU64::new(0),
            ready,
        }),
    );
    match result {
        Ok(_) => panic!("stream crash checkpoint was not reached before success"),
        Err(error) => panic!("stream crash checkpoint was not reached: {error}"),
    }
}

fn wait_for_ready(child: &mut std::process::Child, ready: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if ready.is_file() {
            return;
        }
        if let Some(status) = child.try_wait().expect("poll TASK-005 stream child") {
            panic!("TASK-005 stream child exited before acknowledgement: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    panic!("TASK-005 stream crash child timed out");
}
