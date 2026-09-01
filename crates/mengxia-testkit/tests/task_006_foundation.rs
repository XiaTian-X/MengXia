use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

#[test]
fn task_006_dependency_and_architecture_boundaries_are_exact() {
    let root = repository_root();
    for crate_name in [
        "mengxia-app",
        "mengxia-domain",
        "mengxia-events",
        "mengxia-ports",
    ] {
        let cargo =
            fs::read_to_string(root.join(format!("crates/{crate_name}/Cargo.toml"))).unwrap();
        assert!(
            !cargo.contains("rusqlite"),
            "{crate_name} must not own SQLite"
        );
        assert!(
            !cargo.contains("async-trait"),
            "{crate_name} must use the exact boxed-future port"
        );
        let source =
            fs::read_to_string(root.join(format!("crates/{crate_name}/src/lib.rs"))).unwrap();
        assert!(!source.contains("rusqlite"));
        assert!(!source.contains("tokio::sync"));
    }

    let store = fs::read_to_string(root.join("crates/mengxia-store-sqlite/src/lib.rs")).unwrap();
    let repository =
        fs::read_to_string(root.join("crates/mengxia-store-sqlite/src/asset_repository.rs"))
            .unwrap();
    let repository_production = repository.split("#[cfg(test)]").next().unwrap();
    let ports = fs::read_to_string(root.join("crates/mengxia-ports/src/lib.rs")).unwrap();
    let app = fs::read_to_string(root.join("crates/mengxia-app/src/asset_persistence.rs")).unwrap();
    let events = fs::read_to_string(root.join("crates/mengxia-events/src/lib.rs")).unwrap();
    let lifecycle =
        fs::read_to_string(root.join("crates/mengxia-store-sqlite/src/lifecycle.rs")).unwrap();
    let migration =
        fs::read_to_string(root.join("crates/mengxia-store-sqlite/src/migration.rs")).unwrap();

    assert!(store.contains("pub fn asset_store_handle(&self) -> SqliteAssetStoreHandle"));
    assert!(ports.contains("pub trait AssetUnitOfWork: Send + Sync"));
    assert!(ports.contains("pub type AssetPortFuture<'a, T>"));
    assert!(ports.contains("pub const ASSET_INGEST_COPY_V1"));
    assert!(ports.contains("pub const ASSET_REVISION_CREATE_V1"));
    assert!(ports.contains("pub const BLOB_LOCATION_RECORD_V1"));
    assert!(!repository_production.contains("__from_verified_local_adapter"));
    assert!(!repository_production.contains("PathBuf"));
    assert!(!repository_production.contains("source_path"));
    assert!(app.contains("pub(crate) struct AssetPersistenceService"));
    assert!(app.contains("pub(crate) trait AssetIdentitySource"));
    assert!(app.contains("pub(crate) trait Clock"));
    assert!(events.contains("pub enum DomainEvent {}"));
    assert!(events.contains("pub enum ProvenanceEvent {}"));
    assert!(!events.contains("DomainEventIdentity"));
    assert!(!events.contains("ProvenanceEventIdentity"));
    assert!(repository.contains("fn validate_command_row("));
    assert!(repository.contains("fn validate_known_command_matrix("));
    assert!(repository.contains("fn replay_external_result("));
    assert!(
        lifecycle.contains("verify_current_library_connection_metadata(&connection, metadata)?;")
    );
    let prepare = migration
        .split("pub(crate) fn prepare_current_library_schema")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn verify_current_library_connection_metadata")
        .next()
        .unwrap();
    assert!(!prepare.contains("verify_quick_check"));
    assert!(!prepare.contains("verify_current_schema_allowlist"));
}

#[test]
fn task_006_migration_bytes_and_sqlite_ownership_are_frozen() {
    let root = repository_root();
    let migration = fs::read(root.join("migrations/sqlite/0001_library_assets.sql")).unwrap();
    assert_eq!(migration.len(), 12_733);
    let digest: [u8; 32] = Sha256::digest(&migration).into();
    assert_eq!(
        digest,
        [
            0x91, 0xc7, 0x6e, 0x61, 0x5f, 0xe2, 0x48, 0xab, 0xd8, 0x52, 0x86, 0x0d, 0xcd, 0x42,
            0xb3, 0x2a, 0x01, 0xf6, 0xf0, 0x24, 0xe9, 0x1a, 0xc8, 0x38, 0x7f, 0x34, 0x06, 0x9b,
            0xe2, 0x43, 0x5d, 0xb1,
        ]
    );
    for table in [
        "commands",
        "assets",
        "asset_revisions",
        "asset_revision_parents",
        "representations",
        "resources",
        "blobs",
        "resource_members",
        "locations",
        "provenance_events",
        "domain_events",
        "event_commit_sequence",
    ] {
        let needle = format!("CREATE TABLE {table} (");
        assert_eq!(
            String::from_utf8_lossy(&migration).matches(&needle).count(),
            1,
            "table {table}"
        );
    }
}

#[test]
fn task_006_gate_driver_owns_all_fourteen_stable_test_ids() {
    let script = fs::read_to_string(repository_root().join("scripts/verify-task-006.sh")).unwrap();
    for id in [
        "TEST-DOMAIN-006",
        "TEST-MAPPER-006",
        "TEST-MIGRATION-006",
        "TEST-SCHEMA-006",
        "TEST-COMMAND-006",
        "TEST-CONCURRENCY-006",
        "TEST-EVENT-006",
        "TEST-CUSTODY-006",
        "TEST-ERROR-006",
        "TEST-RECOVERY-006",
        "TEST-LIFECYCLE-006",
        "TEST-ARCH-006",
        "TEST-SUPPLY-006",
        "TEST-DOC-006",
    ] {
        assert!(script.contains(id), "missing stable gate ID {id}");
    }
    assert!(script.contains("FAST_PASS"));
    assert!(script.contains("scripts/verify-task-005.sh developer"));
    assert!(script.contains("component) component=1"));
}
