use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

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
    assert!(proto.contains("CLIENT_INTENT_HANDSHAKE_ONLY = 0;"));
    assert!(proto.contains("CLIENT_INTENT_SINGLE_COMMAND = 1;"));
    assert!(proto.contains("message CoreRequest"));
    assert!(proto.contains("optional RetryAction retry_action = 6"));
    assert_sha256(
        &root.join("proto/core/v1/handshake.proto"),
        "a3f8cdb3cff78a4b73654310a38e5e54db51837afde8924315e07cd656138177",
    );
    assert_sha256(
        &root.join("proto/core/v1/handshake.pb"),
        "7b058e1026c1447943a45c9830105104b87e4730b7473a440b6583a065cd2d08",
    );
    assert!(provenance.contains("protoc_version=35.1"));
    assert!(
        provenance.contains(
            "proto_sha256=a3f8cdb3cff78a4b73654310a38e5e54db51837afde8924315e07cd656138177"
        )
    );
    assert!(provenance.contains(
        "descriptor_sha256=7b058e1026c1447943a45c9830105104b87e4730b7473a440b6583a065cd2d08"
    ));
}

fn assert_sha256(path: &Path, expected: &str) {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let actual =
        Sha256::digest(bytes)
            .iter()
            .fold(String::with_capacity(64), |mut output, byte| {
                use std::fmt::Write as _;
                write!(&mut output, "{byte:02x}").unwrap();
                output
            });
    assert_eq!(actual, expected, "digest drift: {}", path.display());
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
