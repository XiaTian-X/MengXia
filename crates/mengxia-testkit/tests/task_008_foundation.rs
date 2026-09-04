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

fn sha256(path: &Path) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in
        Sha256::digest(fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display())))
    {
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[test]
fn task_008_protocol_1_2_is_exact_and_retains_1_1_fixture() {
    let root = root();
    let fixture = root.join("crates/mengxia-testkit/tests/fixtures/task_007");
    assert_eq!(
        sha256(&fixture.join("handshake-v1.1.proto")),
        "a3f8cdb3cff78a4b73654310a38e5e54db51837afde8924315e07cd656138177"
    );
    assert_eq!(
        sha256(&fixture.join("handshake-v1.1.pb")),
        "7b058e1026c1447943a45c9830105104b87e4730b7473a440b6583a065cd2d08"
    );
    let fixture_provenance = fs::read_to_string(fixture.join("handshake-v1.1.provenance")).unwrap();
    assert!(
        fixture_provenance.contains(
            "proto_sha256=a3f8cdb3cff78a4b73654310a38e5e54db51837afde8924315e07cd656138177"
        )
    );
    assert!(fixture_provenance.contains(
        "descriptor_sha256=7b058e1026c1447943a45c9830105104b87e4730b7473a440b6583a065cd2d08"
    ));

    let proto_path = root.join("proto/core/v1/handshake.proto");
    let descriptor_path = root.join("proto/core/v1/handshake.pb");
    assert_eq!(
        sha256(&proto_path),
        "78f52b6c854ed04c9d35fb2533a2135eb443ba2c00f119c5ae596c55307df86f"
    );
    assert_eq!(
        sha256(&descriptor_path),
        "8ba247f92ef0fa11656a7490b94607da9dc0b6df88d3c3b887f024ca43dff12c"
    );
    let proto = fs::read_to_string(proto_path).unwrap();
    for exact in [
        "IngestAssetCopyRequest ingest_asset_copy = 1;",
        "GetLibraryStatusRequest get_library_status = 2;",
        "VerifyLibraryRequest verify_library = 3;",
        "ListIntegrityIssuesRequest list_integrity_issues = 4;",
        "InspectAssetRequest inspect_asset = 5;",
        "ListAssetsRequest list_assets = 6;",
        "MaterializeAssetRequest materialize_asset = 7;",
        "reserved 8 to 15;",
        "GetLibraryStatusResult get_library_status = 2;",
        "VerifyLibraryResult verify_library = 3;",
        "ListIntegrityIssuesResult list_integrity_issues = 4;",
        "InspectAssetResult inspect_asset = 5;",
        "ListAssetsResult list_assets = 6;",
        "MaterializeAssetResult materialize_asset = 7;",
        "ErrorEnvelope error = 15;",
        "reserved 8 to 14;",
    ] {
        assert!(proto.contains(exact), "protocol is missing {exact}");
    }
    let provenance = fs::read_to_string(root.join("proto/core/v1/handshake.provenance")).unwrap();
    for exact in [
        "format=mengxia-proto-provenance-v1",
        "proto_sha256=78f52b6c854ed04c9d35fb2533a2135eb443ba2c00f119c5ae596c55307df86f",
        "descriptor_sha256=8ba247f92ef0fa11656a7490b94607da9dc0b6df88d3c3b887f024ca43dff12c",
        "protoc_version=35.1",
        "protoc_artifact_sha256=193289af0470c6a1aada357d4fba0bbf8d78bfaac8b5e42ca30af2ef75583de2",
        "prost_build_version=0.14.4",
    ] {
        assert!(
            provenance.lines().any(|line| line == exact),
            "missing {exact}"
        );
    }
}

#[test]
fn task_008_protocol_boundary_has_no_authority_bearing_fields() {
    let proto = fs::read_to_string(root().join("proto/core/v1/handshake.proto")).unwrap();
    let materialize = proto
        .split("message MaterializeAssetRequest {")
        .nth(1)
        .and_then(|tail| tail.split("}\n").next())
        .expect("materialize request definition");
    for forbidden in [
        "cas_root =",
        "backend_id =",
        "locator =",
        "overwrite =",
        "recursive =",
    ] {
        assert!(
            !materialize.lines().any(|line| {
                let line = line.trim_start();
                !line.starts_with("reserved") && line.contains(forbidden)
            }),
            "authority-bearing field escaped reservation: {forbidden}"
        );
    }
    assert!(materialize.contains("reserved \"actor\", \"principal\", \"project_id\""));
    assert!(materialize.contains("\"cas_root\", \"overwrite\", \"recursive\";"));
}
