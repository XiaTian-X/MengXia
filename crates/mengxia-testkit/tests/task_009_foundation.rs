use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("testkit belongs to the workspace")
        .to_path_buf()
}

fn sha256(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[test]
fn accepted_task_009_gate_and_candidate_are_exact() {
    let root = root();
    let proposal =
        fs::read_to_string(root.join("docs/proposals/TASK-009-GATE-PROPOSAL.md")).unwrap();
    let adr = fs::read_to_string(
        root.join("docs/spec/adr/ADR-0012-task-009-creative-ledger-migration.md"),
    )
    .unwrap();
    let candidate = fs::read(root.join("docs/proposals/TASK-009-0002-CANDIDATE.sql")).unwrap();

    for required in [
        "status: \"ACCEPTED_INCORPORATED_BY_CANONICAL_SPECIFICATION_1_1_34\"",
        "TASK009_LIFECYCLE: IN_PROGRESS",
        "TASK009_IMPLEMENTATION_AUTHORITY: TASK_009_ONLY",
        "TASK009_UNRESOLVED_BLOCKING_FINDINGS: NONE",
        "TASK-010+ remains unauthorized",
    ] {
        assert!(
            proposal.contains(required),
            "proposal is missing {required}"
        );
    }
    assert!(adr.contains("- Status: `ACCEPTED`"));
    assert!(adr.contains("TASK-009-GATE-PROPOSAL.md` v0.1.5"));
    assert_eq!(candidate.len(), 18_681);
    assert_eq!(
        sha256(&candidate),
        "dc95fcfee381d07834e14975a0fdacd0874de9c6512c72ff0ac04777e07522d1"
    );
}

#[test]
fn completed_migration_prefix_remains_byte_immutable() {
    let root = root();
    let migration_0000 = fs::read(root.join("migrations/sqlite/0000_store_bootstrap.sql")).unwrap();
    let migration_0001 = fs::read(root.join("migrations/sqlite/0001_library_assets.sql")).unwrap();

    assert_eq!(
        sha256(&migration_0000),
        "35a69e30b627e994a172c9490f391552a8d60212c75ad2f478ea1005c0b94ce2"
    );
    assert_eq!(migration_0001.len(), 12_733);
    assert_eq!(
        sha256(&migration_0001),
        "91c76e615fe248abd852860dcd42b32a01f6f024e91ac8387f34069be2435db1"
    );
}

#[test]
fn protocol_1_2_fixture_and_migration_0002_are_byte_exact() {
    let root = root();
    let fixture = root.join("crates/mengxia-testkit/tests/fixtures/task_008");
    let source_proto = fs::read(root.join("proto/core/v1/handshake.proto")).unwrap();
    let source_descriptor = fs::read(root.join("proto/core/v1/handshake.pb")).unwrap();
    let source_provenance = fs::read(root.join("proto/core/v1/handshake.provenance")).unwrap();

    assert_ne!(
        fs::read(fixture.join("handshake-v1.2.proto")).unwrap(),
        source_proto,
        "protocol 1.3 must not overwrite the immutable 1.2 source fixture"
    );
    assert_ne!(
        fs::read(fixture.join("handshake-v1.2.pb")).unwrap(),
        source_descriptor,
        "protocol 1.3 must not overwrite the immutable 1.2 descriptor fixture"
    );
    assert_ne!(
        fs::read(fixture.join("handshake-v1.2.provenance")).unwrap(),
        source_provenance,
        "protocol 1.3 must carry its own current provenance"
    );
    assert_eq!(
        sha256(&fs::read(fixture.join("handshake-v1.2.proto")).unwrap()),
        "78f52b6c854ed04c9d35fb2533a2135eb443ba2c00f119c5ae596c55307df86f"
    );
    assert_eq!(
        sha256(&fs::read(fixture.join("handshake-v1.2.pb")).unwrap()),
        "8ba247f92ef0fa11656a7490b94607da9dc0b6df88d3c3b887f024ca43dff12c"
    );
    assert_eq!(
        sha256(&source_proto),
        "52f96b1c19ae7b8813e07873ac091fff99c571a5eb2effe085e401c8e9ad25e8"
    );
    assert_eq!(
        sha256(&source_descriptor),
        "f417f10b0a30bb28b234398d9f1a54ef2489c2ee85c512eca68838b5732a9341"
    );
    let source_provenance = String::from_utf8(source_provenance).unwrap();
    for exact in [
        "format=mengxia-proto-provenance-v1",
        "proto_sha256=52f96b1c19ae7b8813e07873ac091fff99c571a5eb2effe085e401c8e9ad25e8",
        "descriptor_sha256=f417f10b0a30bb28b234398d9f1a54ef2489c2ee85c512eca68838b5732a9341",
        "protoc_version=35.1",
        "protoc_artifact_sha256=193289af0470c6a1aada357d4fba0bbf8d78bfaac8b5e42ca30af2ef75583de2",
        "prost_build_version=0.14.4",
    ] {
        assert!(
            source_provenance.lines().any(|line| line == exact),
            "current protocol provenance is missing {exact}"
        );
    }

    let candidate = fs::read(root.join("docs/proposals/TASK-009-0002-CANDIDATE.sql")).unwrap();
    let migration = fs::read(root.join("migrations/sqlite/0002_projects_work.sql")).unwrap();
    assert_eq!(migration, candidate);
    assert_eq!(migration.len(), 18_681);
    assert_eq!(
        sha256(&migration),
        "dc95fcfee381d07834e14975a0fdacd0874de9c6512c72ff0ac04777e07522d1"
    );
}

#[test]
fn task_009_gate_driver_owns_all_twenty_seven_stable_test_ids() {
    let script = fs::read_to_string(root().join("scripts/verify-task-009.sh")).unwrap();
    for test_id in [
        "TEST-MIGRATION-009",
        "TEST-SCHEMA-009",
        "TEST-OUTCOME-009",
        "TEST-REPLAY-009",
        "TEST-EVENT-009",
        "TEST-DOMAIN-009",
        "TEST-JSON-009",
        "TEST-CONFIG-009",
        "TEST-PROTO-009",
        "TEST-CLI-009",
        "TEST-AUTH-009",
        "TEST-PROJECT-009",
        "TEST-SUBJECT-009",
        "TEST-WORK-009",
        "TEST-TAKE-009",
        "TEST-ASSET-LIFECYCLE-009",
        "TEST-CONCURRENCY-009",
        "TEST-PAGINATION-009",
        "TEST-CORRUPTION-009",
        "TEST-RECOVERY-009",
        "TEST-ERROR-009",
        "TEST-OBSERVABILITY-009",
        "TEST-LIFECYCLE-009",
        "TEST-ARCH-009",
        "TEST-SUPPLY-009",
        "TEST-DOC-009",
        "TEST-ENDTOEND-009",
    ] {
        assert_eq!(
            script.matches(&format!("run {test_id} ")).count(),
            1,
            "TASK-009 driver must own {test_id} exactly once"
        );
    }
    assert!(script.contains("component=0"));
    assert!(script.contains("[ \"$component\" -eq 0 ]"));
    assert!(script.contains("task_009_foundation"));
    assert!(script.contains("document_traceability"));
    assert!(script.contains("scripts/verify-task-008.sh formal"));
    assert!(script.contains("scripts/verify-task-008.sh developer"));
    assert!(!script.contains("TASK009_LIFECYCLE: DONE"));
    assert!(!script.contains("TEST-ENDTOEND-009: PASS"));
}

#[test]
fn immutable_snapshot_sqlite_open_is_the_exact_task_009_path_exception() {
    let source = fs::read_to_string(
        root().join("crates/mengxia-store-sqlite/src/migration_snapshot_sqlite.rs"),
    )
    .unwrap();
    assert_eq!(source.matches("SqliteSnapshot::open_with_flags").count(), 1);
    for required in [
        "?mode=ro&immutable=1",
        "SQLITE_OPEN_READ_ONLY",
        "SQLITE_OPEN_URI",
        "SQLITE_OPEN_NOFOLLOW",
        "SQLITE_OPEN_NO_MUTEX",
        "SQLITE_OPEN_PRIVATE_CACHE",
        "SQLITE_OPEN_EXRESCODE",
        "validate_migration_snapshot_manifest",
    ] {
        assert!(
            source.contains(required),
            "snapshot opener lacks {required}"
        );
    }
    assert!(!source.contains("SQLITE_OPEN_CREATE"));
    assert!(!source.contains("SQLITE_OPEN_READ_WRITE"));
}
