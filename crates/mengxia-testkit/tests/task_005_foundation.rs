use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

#[test]
fn task_005_dependency_and_architecture_boundaries_are_exact() {
    let root = repository_root();
    let cargo = fs::read_to_string(root.join("crates/mengxia-storage-local/Cargo.toml")).unwrap();
    for dependency in [
        "getrandom.workspace = true",
        "mengxia-platform-fs.workspace = true",
        "mengxia-ports.workspace = true",
        "mengxia-types.workspace = true",
        "rustix.workspace = true",
        "sha2.workspace = true",
    ] {
        assert!(
            cargo.contains(dependency),
            "missing exact dependency {dependency}"
        );
    }
    for forbidden in ["rusqlite", "tokio", "serde", "async-trait"] {
        assert!(
            !cargo.contains(forbidden),
            "forbidden storage dependency {forbidden}"
        );
    }
    let platform =
        fs::read_to_string(root.join("crates/mengxia-platform-fs/src/blob_storage.rs")).unwrap();
    let storage = fs::read_to_string(root.join("crates/mengxia-storage-local/src/lib.rs")).unwrap();
    let ports = fs::read_to_string(root.join("crates/mengxia-ports/src/lib.rs")).unwrap();
    assert!(!platform.contains("unsafe {"));
    assert!(!storage.contains("unsafe {"));
    assert!(ports.contains("pub trait BlobStorage: Send + Sync"));
    assert!(storage.contains("type Source = OpenedLocalSource"));
    for forbidden in [
        "IngestAsset",
        "Managed",
        "Purge",
        "GarbageCollect",
        "LocationRow",
    ] {
        assert!(
            !storage.contains(forbidden),
            "later-task symbol leaked: {forbidden}"
        );
        assert!(
            !platform.contains(forbidden),
            "later-task symbol leaked: {forbidden}"
        );
    }
}

#[test]
fn trusted_durable_blob_constructor_has_one_local_adapter_call_site() {
    let root = repository_root();
    let mut call_sites = Vec::new();
    for path in [
        "crates/mengxia-storage-local/src/lib.rs",
        "crates/mengxia-store-sqlite/src/lib.rs",
        "crates/mengxia-app/src/lib.rs",
        "crates/mengxia-domain/src/lib.rs",
    ] {
        let source = fs::read_to_string(root.join(path)).unwrap();
        let count = source.matches("__from_verified_local_adapter").count();
        call_sites.extend(std::iter::repeat_n(path, count));
    }
    assert_eq!(call_sites, ["crates/mengxia-storage-local/src/lib.rs"]);
}

#[test]
fn durable_commit_precedes_result_construction_and_terminal_reply() {
    let storage =
        fs::read_to_string(repository_root().join("crates/mengxia-storage-local/src/lib.rs"))
            .unwrap();
    let process_start = storage.find("fn process_ingest(").unwrap();
    let process_end = storage[process_start..]
        .find("\nfn hash_worker(")
        .map(|offset| process_start + offset)
        .unwrap();
    let process = &storage[process_start..process_end];
    assert!(
        process.find(".commit_staging(").unwrap()
            < process
                .find("DurableBlob::__from_verified_local_adapter")
                .unwrap(),
        "KILL-005-030 requires durable commit before constructing success"
    );

    let worker_start = storage.find("fn io_worker(").unwrap();
    let worker_end = storage[worker_start..]
        .find("\ntype ProcessResult")
        .map(|offset| worker_start + offset)
        .unwrap();
    let worker = &storage[worker_start..worker_end];
    assert!(
        worker.find("process_ingest(").unwrap() < worker.find("reply.send(result)").unwrap(),
        "KILL-005-030 requires process completion before the terminal reply"
    );
}

#[test]
fn worker_and_channel_fault_wiring_is_failed_closed_joined_and_cleanup_owned() {
    let storage =
        fs::read_to_string(repository_root().join("crates/mengxia-storage-local/src/lib.rs"))
            .unwrap();

    let ingest_start = storage.find("fn ingest(").unwrap();
    let ingest_end = storage[ingest_start..]
        .find("\n    }\n}")
        .map(|offset| ingest_start + offset)
        .unwrap();
    let ingest = &storage[ingest_start..ingest_end];
    let dispatch_failure = ingest.find("dispatch_admitted(").unwrap();
    assert!(
        ingest[dispatch_failure..].contains("release_admission(&self.shared"),
        "FAULT-005-065 must return ownership and release the admitted reservation"
    );
    assert!(ingest[dispatch_failure..].contains("self.shared.fail()"));
    assert!(ingest.contains("response.recv().unwrap_or_else"));
    assert!(ingest.contains("Err(BlobStorageError::Internal)"));

    let worker_start = storage.find("fn io_worker(").unwrap();
    let worker_end = storage[worker_start..]
        .find("\nfn dispatch_admitted")
        .map(|offset| worker_start + offset)
        .unwrap();
    let worker = &storage[worker_start..worker_end];
    assert!(worker.contains("catch_worker_process"));
    let panic_branch = worker.find("Err(_) =>").unwrap();
    assert!(worker[panic_branch..].contains("shared.fail()"));
    assert!(worker[panic_branch..].contains("release_admission(&shared, &admission, 0, true)"));

    let process_start = storage.find("fn process_ingest(").unwrap();
    let process_end = storage[process_start..]
        .find("\nfn hash_worker(")
        .map(|offset| process_start + offset)
        .unwrap();
    let process = &storage[process_start..process_end];
    assert!(process.contains("hash_result.recv()"));
    assert!(process.contains("staging.take()"));
    assert!(process.contains("BlobStorageError::Internal"));

    let shutdown_start = storage.find("fn shutdown_inner(").unwrap();
    let shutdown_end = storage[shutdown_start..]
        .find("\n    fn admit(")
        .map(|offset| shutdown_start + offset)
        .unwrap();
    let shutdown = &storage[shutdown_start..shutdown_end];
    assert!(
        shutdown.find("join_workers").unwrap() < shutdown.find("sync_for_shutdown").unwrap(),
        "FAULT-005-068 must join every worker before final authority sync/drop"
    );
}

#[test]
fn task_005_gate_driver_owns_all_seventeen_stable_test_ids() {
    let script = fs::read_to_string(repository_root().join("scripts/verify-task-005.sh")).unwrap();
    for id in [
        "TEST-CONFIG-005",
        "TEST-NAMESPACE-005",
        "TEST-PATH-005",
        "TEST-SOURCE-005",
        "TEST-STREAM-005",
        "TEST-CONTROL-005",
        "TEST-RESOURCE-005",
        "TEST-PROMOTE-005",
        "TEST-LOCATION-005",
        "TEST-RECOVERY-005",
        "TEST-ORPHAN-005",
        "TEST-CONCURRENCY-005",
        "TEST-ERROR-005",
        "TEST-LIFECYCLE-005",
        "TEST-ARCH-005",
        "TEST-SUPPLY-005",
        "TEST-DOC-005",
    ] {
        assert!(script.contains(id), "missing stable gate ID {id}");
    }
    assert!(script.contains("FAST_PASS"));
    assert!(script.contains("scripts/verify-task-003.sh"));
    assert!(script.contains(concat!(
        "/usr/bin/env -u MENGXIA_ACL_BUILD_CLASS \\\n",
        "    cargo clippy --locked --offline -p mengxia-platform-fs"
    )));
}
