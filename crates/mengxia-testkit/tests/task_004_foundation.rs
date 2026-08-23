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

#[test]
fn macos_acl_path_authority_is_isolated_and_source_pinned() {
    let root = workspace_root();
    let platform = root.join("crates/mengxia-platform-fs");
    let store_source = root.join("crates/mengxia-store-sqlite/src");

    assert_sha256(
        &platform.join("include/mengxia_acl_shim.h"),
        "290a5d14690153492ae2e2be9bd6a9449212dbe6c44436950bd6e53bf6689ada",
    );
    assert_sha256(
        &platform.join("src/macos_acl_shim.c"),
        "f623dcee15ca47f6bea92a678532d582995eaae5a34f45f5c81e54f38af4a110",
    );
    assert_sha256(
        &platform.join("src/macos_acl_abi_probe.c"),
        "b86f7355ec61100c96b52d0cef34daea469dd711088985d208cc315d1d302982",
    );
    assert_sha256(
        &platform.join("tests/macos_acl_shim_test.c"),
        "2f2c458b62773d31e2cd22f11e43c0a8f1ee52296ab7f5689c05bf4bbb1fd026",
    );

    let platform_lib = fs::read_to_string(platform.join("src/lib.rs")).expect("platform lib");
    let platform_ffi =
        fs::read_to_string(platform.join("src/macos_ffi.rs")).expect("private FFI module");
    let platform_build = fs::read_to_string(platform.join("build.rs")).expect("build script");
    assert!(platform_lib.contains("#![deny(unsafe_code)]"));
    assert!(!platform_lib.contains("unsafe {"));
    assert!(!platform_lib.contains("extern \"C\""));
    assert!(
        !platform_lib.contains("pub fn library_root_fd"),
        "opaque path authority must not expose its retained root descriptor"
    );
    assert!(platform_ffi.contains("#![allow(unsafe_code)]"));
    assert_eq!(platform_ffi.matches("unsafe extern \"C\"").count(), 1);
    assert_eq!(platform_ffi.matches("unsafe {").count(), 2);
    assert!(!platform_build.contains("cc::"));
    assert!(!platform_build.contains("Command::new(\"sh\")"));
    assert!(!platform_build.contains("Command::new(\"/bin/sh\")"));

    let store_files: Vec<_> = fs::read_dir(&store_source)
        .expect("store source")
        .map(|entry| entry.expect("store entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect();
    for path in store_files {
        let source = fs::read_to_string(&path).expect("store Rust source");
        let name = path.file_name().and_then(|name| name.to_str());
        if name != Some("stock_sqlite_open.rs") {
            assert!(
                !source.contains("FixedSqliteChildPath"),
                "fixed child token escaped its sole consumer: {}",
                path.display()
            );
            assert!(
                !source.contains("Connection::open_with_flags"),
                "stock SQLite path open escaped its sole consumer: {}",
                path.display()
            );
        }
    }

    let stock = fs::read_to_string(store_source.join("stock_sqlite_open.rs"))
        .expect("stock SQLite consumer");
    let stock_production = stock
        .split("#[cfg(test)]")
        .next()
        .expect("production prefix");
    assert_eq!(
        stock_production
            .matches("Connection::open_with_flags")
            .count(),
        1
    );
    for forbidden in [
        "to_path_buf",
        "PathBuf::from",
        ".display()",
        "format!(",
        "println!(",
        "eprintln!(",
    ] {
        assert!(
            !stock_production.contains(forbidden),
            "fixed child consumer may not copy or log paths: {forbidden}"
        );
    }

    let workflow =
        fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("formal CI workflow");
    assert!(workflow.contains("runs-on: macos-26"));
    assert!(workflow.contains("MENGXIA_ACL_BUILD_CLASS: attested"));
    let preflight = workflow
        .find("scripts/verify-macos-acl-toolchain.sh")
        .expect("pre-Cargo platform preflight");
    let first_cargo = workflow.find("cargo ").expect("Cargo command");
    assert!(
        preflight < first_cargo,
        "platform preflight must precede Cargo"
    );

    let preflight_script = fs::read_to_string(root.join("scripts/verify-macos-acl-toolchain.sh"))
        .expect("platform preflight script");
    for required in [
        "/Applications/Xcode_26.6.app/Contents/Developer",
        "Build version 17F113",
        "require_exact_directory /Applications 0 80 775",
        "7def90dd8829726686213a747fc5bff1583df933dae5edc55d755479e0bfe00a",
        "9511f84f0abe1e108e10979900d4fea8567534aef78f0984f7050c49f6c29ff7",
    ] {
        assert!(
            preflight_script.contains(required),
            "preflight is missing {required}"
        );
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
