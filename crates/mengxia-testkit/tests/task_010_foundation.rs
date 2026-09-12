use std::fs;
use std::path::{Path, PathBuf};

use mengxia_plugin_package::{PluginPackageError, RuntimeDependencyRole, inspect_manifest};
use mengxia_plugin_security::{
    IncomparableReason, PermissionChangeDisposition, PermissionChangeNamespace,
    PermissionDiffClassification, diff_permissions,
};
use sha2::{Digest, Sha256};

const GOLDEN_FILE: &[u8] = include_bytes!("fixtures/task_010/manifest-v1.golden.json");
const GOLDEN_DIGEST: &str = "7611be011dfc367d469eb730f67e5e26d4dbff3b909fbb905249cec2e0442a39";
const SCHEMA_DIGEST: &str = "2b67092d2a4b9a8d417db35a41249f2f40b5bcc4e505c92aeaab18281d19cd2b";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_owned()
}

fn golden() -> &'static [u8] {
    GOLDEN_FILE
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn manifest(
    publisher: &str,
    plugin_id: &str,
    version: &str,
    capabilities: &[String],
    permission: bool,
    dependencies: &[(String, &'static str, u64, char)],
) -> Vec<u8> {
    let capabilities = capabilities
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(",");
    let permission = if permission {
        "{\"kind\":\"broker.asset.read@1\",\"scope\":\"run-inputs\"}"
    } else {
        ""
    };
    let dependencies = dependencies
        .iter()
        .map(|(id, role, length, digest)| {
            format!(
                "{{\"byte_length\":{length},\"dependency_id\":\"{id}\",\"role\":\"{role}\",\"sha256\":\"{}\",\"target\":\"aarch64-apple-darwin\"}}",
                digest.to_string().repeat(64)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"$schema\":\"https://schemas.mengxia.local/plugin/manifest-v1.schema.json\",\"capabilities\":[{capabilities}],\"manifest_version\":1,\"plugin_id\":\"{plugin_id}\",\"publisher\":\"{publisher}\",\"requested_permissions\":[{permission}],\"runtime_dependencies\":[{dependencies}],\"version\":\"{version}\"}}"
    )
    .into_bytes()
}

fn one_dependency() -> Vec<(String, &'static str, u64, char)> {
    vec![("entrypoint".to_owned(), "PLUGIN_ENTRYPOINT", 1, '1')]
}

#[test]
fn manifest_schema_and_golden_bytes_are_exact_and_offline() {
    let schema = fs::read(root().join("schemas/plugin/manifest-v1.schema.json")).unwrap();
    assert_eq!(digest_hex(&schema), SCHEMA_DIGEST);
    let schema_text = std::str::from_utf8(&schema).unwrap();
    assert!(schema_text.contains("https://json-schema.org/draft/2020-12/schema"));
    assert!(schema_text.contains("\"additionalProperties\":false"));
    assert!(!schema_text.contains("http://"));
    assert_eq!(digest_hex(golden()), GOLDEN_DIGEST);

    let package = inspect_manifest(golden()).expect("canonical fixture");
    assert_eq!(package.canonical_bytes(), golden());
    assert_eq!(package.digest().to_string(), GOLDEN_DIGEST);
    assert_eq!(
        package.runtime_dependencies()[0].role(),
        RuntimeDependencyRole::PluginEntrypoint
    );
}

#[test]
fn package_digest_covers_every_byte_and_tamper_fails_closed() {
    let package = inspect_manifest(golden()).expect("canonical fixture");
    assert_eq!(
        package.digest().to_bytes(),
        <[u8; 32]>::from(Sha256::digest(golden()))
    );
    let mut tampered = golden().to_vec();
    tampered[0] = b'[';
    assert!(inspect_manifest(&tampered).is_err());
    assert_eq!(package.canonical_bytes(), golden());
}

#[test]
fn dependency_declarations_are_bounded_and_contain_no_path_authority() {
    let package = inspect_manifest(golden()).expect("canonical fixture");
    let dependency = &package.runtime_dependencies()[0];
    assert_eq!(dependency.dependency_id(), "entrypoint");
    assert_eq!(dependency.target(), "aarch64-apple-darwin");
    assert_eq!(dependency.byte_length(), 1234);

    for length in [1_073_741_823, 1_073_741_824] {
        assert!(
            inspect_manifest(&manifest(
                "publisher",
                "plugin.id",
                "1.0.0",
                &[],
                false,
                &[("entrypoint".to_owned(), "PLUGIN_ENTRYPOINT", length, '1')]
            ))
            .is_ok()
        );
    }

    for forbidden in ["path", "uri", "command", "argument", "environment", "shell"] {
        assert!(
            !std::str::from_utf8(golden())
                .unwrap()
                .contains(&format!("\"{forbidden}\""))
        );
    }
    let no_entrypoint = manifest(
        "publisher",
        "plugin.id",
        "1.0.0",
        &[],
        false,
        &[("tool".to_owned(), "TOOL", 1, '1')],
    );
    assert_eq!(
        inspect_manifest(&no_entrypoint),
        Err(PluginPackageError::ManifestInvalid)
    );
    for invalid in [
        std::str::from_utf8(golden())
            .unwrap()
            .replace("\"byte_length\":1234", "\"byte_length\":0"),
        std::str::from_utf8(golden())
            .unwrap()
            .replace("\"byte_length\":1234", "\"byte_length\":1073741825"),
        std::str::from_utf8(golden())
            .unwrap()
            .replace(&"1".repeat(64), &"0".repeat(64)),
        std::str::from_utf8(golden()).unwrap().replace(
            "\"target\":\"aarch64-apple-darwin\"",
            "\"path\":\"/tmp/plugin\",\"target\":\"aarch64-apple-darwin\"",
        ),
    ] {
        assert_eq!(
            inspect_manifest(invalid.as_bytes()),
            Err(PluginPackageError::ManifestInvalid)
        );
    }
    let aggregate_cap_minus_one = vec![
        (
            "dependency0".to_owned(),
            "PLUGIN_ENTRYPOINT",
            1_073_741_824,
            '1',
        ),
        ("dependency1".to_owned(), "TOOL", 1_073_741_824, '2'),
        ("dependency2".to_owned(), "TOOL", 1_073_741_824, '3'),
        ("dependency3".to_owned(), "TOOL", 1_073_741_823, '4'),
    ];
    let mut aggregate_cap = aggregate_cap_minus_one.clone();
    aggregate_cap[3].2 = 1_073_741_824;
    for dependencies in [&aggregate_cap_minus_one, &aggregate_cap] {
        assert!(
            inspect_manifest(&manifest(
                "publisher",
                "plugin.id",
                "1.0.0",
                &[],
                false,
                dependencies
            ))
            .is_ok()
        );
    }
    let mut aggregate_cap_plus_one = aggregate_cap;
    aggregate_cap_plus_one.push(("dependency4".to_owned(), "TOOL", 1, '5'));
    assert_eq!(
        inspect_manifest(&manifest(
            "publisher",
            "plugin.id",
            "1.0.0",
            &[],
            false,
            &aggregate_cap_plus_one
        )),
        Err(PluginPackageError::ResourceLimitExceeded)
    );
}

#[test]
fn semantic_diff_is_total_sorted_and_reaches_the_exact_193_bound() {
    let old_caps = (0..64)
        .map(|index| format!("old.cap{index:02}@1"))
        .collect::<Vec<_>>();
    let new_caps = (0..64)
        .map(|index| format!("new.cap{index:02}@1"))
        .collect::<Vec<_>>();
    let old_dependencies = (0..32)
        .map(|index| {
            (
                format!("old{index:02}"),
                if index == 0 {
                    "PLUGIN_ENTRYPOINT"
                } else {
                    "TOOL"
                },
                1,
                '1',
            )
        })
        .collect::<Vec<_>>();
    let new_dependencies = (0..32)
        .map(|index| {
            (
                format!("new{index:02}"),
                if index == 0 {
                    "PLUGIN_ENTRYPOINT"
                } else {
                    "TOOL"
                },
                1,
                '2',
            )
        })
        .collect::<Vec<_>>();
    let old = inspect_manifest(&manifest(
        "publisher",
        "plugin.id",
        "1.0.0",
        &old_caps,
        true,
        &old_dependencies,
    ))
    .unwrap();
    let candidate_192 = inspect_manifest(&manifest(
        "publisher",
        "plugin.id",
        "2.0.0",
        &new_caps,
        true,
        &new_dependencies,
    ))
    .unwrap();
    assert_eq!(diff_permissions(&old, &candidate_192).changes().len(), 192);
    let candidate = inspect_manifest(&manifest(
        "publisher",
        "plugin.id",
        "2.0.0",
        &new_caps,
        false,
        &new_dependencies,
    ))
    .unwrap();
    let diff = diff_permissions(&old, &candidate);
    assert_eq!(
        diff.classification(),
        PermissionDiffClassification::Expansion
    );
    assert_eq!(diff.reason(), IncomparableReason::None);
    assert_eq!(diff.changes().len(), 193);
    assert_eq!(
        diff.changes()[0].namespace(),
        PermissionChangeNamespace::Permission
    );
    assert_eq!(
        diff.changes()[0].disposition(),
        PermissionChangeDisposition::Removed
    );
    assert!(diff.changes().windows(2).all(|pair| pair[0] <= pair[1]));
}

#[test]
fn publisher_text_never_establishes_identity_or_grant() {
    let old = inspect_manifest(golden()).unwrap();
    let version_changed = inspect_manifest(
        &std::str::from_utf8(golden())
            .unwrap()
            .replace("1.0.0", "1.0.1")
            .into_bytes(),
    )
    .unwrap();
    assert_ne!(old.digest(), version_changed.digest());
    let diff = diff_permissions(&old, &version_changed);
    assert_eq!(
        diff.classification(),
        PermissionDiffClassification::Unchanged
    );
    assert!(diff.changes().is_empty());

    let other_identity = inspect_manifest(
        &std::str::from_utf8(golden())
            .unwrap()
            .replace("example.transcoder", "example.impostor")
            .into_bytes(),
    )
    .unwrap();
    assert_eq!(
        diff_permissions(&old, &other_identity).classification(),
        PermissionDiffClassification::IncomparableDeny
    );
}

#[test]
fn parser_and_collection_boundaries_have_exact_error_classes() {
    for token in ["+1", "01", ".1", "NaN", "Infinity", "1e+"] {
        assert_eq!(
            inspect_manifest(token.as_bytes()),
            Err(PluginPackageError::MalformedJson)
        );
    }
    for token in ["-1", "-0", "1.0", "1e0", "1e999", "18446744073709551616"] {
        assert_eq!(
            inspect_manifest(token.as_bytes()),
            Err(PluginPackageError::ManifestInvalid)
        );
    }
    for length in [65_535, 65_536] {
        assert_ne!(
            inspect_manifest(&vec![b' '; length]),
            Err(PluginPackageError::InputTooLarge)
        );
    }
    assert_eq!(
        inspect_manifest(&vec![b' '; 65_537]),
        Err(PluginPackageError::InputTooLarge)
    );
    let depth_17 = format!("{}0{}", "[".repeat(17), "]".repeat(17));
    assert_eq!(
        inspect_manifest(depth_17.as_bytes()),
        Err(PluginPackageError::ResourceLimitExceeded)
    );
    let string_4097 = format!("\"{}\"", "a".repeat(4097));
    assert_eq!(
        inspect_manifest(string_4097.as_bytes()),
        Err(PluginPackageError::ResourceLimitExceeded)
    );
    assert_eq!(
        inspect_manifest(br#"{"x":1,"x":2}"#),
        Err(PluginPackageError::DuplicateKey)
    );

    for count in [63, 64] {
        let capabilities = (0..count)
            .map(|index| format!("cap.item{index:02}@1"))
            .collect::<Vec<_>>();
        assert!(
            inspect_manifest(&manifest(
                "publisher",
                "plugin.id",
                "1.0.0",
                &capabilities,
                false,
                &one_dependency()
            ))
            .is_ok()
        );
    }
    let caps_65 = (0..65)
        .map(|index| format!("cap.item{index:02}@1"))
        .collect::<Vec<_>>();
    assert_eq!(
        inspect_manifest(&manifest(
            "publisher",
            "plugin.id",
            "1.0.0",
            &caps_65,
            false,
            &one_dependency()
        )),
        Err(PluginPackageError::ResourceLimitExceeded)
    );

    let dependencies_32 = (0..32)
        .map(|index| {
            (
                format!("dependency{index:02}"),
                if index == 0 {
                    "PLUGIN_ENTRYPOINT"
                } else {
                    "TOOL"
                },
                1,
                '1',
            )
        })
        .collect::<Vec<_>>();
    for dependencies in [&dependencies_32[..31], &dependencies_32[..32]] {
        assert!(
            inspect_manifest(&manifest(
                "publisher",
                "plugin.id",
                "1.0.0",
                &[],
                false,
                dependencies
            ))
            .is_ok()
        );
    }
    let mut dependencies_33 = dependencies_32;
    dependencies_33.push(("dependency32".to_owned(), "TOOL", 1, '2'));
    assert_eq!(
        inspect_manifest(&manifest(
            "publisher",
            "plugin.id",
            "1.0.0",
            &[],
            false,
            &dependencies_33
        )),
        Err(PluginPackageError::ResourceLimitExceeded)
    );

    let permission = "{\"kind\":\"broker.asset.read@1\",\"scope\":\"run-inputs\"}";
    let duplicate_permissions = std::str::from_utf8(golden())
        .unwrap()
        .replace(permission, &format!("{permission},{permission}"));
    assert_eq!(
        inspect_manifest(duplicate_permissions.as_bytes()),
        Err(PluginPackageError::ResourceLimitExceeded)
    );
}

#[test]
fn unknown_noncanonical_and_invalid_security_fields_never_disappear() {
    let duplicate = std::str::from_utf8(golden()).unwrap().replacen(
        "\"plugin_id\":",
        "\"plugin_id\":\"duplicate\",\"plugin_id\":",
        1,
    );
    assert_eq!(
        inspect_manifest(duplicate.as_bytes()),
        Err(PluginPackageError::DuplicateKey)
    );
    let unknown = std::str::from_utf8(golden())
        .unwrap()
        .replacen("}", ",\"zzz\":0}", 1);
    assert_eq!(
        inspect_manifest(unknown.as_bytes()),
        Err(PluginPackageError::ManifestInvalid)
    );
    let invalid_permission = std::str::from_utf8(golden())
        .unwrap()
        .replace("broker.asset.read@1", "broker.asset.write@1");
    assert_eq!(
        inspect_manifest(invalid_permission.as_bytes()),
        Err(PluginPackageError::ManifestInvalid)
    );
    let mut padded = b" ".to_vec();
    padded.extend_from_slice(golden());
    assert_eq!(
        inspect_manifest(&padded),
        Err(PluginPackageError::NoncanonicalBytes)
    );
}

#[test]
fn dependency_and_supply_graph_is_exact_and_forbidden_runtime_edges_are_absent() {
    let package_manifest =
        fs::read_to_string(root().join("crates/mengxia-plugin-package/Cargo.toml")).unwrap();
    let security_manifest =
        fs::read_to_string(root().join("crates/mengxia-plugin-security/Cargo.toml")).unwrap();
    assert!(package_manifest.contains("jsonschema.workspace = true"));
    assert!(package_manifest.contains("semver.workspace = true"));
    assert!(security_manifest.contains("mengxia-plugin-package"));
    for forbidden in [
        "tokio",
        "rusqlite",
        "mengxia-app",
        "mengxia-ports",
        "mengxia-platform-fs",
    ] {
        assert!(!package_manifest.contains(forbidden));
        assert!(!security_manifest.contains(forbidden));
    }
    let lock = fs::read(root().join("Cargo.lock")).unwrap();
    assert_eq!(
        digest_hex(&lock),
        "302df8141acee77aa58ecb796a53ecbb4faf9f6cd55dc384667bb08e725c0b2e"
    );
    let root_manifest = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    assert!(
        root_manifest.contains("jsonschema = { version = \"=0.56.0\", default-features = false }")
    );
    assert!(!root_manifest.contains("resolve-http"));
    assert!(!root_manifest.contains("resolve-file"));
    let implementation =
        fs::read_to_string(root().join("crates/mengxia-plugin-package/src/manifest.rs")).unwrap();
    assert!(implementation.contains(".with_draft(Draft::Draft202012)"));
    assert!(implementation.contains(".offline()"));
    assert!(implementation.contains(".should_validate_formats(false)"));
    for forbidden in [
        "std::fs",
        "std::net",
        "std::process",
        "tokio::",
        "rusqlite::",
    ] {
        assert!(!implementation.contains(forbidden));
    }
}

#[test]
fn task_010_gate_driver_owns_all_nine_stable_test_ids() {
    let script = fs::read_to_string(root().join("scripts/verify-task-010.sh")).unwrap();
    for (test_id, command) in [
        ("TEST-MANIFEST-010", "manifest_check"),
        (
            "TEST-PACKAGE-010",
            "cargo test --locked --offline -p mengxia-testkit --test task_010_foundation package_digest",
        ),
        (
            "TEST-DEPENDENCY-010",
            "cargo test --locked --offline -p mengxia-testkit --test task_010_foundation dependency_declarations",
        ),
        ("TEST-DIFF-010", "diff_check"),
        (
            "TEST-PUBLISHER-010",
            "cargo test --locked --offline -p mengxia-testkit --test task_010_foundation publisher_text",
        ),
        (
            "TEST-BOUNDS-010",
            "cargo test --locked --offline -p mengxia-testkit --test task_010_foundation parser_and_collection",
        ),
        (
            "TEST-ARCH-010",
            "cargo test --locked --offline -p mengxia-testkit --test architecture",
        ),
        ("TEST-SUPPLY-010", "supply_check"),
        (
            "TEST-DOC-010",
            "cargo test --locked --offline -p mengxia-testkit --test document_traceability",
        ),
    ] {
        assert_eq!(script.matches(test_id).count(), 1, "mapping for {test_id}");
        assert_eq!(
            script.matches(&format!("run {test_id} {command}")).count(),
            1,
            "executable mapping for {test_id}"
        );
    }
    for directly_owned_test in [
        "manifest_schema_and_golden_bytes_are_exact_and_offline",
        "unknown_noncanonical_and_invalid_security_fields_never_disappear",
        "semantic_diff_is_total_sorted_and_reaches_the_exact_193_bound",
    ] {
        assert_eq!(
            script.matches(directly_owned_test).count(),
            1,
            "stable-ID mapping must directly execute {directly_owned_test}"
        );
    }
    let component_branch = script.find("if [ \"$component\" -eq 0 ]").unwrap();
    let last_mapping = script.rfind("run TEST-DOC-010").unwrap();
    assert!(last_mapping < component_branch);
    assert!(script.contains("component=0"));
    assert!(script.contains("[ \"$component\" -eq 0 ]"));
    assert!(!script.contains("TASK-011"));
}
