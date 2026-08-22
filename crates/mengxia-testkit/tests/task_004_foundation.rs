mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

use support::{cargo_metadata, parse_packages, workspace_root};

#[test]
fn task_004_dependency_and_unsafe_boundaries_are_exact() {
    let root = workspace_root();
    let output = cargo_metadata(&root.join("Cargo.toml"), true);
    assert!(
        output.status.success(),
        "locked metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packages = parse_packages(&String::from_utf8(output.stdout).expect("metadata is UTF-8"))
        .expect("metadata JSON is valid");

    let platform = packages
        .iter()
        .find(|package| package.name == "mengxia-platform-fs")
        .expect("platform filesystem package exists");
    assert_eq!(
        dependency_set(&platform.dependencies),
        BTreeSet::from(["rustix", "sha2"]),
        "platform FFI leaf may use only safe rustix plus build-time sha2"
    );

    let store = packages
        .iter()
        .find(|package| package.name == "mengxia-store-sqlite")
        .expect("SQLite store package exists");
    assert_eq!(
        dependency_set(&store.dependencies),
        BTreeSet::from([
            "mengxia-app",
            "mengxia-domain",
            "mengxia-events",
            "mengxia-platform-fs",
            "mengxia-ports",
            "mengxia-types",
            "rusqlite",
            "rustix",
            "sha2",
            "tokio",
        ])
    );

    let store_source = read_rust_tree(&root.join("crates/mengxia-store-sqlite/src"));
    assert!(store_source.contains("#![forbid(unsafe_code)]"));
    assert!(!store_source.contains("unsafe {"));
    assert!(!store_source.contains("libc::"));
    assert!(!store_source.contains("extern \"C\""));
}

#[test]
fn source_pinned_sqlite_patch_has_exact_bytes_and_no_fallback_tooling() {
    let root = workspace_root();
    let patch = root.join("third_party/libsqlite3-sys-0.38.2");

    assert_sha256(
        &patch.join("sqlite3/sqlite3.c"),
        "b1dd5d74ec7f29055a6684fa06fb3c2f6821c87dd38f9a458dfd2e8a1db28189",
    );
    assert_sha256(
        &patch.join("sqlite3/sqlite3.h"),
        "919e7f2e8ed1d8f56ac17b412b8971c76aa5d1a879752cc6058f75e7d5910e1d",
    );
    assert_sha256(
        &root.join("migrations/sqlite/0000_store_bootstrap.sql"),
        "35a69e30b627e994a172c9490f391552a8d60212c75ad2f478ea1005c0b94ce2",
    );

    let manifest = fs::read_to_string(patch.join("Cargo.toml")).expect("patch manifest");
    let build = fs::read_to_string(patch.join("build.rs")).expect("patch build script");
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("workspace lock");
    for forbidden in [
        "[build-dependencies.cc]",
        "[build-dependencies.bindgen]",
        "[build-dependencies.pkg-config]",
        "[build-dependencies.vcpkg]",
    ] {
        assert!(!manifest.contains(forbidden), "forbidden: {forbidden}");
    }
    for forbidden in ["cc::", "pkg_config", "vcpkg", "bindgen::"] {
        assert!(
            !build.contains(forbidden),
            "forbidden build path: {forbidden}"
        );
    }
    for forbidden_package in ["cc", "pkg-config", "vcpkg", "bindgen"] {
        assert!(
            !lock.contains(&format!("name = \"{forbidden_package}\"")),
            "forbidden locked package: {forbidden_package}"
        );
    }
    for required_define in [
        "-DSQLITE_THREADSAFE=1",
        "-DSQLITE_DQS=0",
        "-DSQLITE_DEFAULT_FOREIGN_KEYS=1",
        "-DSQLITE_DEFAULT_WAL_SYNCHRONOUS=2",
        "-DSQLITE_TRUSTED_SCHEMA=0",
        "-DSQLITE_OMIT_LOAD_EXTENSION",
    ] {
        assert!(build.contains(required_define));
    }
}

fn dependency_set(dependencies: &[String]) -> BTreeSet<&str> {
    dependencies.iter().map(String::as_str).collect()
}

fn read_rust_tree(directory: &Path) -> String {
    let mut paths: Vec<_> = fs::read_dir(directory)
        .expect("read Rust source directory")
        .map(|entry| entry.expect("source entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .map(|path| fs::read_to_string(path).expect("read Rust source"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_sha256(path: &Path, expected: &str) {
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("start shasum");
    assert!(
        output.status.success(),
        "shasum failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("shasum output is UTF-8");
    let actual = stdout.split_whitespace().next().expect("digest field");
    assert_eq!(actual, expected, "digest mismatch for {}", path.display());
}
