mod support;

use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use support::{cargo_metadata, parse_packages, workspace_root};

#[test]
fn task_002_dependency_surface_is_exact_and_layer_safe() {
    let root = workspace_root();
    let output = cargo_metadata(&root.join("Cargo.toml"), true);
    assert!(
        output.status.success(),
        "locked metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packages = parse_packages(&String::from_utf8(output.stdout).expect("metadata is UTF-8"))
        .expect("metadata package JSON is valid");

    let types = packages
        .iter()
        .find(|package| package.name == "mengxia-types")
        .expect("mengxia-types exists");
    let actual: BTreeSet<_> = types.dependencies.iter().map(String::as_str).collect();
    let expected = BTreeSet::from(["getrandom", "proptest", "time", "uuid"]);
    assert_eq!(actual, expected, "TASK-002 dependency surface drifted");

    let domain = packages
        .iter()
        .find(|package| package.name == "mengxia-domain")
        .expect("mengxia-domain exists");
    let actual: BTreeSet<_> = domain.dependencies.iter().map(String::as_str).collect();
    let expected = BTreeSet::from(["mengxia-events", "mengxia-types"]);
    assert_eq!(actual, expected, "domain dependency surface drifted");

    let manifest = fs::read_to_string(root.join("crates/mengxia-types/Cargo.toml"))
        .expect("types manifest is readable");
    for declaration in [
        "getrandom.workspace = true",
        "time.workspace = true",
        "uuid.workspace = true",
        "[dev-dependencies]",
        "proptest.workspace = true",
    ] {
        assert!(
            manifest.contains(declaration),
            "missing exact dependency declaration: {declaration}"
        );
    }

    let production_tree = Command::new(env!("CARGO"))
        .args([
            "tree",
            "-p",
            "mengxia-types",
            "--edges",
            "normal,build",
            "--locked",
        ])
        .current_dir(&root)
        .output()
        .expect("cargo tree must start");
    assert!(
        production_tree.status.success(),
        "production tree failed: {}",
        String::from_utf8_lossy(&production_tree.stderr)
    );
    let production_tree =
        String::from_utf8(production_tree.stdout).expect("cargo tree output is UTF-8");
    for required in ["getrandom v0.4.3", "time v0.3.55", "uuid v1.24.1"] {
        assert!(
            production_tree.contains(required),
            "production tree is missing {required}"
        );
    }
    for forbidden in [
        "getrandom v0.3.4",
        "proptest",
        "serde",
        "prost",
        "rusqlite",
        "tokio",
        "reqwest",
    ] {
        assert!(
            !production_tree.contains(forbidden),
            "production tree contains forbidden dependency {forbidden}"
        );
    }
}

#[test]
fn marker_mismatch_fixture_fails_to_compile() {
    let root = workspace_root();
    let manifest =
        root.join("crates/mengxia-testkit/tests/fixtures/types/marker-mismatch/Cargo.toml");
    let output = Command::new(env!("CARGO"))
        .args(["check", "--manifest-path"])
        .arg(&manifest)
        .args(["--locked", "--offline"])
        .env(
            "CARGO_TARGET_DIR",
            root.join("target/task-002-marker-mismatch"),
        )
        .output()
        .expect("compile-fail fixture must start");
    assert!(
        !output.status.success(),
        "Id<Project> unexpectedly compiled where Id<Asset> was required"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("mismatched types")
            && stderr.contains("Id<Asset>")
            && stderr.contains("Id<Project>"),
        "fixture failed for an unexpected reason: {stderr}"
    );
}
