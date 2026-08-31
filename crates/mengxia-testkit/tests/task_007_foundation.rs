use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap()
        .to_path_buf()
}

#[test]
fn task_007_proto_descriptor_and_provenance_are_committed() {
    let root = root();
    let proto = fs::read_to_string(root.join("proto/core/v1/handshake.proto")).unwrap();
    let provenance = fs::read_to_string(root.join("proto/core/v1/handshake.provenance")).unwrap();
    assert!(proto.contains("CLIENT_INTENT_SINGLE_COMMAND"));
    assert!(proto.contains("message CoreRequest"));
    assert!(proto.contains("optional RetryAction retry_action = 6"));
    assert!(root.join("proto/core/v1/handshake.pb").is_file());
    assert!(provenance.contains("protoc_version=35.1"));
    assert!(
        provenance.contains(
            "proto_sha256=d54f3f9a4d64b1d2bca58aace2e06ba9a2960f45537db7cfeb9fb55c228e0adb"
        )
    );
    assert!(provenance.contains(
        "descriptor_sha256=a21e4d17c33b8e99d1df544436007c83f6a7285ae75baf9b4666aa265e3b36de"
    ));
}

#[test]
fn task_007_architecture_boundaries_remain_directional() {
    let root = root();
    let app = fs::read_to_string(root.join("crates/mengxia-app/Cargo.toml")).unwrap();
    let core = fs::read_to_string(root.join("crates/mengxia-core-proto/Cargo.toml")).unwrap();
    let cli_cargo = fs::read_to_string(root.join("bins/mengxia/Cargo.toml")).unwrap();
    let daemon_cargo = fs::read_to_string(root.join("bins/mengxiad/Cargo.toml")).unwrap();
    let cli = fs::read_to_string(root.join("bins/mengxia/src/main.rs")).unwrap();
    assert!(!app.contains("tokio"));
    assert!(!app.contains("prost"));
    assert!(!app.contains("rusqlite"));
    assert!(!core.contains("mengxia-app"));
    assert!(!core.contains("mengxia-store-sqlite"));
    assert!(!cli_cargo.contains("mengxia-domain"));
    assert!(!cli_cargo.contains("mengxia-ports"));
    for dependency in [
        "mengxia-domain.workspace = true",
        "mengxia-ports.workspace = true",
        "mengxia-storage-local.workspace = true",
    ] {
        assert!(daemon_cargo.contains(dependency));
    }
    assert!(!cli.contains("rusqlite"));
    assert!(!cli.contains("mengxia-storage-local"));
    assert!(cli.contains("request_single_command"));
}

#[test]
fn task_007_gate_driver_owns_all_nineteen_stable_test_ids() {
    let script = fs::read_to_string(root().join("scripts/verify-task-007.sh")).unwrap();
    for id in [
        "TEST-PROTO-007",
        "TEST-CLI-007",
        "TEST-CONFIG-007",
        "TEST-AUTH-007",
        "TEST-DIGEST-007",
        "TEST-INGEST-007",
        "TEST-SOURCE-007",
        "TEST-CUSTODY-007",
        "TEST-COMMAND-007",
        "TEST-CONCURRENCY-007",
        "TEST-CANCEL-007",
        "TEST-RECOVERY-007",
        "TEST-ROOT-007",
        "TEST-ERROR-007",
        "TEST-LIFECYCLE-007",
        "TEST-ARCH-007",
        "TEST-SUPPLY-007",
        "TEST-DOC-007",
        "TEST-ENDTOEND-007",
    ] {
        assert_eq!(
            script.matches(id).count(),
            1,
            "gate ID {id} must occur once"
        );
        assert!(
            script.contains(&format!("run {id} ")),
            "gate ID {id} must own a non-empty command"
        );
    }
    assert!(script.contains("FAST_PASS"));
    assert!(script.contains("scripts/verify-task-006.sh formal"));
}
