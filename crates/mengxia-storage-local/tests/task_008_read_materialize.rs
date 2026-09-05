use std::fs;
use std::future::Future;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll, Waker};

use mengxia_ports::{
    ASSET_MATERIALIZE_V1, AssetStoreError, BlobStorage, Command, CommandBinding, IngestControl,
    IngestDirective, IngestOutcome, IntegrityIssueKind, MaterializationCommandBinding,
    MaterializationEffectRequest, MaterializationStoragePort, RegisteredBlobVerificationCandidate,
    RegisteredBlobVerificationPort, ResolvedManagedMember, VerificationMode,
};
use mengxia_storage_local::{BlobConfigSource, LocalBlobStorage, ResolvedBlobStorageConfig};
use mengxia_store_sqlite::{ConfigSource, OpenedLibrary, ResolvedStoreConfig};
use mengxia_types::{Id, RevisionNo, Sha256Digest};
use sha2::{Digest as _, Sha256};

static NEXT: AtomicU64 = AtomicU64::new(0);
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;

struct Continue;

impl IngestControl for Continue {
    fn checkpoint(&self) -> IngestDirective {
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
            .expect("workspace root")
            .join("target/task-008-read-materialize")
            .join(format!(
                "{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&root)
            .expect("fixture root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("fixture mode");
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
        .expect("store config")
    }

    fn blob_config(&self) -> mengxia_storage_local::BlobStorageConfig {
        let source = BlobConfigSource::CompiledDefault;
        ResolvedBlobStorageConfig::from_selected(
            Some(self.library_root()),
            BlobConfigSource::Cli,
            Some(self.blob_root()),
            source,
            Some("2".into()),
            source,
            Some("2".into()),
            source,
            Some("2".into()),
            source,
            Some(MIB.to_string()),
            source,
            Some(GIB.to_string()),
            source,
            Some((2 * GIB).to_string()),
            source,
            Some((10 * GIB).to_string()),
            source,
            Some("5".into()),
            source,
        )
        .validate()
        .expect("blob config")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn start(fixture: &Fixture) -> (OpenedLibrary, LocalBlobStorage) {
    let store = OpenedLibrary::open_or_bootstrap(&fixture.store_config()).expect("store");
    let config = fixture.blob_config();
    let authority = store
        .authorize_blob_root(config.blob_root_request())
        .expect("blob authority");
    let (storage, _) = LocalBlobStorage::start(config, authority).expect("blob storage");
    (store, storage)
}

fn candidate(
    digest: Sha256Digest,
    length: u64,
    backend: &str,
    locator: &str,
) -> RegisteredBlobVerificationCandidate {
    RegisteredBlobVerificationCandidate::__from_store(
        digest,
        length,
        RevisionNo::new(1),
        Id::try_new().expect("location id"),
        RevisionNo::new(1),
        backend.to_owned(),
        locator.to_owned(),
    )
    .expect("candidate")
}

fn block_on_ready<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("verification future unexpectedly pending"),
    }
}

#[test]
fn normal_and_deep_verification_classify_registered_blob_without_mutation() {
    let fixture = Fixture::new();
    let bytes = b"TASK-008 registered Blob verification";
    let source_path = fixture.root.join("source.bin");
    fs::write(&source_path, bytes).expect("source");
    let digest = Sha256Digest::from_bytes(Sha256::digest(bytes).into());
    let (store, storage) = start(&fixture);
    let outcome = storage
        .ingest(
            storage.open_source(&source_path).expect("source authority"),
            Some(digest),
            Arc::new(Continue),
        )
        .expect("ingest");
    let IngestOutcome::Stored(blob) = outcome else {
        panic!("stored outcome");
    };
    let backend = blob.location().backend_id().to_owned();
    let locator = blob.location().locator().to_owned();

    assert!(
        block_on_ready(storage.verify_registered_blob(
            candidate(digest, bytes.len() as u64, &backend, &locator),
            VerificationMode::Normal,
        ))
        .expect("normal verify")
        .is_none()
    );
    assert!(
        block_on_ready(storage.verify_registered_blob(
            candidate(digest, bytes.len() as u64, &backend, &locator),
            VerificationMode::Deep,
        ))
        .expect("deep verify")
        .is_none()
    );

    let wrong_backend = block_on_ready(storage.verify_registered_blob(
        candidate(
            digest,
            bytes.len() as u64,
            "mengxia.local-cas.v1/invalid",
            &locator,
        ),
        VerificationMode::Normal,
    ))
    .expect("backend finding")
    .expect("backend mismatch");
    assert_eq!(
        wrong_backend.kind(),
        IntegrityIssueKind::LocalBackendMismatch
    );

    let wrong_locator = block_on_ready(storage.verify_registered_blob(
        candidate(digest, bytes.len() as u64, &backend, "sha256-v1/invalid"),
        VerificationMode::Normal,
    ))
    .expect("locator finding")
    .expect("unsafe locator");
    assert_eq!(wrong_locator.kind(), IntegrityIssueKind::ManagedBlobUnsafe);

    let canonical = fixture.blob_root().join(&locator);
    fs::write(&canonical, vec![b'x'; bytes.len()]).expect("same-length tamper");
    assert!(
        block_on_ready(storage.verify_registered_blob(
            candidate(digest, bytes.len() as u64, &backend, &locator),
            VerificationMode::Normal,
        ))
        .expect("normal metadata verify")
        .is_none()
    );
    let digest_mismatch = block_on_ready(storage.verify_registered_blob(
        candidate(digest, bytes.len() as u64, &backend, &locator),
        VerificationMode::Deep,
    ))
    .expect("deep finding")
    .expect("digest mismatch");
    assert_eq!(
        digest_mismatch.kind(),
        IntegrityIssueKind::ManagedBlobDigestMismatch
    );

    fs::write(&canonical, b"short").expect("length tamper");
    let length_mismatch = block_on_ready(storage.verify_registered_blob(
        candidate(digest, bytes.len() as u64, &backend, &locator),
        VerificationMode::Normal,
    ))
    .expect("length finding")
    .expect("length mismatch");
    assert_eq!(
        length_mismatch.kind(),
        IntegrityIssueKind::ManagedBlobLengthMismatch
    );

    fs::set_permissions(&canonical, fs::Permissions::from_mode(0o644)).expect("unsafe mode");
    let unsafe_blob = block_on_ready(storage.verify_registered_blob(
        candidate(digest, 5, &backend, &locator),
        VerificationMode::Normal,
    ))
    .expect("unsafe finding")
    .expect("unsafe blob");
    assert_eq!(unsafe_blob.kind(), IntegrityIssueKind::ManagedBlobUnsafe);
    fs::set_permissions(&canonical, fs::Permissions::from_mode(0o600)).expect("restore mode");

    fs::remove_file(&canonical).expect("remove canonical");
    let missing = block_on_ready(storage.verify_registered_blob(
        candidate(digest, bytes.len() as u64, &backend, &locator),
        VerificationMode::Normal,
    ))
    .expect("missing finding")
    .expect("missing blob");
    assert_eq!(missing.kind(), IntegrityIssueKind::ManagedBlobMissing);

    storage.shutdown().expect("storage shutdown");
    store.shutdown().expect("store shutdown");
}

#[test]
fn exact_managed_member_materializes_no_replace_then_cleans_intent() {
    let fixture = Fixture::new();
    let bytes = b"TASK-008 exact physical materialization";
    let source_path = fixture.root.join("source.bin");
    fs::write(&source_path, bytes).expect("source");
    let digest = Sha256Digest::from_bytes(Sha256::digest(bytes).into());
    let (store, storage) = start(&fixture);
    let outcome = storage
        .ingest(
            storage.open_source(&source_path).expect("source authority"),
            Some(digest),
            Arc::new(Continue),
        )
        .expect("ingest");
    let IngestOutcome::Stored(blob) = outcome else {
        panic!("stored outcome");
    };
    let asset_id = Id::try_new().unwrap();
    let revision_id = Id::try_new().unwrap();
    let representation_id = Id::try_new().unwrap();
    let resource_id = Id::try_new().unwrap();
    let command_id = Id::<Command>::try_new().unwrap();
    let command = MaterializationCommandBinding::new(
        CommandBinding::new(
            command_id,
            ASSET_MATERIALIZE_V1,
            Sha256Digest::from_bytes([0x77; 32]),
        ),
        asset_id,
        revision_id,
        representation_id,
        resource_id,
        0,
    )
    .unwrap();
    let member = ResolvedManagedMember::__from_store(
        asset_id,
        revision_id,
        representation_id,
        resource_id,
        0,
        digest,
        bytes.len() as u64,
        Id::try_new().unwrap(),
        blob.location().backend_id().to_owned(),
        blob.location().locator().to_owned(),
    )
    .unwrap();
    let output = fixture.root.join("output");
    fs::DirBuilder::new().mode(0o700).create(&output).unwrap();
    let final_path = output.join("copy.bin");
    let request = MaterializationEffectRequest::new(
        command,
        member,
        final_path.as_os_str().as_encoded_bytes().to_vec(),
    )
    .unwrap();
    let mut prepared = block_on_ready(storage.prepare_materialization(request)).unwrap();
    assert!(!final_path.exists());
    let mut published = block_on_ready(prepared.publish()).unwrap();
    assert_eq!(fs::read(&final_path).unwrap(), bytes);
    assert_eq!(
        fs::metadata(&final_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let command_hex = command_id.to_string().replace('-', "");
    let intent = output.join(format!(".mengxia-materialize-{command_hex}.intent"));
    let staging = output.join(format!(".mengxia-materialize-{command_hex}.staging"));
    assert!(intent.exists());
    assert!(!staging.exists());
    block_on_ready(published.cleanup()).unwrap();
    assert!(!intent.exists());
    assert_eq!(fs::read(&final_path).unwrap(), bytes);

    let second_command = MaterializationCommandBinding::new(
        CommandBinding::new(
            Id::<Command>::try_new().unwrap(),
            ASSET_MATERIALIZE_V1,
            Sha256Digest::from_bytes([0x78; 32]),
        ),
        asset_id,
        revision_id,
        representation_id,
        resource_id,
        0,
    )
    .unwrap();
    let second_member = ResolvedManagedMember::__from_store(
        asset_id,
        revision_id,
        representation_id,
        resource_id,
        0,
        digest,
        bytes.len() as u64,
        Id::try_new().unwrap(),
        blob.location().backend_id().to_owned(),
        blob.location().locator().to_owned(),
    )
    .unwrap();
    assert_eq!(
        block_on_ready(
            storage.prepare_materialization(
                MaterializationEffectRequest::new(
                    second_command,
                    second_member,
                    final_path.as_os_str().as_encoded_bytes().to_vec(),
                )
                .unwrap(),
            )
        )
        .err(),
        Some(AssetStoreError::Conflict)
    );
    assert_eq!(fs::read(&final_path).unwrap(), bytes);
    storage.shutdown().expect("storage shutdown");
    store.shutdown().expect("store shutdown");
}

#[test]
fn exact_recovery_classifies_published_prefix_before_resuming_and_cleanup() {
    let fixture = Fixture::new();
    let bytes = b"TASK-008 published recovery";
    let source_path = fixture.root.join("source.bin");
    fs::write(&source_path, bytes).expect("source");
    let digest = Sha256Digest::from_bytes(Sha256::digest(bytes).into());
    let (store, storage) = start(&fixture);
    let outcome = storage
        .ingest(
            storage.open_source(&source_path).expect("source authority"),
            Some(digest),
            Arc::new(Continue),
        )
        .expect("ingest");
    let IngestOutcome::Stored(blob) = outcome else {
        panic!("stored outcome");
    };
    let asset_id = Id::try_new().unwrap();
    let revision_id = Id::try_new().unwrap();
    let representation_id = Id::try_new().unwrap();
    let resource_id = Id::try_new().unwrap();
    let command_id = Id::<Command>::try_new().unwrap();
    let command = MaterializationCommandBinding::new(
        CommandBinding::new(
            command_id,
            ASSET_MATERIALIZE_V1,
            Sha256Digest::from_bytes([0x79; 32]),
        ),
        asset_id,
        revision_id,
        representation_id,
        resource_id,
        0,
    )
    .unwrap();
    let member = || {
        ResolvedManagedMember::__from_store(
            asset_id,
            revision_id,
            representation_id,
            resource_id,
            0,
            digest,
            bytes.len() as u64,
            Id::try_new().unwrap(),
            blob.location().backend_id().to_owned(),
            blob.location().locator().to_owned(),
        )
        .unwrap()
    };
    let output = fixture.root.join("output");
    fs::DirBuilder::new().mode(0o700).create(&output).unwrap();
    let final_path = output.join("copy.bin");
    let destination = final_path.as_os_str().as_encoded_bytes().to_vec();
    let mut prepared = block_on_ready(storage.prepare_materialization(
        MaterializationEffectRequest::new(command, member(), destination.clone()).unwrap(),
    ))
    .unwrap();
    let published = block_on_ready(prepared.publish()).unwrap();
    drop(published);
    let command_hex = command_id.to_string().replace('-', "");
    let intent = output.join(format!(".mengxia-materialize-{command_hex}.intent"));
    assert!(intent.exists());
    assert_eq!(fs::read(&final_path).unwrap(), bytes);

    let mut recovered = block_on_ready(storage.prepare_materialization_recovery(
        MaterializationEffectRequest::new(command, member(), destination.clone()).unwrap(),
    ))
    .unwrap();
    assert!(intent.exists());
    assert_eq!(fs::read(&final_path).unwrap(), bytes);
    let mut published = block_on_ready(recovered.publish()).unwrap();
    assert_eq!(fs::read(&final_path).unwrap(), bytes);
    block_on_ready(published.cleanup()).unwrap();
    assert!(!intent.exists());

    block_on_ready(storage.cleanup_completed_materialization(
        MaterializationEffectRequest::new(command, member(), destination).unwrap(),
    ))
    .unwrap();
    assert_eq!(fs::read(&final_path).unwrap(), bytes);

    let held_request = |suffix: u8, name: &str| {
        let command = MaterializationCommandBinding::new(
            CommandBinding::new(
                Id::<Command>::try_new().unwrap(),
                ASSET_MATERIALIZE_V1,
                Sha256Digest::from_bytes([suffix; 32]),
            ),
            asset_id,
            revision_id,
            representation_id,
            resource_id,
            0,
        )
        .unwrap();
        MaterializationEffectRequest::new(
            command,
            member(),
            output.join(name).as_os_str().as_encoded_bytes().to_vec(),
        )
        .unwrap()
    };
    let held_one =
        block_on_ready(storage.prepare_materialization(held_request(0x81, "one.bin"))).unwrap();
    let held_two =
        block_on_ready(storage.prepare_materialization(held_request(0x82, "two.bin"))).unwrap();
    assert_eq!(
        block_on_ready(storage.prepare_materialization(held_request(0x83, "three.bin"))).err(),
        Some(AssetStoreError::Backpressure)
    );
    drop(held_one);
    let retry =
        block_on_ready(storage.prepare_materialization(held_request(0x84, "four.bin"))).unwrap();
    drop(retry);
    drop(held_two);
    storage.shutdown().expect("storage shutdown");
    store.shutdown().expect("store shutdown");
}
