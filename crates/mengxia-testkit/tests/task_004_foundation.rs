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
    assert!(
        !store_source.contains("wal_reset_probe"),
        "test-only WAL-reset schema must not enter production modules"
    );
    let wal_reset_fixture =
        fs::read_to_string(root.join("crates/mengxia-store-sqlite/tests/wal_reset.rs"))
            .expect("standalone WAL-reset integration fixture");
    assert!(wal_reset_fixture.contains("CREATE TABLE wal_reset_probe"));
    assert!(wal_reset_fixture.contains("const SEEDS: u32 = 16"));
    assert!(wal_reset_fixture.contains("const CYCLES: u32 = 256"));
    assert!(wal_reset_fixture.contains("Duration::from_secs(30)"));
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
        "de848d18d6b31b3b0394499d5d67249ec5ecc4b77982dc09c12e10d6ddade719",
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
                !source.contains(".sqlite_child("),
                "fixed child token was minted outside its sole consumer: {}",
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
    assert!(!stock_production.contains("SQLITE_OPEN_CREATE"));
    assert!(!stock_production.contains("SQLITE_OPEN_URI"));
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

    let bootstrap =
        fs::read_to_string(store_source.join("bootstrap.rs")).expect("staging bootstrap module");
    let bootstrap_production = bootstrap
        .split("#[cfg(test)]")
        .next()
        .expect("bootstrap production prefix");
    assert!(bootstrap_production.contains("SqliteChild::BootstrapStaging"));
    assert!(bootstrap_production.contains("SqliteChild::Canonical"));
    assert!(!bootstrap_production.contains("SQLITE_OPEN_CREATE"));
    assert!(!bootstrap_production.contains("SQLITE_OPEN_URI"));

    let store_lib = fs::read_to_string(store_source.join("lib.rs")).expect("store crate root");
    assert!(store_lib.contains("mod lifecycle;"));
    assert!(!store_lib.contains("pub mod lifecycle"));
    assert!(!store_lib.contains("pub use lifecycle"));
    let lifecycle =
        fs::read_to_string(store_source.join("lifecycle.rs")).expect("store lifecycle module");
    assert!(lifecycle.contains("verify_on_writer"));
    assert!(lifecycle.contains("verify_on_reader"));
    for forbidden in [
        "pub fn submit",
        "pub(crate) fn submit",
        "pub trait WriterJob",
        "pub(crate) trait WriterJob",
        "pub trait ReadJob",
        "pub(crate) trait ReadJob",
        "Box<dyn Fn",
    ] {
        assert!(
            !lifecycle.contains(forbidden),
            "lifecycle must not expose a generic/raw command seam: {forbidden}"
        );
    }

    let workflow =
        fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("formal CI workflow");
    assert!(workflow.contains("runs-on: macos-26"));
    assert!(workflow.contains("MENGXIA_ACL_BUILD_CLASS: attested"));
    assert!(workflow.contains("scripts/verify-task-004.sh"));
    let task_001_gates =
        fs::read_to_string(root.join("scripts/verify-task-001.sh")).expect("TASK-001 gates");
    assert!(task_001_gates.contains(concat!(
        "/usr/bin/env -u MENGXIA_ACL_BUILD_CLASS \\\n",
        "        cargo clippy --workspace --all-targets --all-features --locked -- -D warnings"
    )));
    let preflight = workflow
        .find("scripts/verify-macos-acl-toolchain.sh")
        .expect("pre-Cargo platform preflight");
    let first_cargo = workflow.find("cargo ").expect("Cargo command");
    assert!(
        preflight < first_cargo,
        "platform preflight must precede Cargo"
    );

    let task_004_gates =
        fs::read_to_string(root.join("scripts/verify-task-004.sh")).expect("TASK-004 gates");
    for test_id in [
        "TEST-SQLITE-004",
        "TEST-CONFIG-004",
        "TEST-BOOTSTRAP-004",
        "TEST-PATH-004",
        "TEST-MIGRATION-004",
        "TEST-LOCK-004",
        "TEST-QUEUE-004",
        "TEST-ERROR-004",
        "TEST-RECOVERY-004",
        "TEST-WAL-004",
        "TEST-CORRUPTION-004",
        "TEST-ARCH-004",
        "TEST-SUPPLY-004",
        "TEST-DOC-004",
    ] {
        assert!(
            task_004_gates.contains(test_id),
            "TASK-004 gate script is missing {test_id}"
        );
    }
    for retained_gate in [
        "cargo fmt --all -- --check",
        "cargo check --workspace --all-targets --all-features --locked --offline",
        "cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings",
        "cargo test --workspace --all-targets --all-features --locked --offline",
        "git diff --check",
    ] {
        assert!(
            task_004_gates.contains(retained_gate),
            "TASK-004 gate script is missing retained gate {retained_gate}"
        );
    }

    let preflight_script = fs::read_to_string(root.join("scripts/verify-macos-acl-toolchain.sh"))
        .expect("platform preflight script");
    for required in [
        "/Applications/Xcode_26.6.app/Contents/Developer",
        "Build version 17F113",
        "require_exact_directory /Applications 0 80 775",
        "/bin/echo \"clang_sha256=$clang_sha256\"",
        "/bin/echo \"libtool_sha256=$libtool_sha256\"",
        "/bin/echo \"sys_acl_h_sha256=$acl_header_sha256\"",
        "7def90dd8829726686213a747fc5bff1583df933dae5edc55d755479e0bfe00a",
        "9511f84f0abe1e108e10979900d4fea8567534aef78f0984f7050c49f6c29ff7",
    ] {
        assert!(
            preflight_script.contains(required),
            "preflight is missing {required}"
        );
    }
    let observed_digest_log = preflight_script
        .find("/bin/echo \"clang_sha256=$clang_sha256\"")
        .expect("observed clang digest log");
    let digest_rejection = preflight_script
        .find("[ \"$clang_sha256\" = \\")
        .expect("fail-closed clang digest comparison");
    assert!(
        observed_digest_log < digest_rejection,
        "observed tool digests must be recorded before fail-closed comparison"
    );
}

#[test]
fn macos_acl_build_script_rejects_every_ambient_override_before_tool_discovery() {
    let root = workspace_root();
    let build_script = newest_platform_build_script(&root);
    let always_rejected = [
        "CC",
        "CFLAGS",
        "CPPFLAGS",
        "CPATH",
        "C_INCLUDE_PATH",
        "CPLUS_INCLUDE_PATH",
        "OBJC_INCLUDE_PATH",
        "SDKROOT",
        "DEVELOPER_DIR",
        "TOOLCHAINS",
        "MACOSX_DEPLOYMENT_TARGET",
        "ARCHFLAGS",
        "LD",
        "LDFLAGS",
        "LIBRARY_PATH",
        "AR",
        "ARFLAGS",
        "RANLIB",
        "RANLIBFLAGS",
        "NM",
        "STRIP",
        "ZERO_AR_DATE",
        "MENGXIA_ACL_TESTING",
        "CRATE_CC_CANARY",
        "CC_CANARY",
        "CFLAGS_CANARY",
        "CPPFLAGS_CANARY",
        "MENGXIA_ACL_CANARY",
        "AARCH64_APPLE_DARWIN_CC",
        "AARCH64_APPLE_DARWIN_CFLAGS",
        "AARCH64_APPLE_DARWIN_CPPFLAGS",
        "AARCH64_APPLE_DARWIN_AR",
        "AARCH64_APPLE_DARWIN_ARFLAGS",
        "aarch64_apple_darwin_CC",
        "aarch64_apple_darwin_CFLAGS",
        "aarch64_apple_darwin_CPPFLAGS",
        "aarch64_apple_darwin_AR",
        "aarch64_apple_darwin_ARFLAGS",
        "aarch64-apple-darwin_CC",
        "aarch64-apple-darwin_CFLAGS",
        "aarch64-apple-darwin_CPPFLAGS",
        "aarch64-apple-darwin_AR",
        "aarch64-apple-darwin_ARFLAGS",
        "HOST_CC",
        "HOST_CFLAGS",
        "HOST_CPPFLAGS",
        "HOST_AR",
        "HOST_ARFLAGS",
        "TARGET_CC",
        "TARGET_CFLAGS",
        "TARGET_CPPFLAGS",
        "TARGET_AR",
        "TARGET_ARFLAGS",
    ];
    for key in always_rejected {
        assert_build_script_rejects_override(&build_script, None, key);
    }

    for key in [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS",
        "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER",
        "BINDGEN_EXTRA_CLANG_ARGS",
        "CLANG_PATH",
        "COMPILER_PATH",
    ] {
        assert_build_script_rejects_override(&build_script, Some("attested"), key);
    }

    let output = Command::new(&build_script)
        .env_clear()
        .env("MENGXIA_ACL_BUILD_CLASS", "developer")
        .output()
        .expect("execute build script with invalid class");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("MENGXIA_ACL_BUILD_CLASS is invalid"),
        "invalid build class must fail before tool discovery"
    );
}

#[test]
fn fixed_sqlite_child_token_cannot_be_forged_and_path_copy_fails_repository_lint() {
    let root = workspace_root();
    let fixtures = root.join("crates/mengxia-testkit/tests/fixtures/platform");

    let forge = cargo_check_fixture(
        &root,
        &fixtures.join("token-forge/Cargo.toml"),
        "task-004-token-forge",
    );
    assert!(!forge.status.success(), "private child token was forgeable");
    let forge_stderr = String::from_utf8_lossy(&forge.stderr);
    assert!(
        forge_stderr.contains("field `path` of struct `FixedSqliteChildPath` is private"),
        "token-forge fixture failed for an unexpected reason: {forge_stderr}"
    );

    let copy = cargo_check_fixture(
        &root,
        &fixtures.join("path-copy/Cargo.toml"),
        "task-004-path-copy",
    );
    assert!(
        copy.status.success(),
        "the contract does not claim Path extraction is a type-system error: {}",
        String::from_utf8_lossy(&copy.stderr)
    );
    let copy_source = fs::read_to_string(fixtures.join("path-copy/src/main.rs"))
        .expect("path-copy fixture source");
    assert_eq!(
        lint_fixed_child_path_consumer(&copy_source),
        Err("fixed SQLite child path may not be copied or persisted")
    );
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

fn newest_platform_build_script(root: &Path) -> std::path::PathBuf {
    let build_root = root.join("target/debug/build");
    fs::read_dir(build_root)
        .expect("read Cargo build-script directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("mengxia-platform-fs-"))
            {
                return None;
            }
            let executable = path.join("build-script-build");
            let modified = executable.metadata().ok()?.modified().ok()?;
            Some((modified, executable))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
        .expect("compiled mengxia-platform-fs build script")
}

fn assert_build_script_rejects_override(build_script: &Path, class: Option<&str>, key: &str) {
    let mut command = Command::new(build_script);
    command.env_clear().env(key, "task-004-negative-canary");
    if let Some(class) = class {
        command.env("MENGXIA_ACL_BUILD_CLASS", class);
    }
    let output = command
        .output()
        .expect("execute build-script negative fixture");
    assert!(!output.status.success(), "override {key} was accepted");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("ambient override {key} is forbidden")),
        "override {key} did not fail at the environment gate: {stderr}"
    );
}

fn cargo_check_fixture(root: &Path, manifest: &Path, target_name: &str) -> std::process::Output {
    Command::new(env!("CARGO"))
        .args(["check", "--manifest-path"])
        .arg(manifest)
        .args(["--locked", "--offline"])
        .env("CARGO_TARGET_DIR", root.join("target").join(target_name))
        .output()
        .expect("TASK-004 compile fixture must start")
}

fn lint_fixed_child_path_consumer(source: &str) -> Result<(), &'static str> {
    if [
        ".to_path_buf()",
        "PathBuf::from",
        ".display()",
        "format!(",
        "println!(",
        "eprintln!(",
    ]
    .iter()
    .any(|forbidden| source.contains(forbidden))
    {
        Err("fixed SQLite child path may not be copied or persisted")
    } else {
        Ok(())
    }
}
