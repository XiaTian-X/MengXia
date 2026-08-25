mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};
use support::{cargo_metadata, parse_packages, workspace_root};

#[test]
fn task_003_dependency_and_authority_boundaries_are_exact() {
    let root = workspace_root();
    let output = cargo_metadata(&root.join("Cargo.toml"), true);
    assert!(
        output.status.success(),
        "locked workspace metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packages = parse_packages(&String::from_utf8(output.stdout).unwrap()).unwrap();

    let client = package_dependencies(&packages, "mengxia");
    assert!(!client.contains("mengxia-store-sqlite"));
    assert!(!client.contains("rusqlite"));
    for required in [
        "mengxia-core-proto",
        "mengxia-framing",
        "mengxia-platform-fs",
        "mengxia-types",
        "tokio",
    ] {
        assert!(
            client.contains(required),
            "missing Client dependency {required}"
        );
    }

    let daemon = package_dependencies(&packages, "mengxiad");
    for required in [
        "mengxia-core-proto",
        "mengxia-framing",
        "mengxia-platform-fs",
        "mengxia-store-sqlite",
        "mengxia-types",
        "tokio",
    ] {
        assert!(
            daemon.contains(required),
            "missing daemon dependency {required}"
        );
    }

    for leaf in [
        "mengxia-core-proto",
        "mengxia-framing",
        "mengxia-platform-fs",
    ] {
        let dependencies = package_dependencies(&packages, leaf);
        for forbidden in ["mengxia-store-sqlite", "rusqlite", "mengxia-storage-local"] {
            assert!(
                !dependencies.contains(forbidden),
                "{leaf} gained persistence authority through {forbidden}"
            );
        }
    }

    let client_source = fs::read_to_string(root.join("bins/mengxia/src/main.rs")).unwrap();
    let daemon_source = fs::read_to_string(root.join("bins/mengxiad/src/main.rs")).unwrap();
    let proto_source =
        fs::read_to_string(root.join("crates/mengxia-core-proto/src/lib.rs")).unwrap();
    for (name, source) in [
        ("Client", client_source.as_str()),
        ("daemon", daemon_source.as_str()),
        ("protocol", proto_source.as_str()),
    ] {
        for forbidden in [
            "TcpListener",
            "TcpStream",
            "rusqlite",
            "CommandRecord",
            "AdminSession",
            "Provider",
            "Plugin",
        ] {
            assert!(!source.contains(forbidden), "{name} contains {forbidden}");
        }
        assert!(!source.contains("unsafe {"), "{name} contains unsafe code");
    }

    let store = fs::read_to_string(root.join("crates/mengxia-store-sqlite/src/lib.rs")).unwrap();
    assert!(store.contains("pub struct OpenedLibrary"));
    assert!(store.contains("pub struct OpenedLibraryIdentity"));
    for forbidden in [
        "pub fn connection",
        "pub fn path",
        "pub fn lock",
        "pub fn submit",
        "pub use lifecycle",
    ] {
        assert!(
            !store.contains(forbidden),
            "opaque seam leaked: {forbidden}"
        );
    }

    let daemon_production = daemon_source
        .split("#[cfg(test)]")
        .next()
        .expect("daemon has a production prefix");
    for forbidden in [
        "MENGXIA_TASK003_TEST_ROLE",
        "MENGXIA_TASK003_TEST_ENDPOINT",
        "mengxia-task003-ci",
    ] {
        assert!(
            !daemon_production.contains(forbidden),
            "formal fixture leaked into production: {forbidden}"
        );
        assert!(!client_source.contains(forbidden));
    }

    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    for exact in [
        "task-003-second-uid:",
        "runs-on: macos-26",
        "scripts/verify-task-003-formal-second-uid.sh",
    ] {
        assert!(workflow.contains(exact), "formal CI is missing {exact}");
    }
}

#[test]
fn descriptor_and_offline_generator_inputs_are_source_pinned() {
    let root = workspace_root();
    assert_sha256(
        &root.join("proto/core/v1/handshake.proto"),
        "ab86851284a9627718c408df76da8e82388f2273ee06fe67d0b46da645fc86c7",
    );
    assert_sha256(
        &root.join("proto/core/v1/handshake.pb"),
        "5a5c995f0a61ee001be44b6be08ee2dba0a730371ead52b8c4a6232acf7d3898",
    );
    let provenance = fs::read_to_string(root.join("proto/core/v1/handshake.provenance")).unwrap();
    for exact in [
        "format=mengxia-proto-provenance-v1",
        "protoc_version=35.1",
        "protoc_artifact=protoc-35.1-osx-aarch_64.zip",
        "protoc_artifact_sha256=193289af0470c6a1aada357d4fba0bbf8d78bfaac8b5e42ca30af2ef75583de2",
        "prost_build_version=0.14.4",
    ] {
        assert!(
            provenance.lines().any(|line| line == exact),
            "missing {exact}"
        );
    }

    let build = fs::read_to_string(root.join("crates/mengxia-core-proto/build.rs")).unwrap();
    assert!(build.contains("compile_fds"));
    for forbidden in [
        "Command::new",
        "std::process",
        "PROTOC_INCLUDE",
        "env::var(\"PROTOC\")",
        "download",
    ] {
        assert!(
            !build.contains(forbidden),
            "ambient generator path: {forbidden}"
        );
    }
}

#[test]
fn task_003_verification_scripts_have_the_exact_owned_mapping_shape() {
    let root = workspace_root();
    let developer = fs::read_to_string(root.join("scripts/verify-task-003.sh")).unwrap();
    let formal =
        fs::read_to_string(root.join("scripts/verify-task-003-formal-second-uid.sh")).unwrap();
    let runner = fs::read_to_string(root.join("scripts/run-task-003-second-uid.sh")).unwrap();
    assert_eq!(developer.matches("task003_run TEST-").count(), 10);
    assert_eq!(
        formal.matches("task003_run TEST-IPC-MACOS-001 --").count(),
        1
    );
    assert_eq!(formal.matches("./scripts/verify-task-003.sh").count(), 1);
    assert!(
        formal.contains("task003_run TEST-IPC-MACOS-001 -- ./scripts/run-task-003-second-uid.sh")
    );
    assert!(runner.contains("mengxia-task003-ci"));
    assert!(runner.contains("600"));
    assert!(runner.contains("699"));
    assert!(!developer.contains("TEST-IPC-MACOS-001: PASS"));
}

fn package_dependencies<'a>(packages: &'a [support::Package], name: &str) -> BTreeSet<&'a str> {
    packages
        .iter()
        .find(|package| package.name == name)
        .unwrap_or_else(|| panic!("missing package {name}"))
        .dependencies
        .iter()
        .map(String::as_str)
        .collect()
}

fn assert_sha256(path: &Path, expected: &str) {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let actual =
        Sha256::digest(bytes)
            .iter()
            .fold(String::with_capacity(64), |mut output, byte| {
                use std::fmt::Write;
                write!(&mut output, "{byte:02x}").unwrap();
                output
            });
    assert_eq!(actual, expected, "digest drift: {}", path.display());
}
