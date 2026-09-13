mod support;

use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = support::workspace_root().join("target");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut owned = None;
        for _ in 0..64 {
            let path = parent.join(format!(
                "maint003-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => {
                    owned = Some(Self(path));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("fixture create: {e}"),
            }
        }
        let fixture = owned.expect("bounded fixture allocation");
        for dir in ["scripts", "docs/provenance", "target"] {
            fs::create_dir_all(fixture.0.join(dir)).unwrap();
        }
        for path in [
            "scripts/toolchain-tools.sh",
            "scripts/toolchain-environment.sh",
            "scripts/toolchain-maintenance.sh",
            "scripts/dev-toolchain.sh",
            "docs/provenance/developer-tools-v1.toml",
            "docs/provenance/toolchain-security-events-v1.tsv",
            "rust-toolchain.toml",
        ] {
            fs::copy(support::workspace_root().join(path), fixture.0.join(path)).unwrap();
        }
        fixture
    }
    fn write(&self, path: &str, text: &str) {
        fs::write(self.0.join(path), text).unwrap();
    }
    fn shell(&self, script: &str) -> Output {
        Command::new("/bin/sh")
            .args(["-eu", "-c", script])
            .env("repository_root", &self.0)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
    fn tool(&self, script: &str) -> Output {
        self.shell(&format!(". scripts/toolchain-tools.sh\n{script}"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn rejected(output: Output, reason: &str) {
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(reason),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn environment_rejects_overrides_and_inspect_does_not_prepare() {
    let f = Fixture::new();
    for key in [
        "CC",
        "DEVELOPER_DIR",
        "RUSTC",
        "RUSTFLAGS",
        "CARGO_BUILD_TARGET",
        "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER",
    ] {
        let output = f.shell(&format!(
            "export {key}=untrusted\n. scripts/toolchain-environment.sh\ntoolchain_rust"
        ));
        rejected(output, "UNVERIFIABLE: unsupported override");
    }
    rejected(
        f.shell("/bin/sh scripts/dev-toolchain.sh prepare"),
        "requires explicit --network",
    );
    rejected(f.tool("tool_resolve"), "NEEDS_PREPARATION");
    assert!(!f.0.join("target/mengxia-tools").exists());
    let root = support::workspace_root();
    let env = fs::read_to_string(root.join("scripts/toolchain-environment.sh")).unwrap();
    for required in [
        "rustup which --toolchain",
        "tool_rust_sha",
        "tool_cargo_sha",
        "toolchain_observe",
        "MENGXIA_TOOLCHAIN_FINGERPRINT=$tool_fingerprint",
    ] {
        assert!(env.contains(required));
    }
}

#[test]
fn closed_tool_manifest_rejects_unknown_duplicate_missing_and_malformed_fields() {
    let f = Fixture::new();
    success(f.tool("tool_load"));
    let path = "docs/provenance/developer-tools-v1.toml";
    let valid = fs::read_to_string(f.0.join(path)).unwrap();
    for malformed in [
        format!("{valid}unknown = \"value\"\n"),
        format!("{valid}target = \"aarch64-apple-darwin\"\n"),
        valid.replace("schema_version = \"1\"\n", ""),
        valid.replace("\"1\"", "\"2\""),
        valid.replace("rust-toolchain.toml", "../untrusted"),
        valid.replace("0.20.2", "0..2"),
        valid.replace("fe67d82a", "FE67D82A"),
        "x".repeat(4097),
    ] {
        f.write(path, &malformed);
        rejected(f.tool("tool_load"), "UNVERIFIABLE");
    }
}

#[test]
fn pinned_rust_ignores_global_default_and_path_tools_without_running_them() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("ambient")).unwrap();
    for tool in ["cargo", "rustc", "protoc", "cargo-deny"] {
        let path = f.0.join("ambient").join(tool);
        fs::write(&path, "#!/bin/sh\ntouch ambient-executed\nexit 99\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let output = success(f.shell("export RUSTUP_TOOLCHAIN=nonexistent-global-override\nexport PATH=$repository_root/ambient:$PATH\n. scripts/toolchain-environment.sh\ntoolchain_rust\ncargo --version"));
    assert!(output.starts_with("cargo 1.98.0 "));
    assert!(!f.0.join("ambient-executed").exists());
    success(f.shell(". scripts/toolchain-environment.sh; tool_entry_fingerprint=a; toolchain_observe() { tool_fingerprint=a; }; toolchain_environment_finish"));
    rejected(f.shell(". scripts/toolchain-environment.sh; tool_entry_fingerprint=a; toolchain_observe() { tool_fingerprint=b; }; toolchain_environment_finish"), "environment changed");
}

#[test]
fn isolated_verifier_checks_bytes_before_execution_and_rejects_links_and_partial_files() {
    let f = Fixture::new();
    let binary = success(f.tool("tool_load; tool_directory \"$tool_base\"; echo \"$tool_binary\""));
    let binary = PathBuf::from(binary.trim());
    // Wrong executable must never be called, even if it would report the right version.
    fs::write(
        &binary,
        "#!/bin/sh\ntouch executed\necho cargo-deny 0.20.2\n",
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    rejected(f.tool("tool_resolve"), "UNVERIFIABLE");
    rejected(f.tool("tool_prepare"), "not overwritten");
    assert!(!f.0.join("executed").exists());
    fs::remove_file(&binary).unwrap();
    symlink(f.0.join("rust-toolchain.toml"), &binary).unwrap();
    rejected(f.tool("tool_resolve"), "UNVERIFIABLE");
    fs::remove_file(&binary).unwrap();
    fs::create_dir(&binary).unwrap();
    rejected(f.tool("tool_resolve"), "UNVERIFIABLE");
    fs::remove_dir(&binary).unwrap();
    fs::set_permissions(
        f.0.join("target/mengxia-tools"),
        fs::Permissions::from_mode(0o777),
    )
    .unwrap();
    rejected(
        f.tool("tool_directory \"$repository_root/target/mengxia-tools\""),
        "unsafe tool directory",
    );
}

#[test]
fn publication_is_atomic_no_clobber_and_concurrent_and_rejects_tamper() {
    let f = Fixture::new();
    f.write("candidate", "fixture only; never executed");
    fs::set_permissions(f.0.join("candidate"), fs::Permissions::from_mode(0o500)).unwrap();
    let hash: String = Sha256::digest(b"fixture only; never executed")
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let command = format!(
        ". scripts/toolchain-tools.sh; tool_binary=$repository_root/published; tool_binary_sha={hash}; tool_publish \"$repository_root/candidate\""
    );
    let mut children = Vec::new();
    for _ in 0..4 {
        children.push(
            Command::new("/bin/sh")
                .args(["-eu", "-c", &command])
                .env("repository_root", &f.0)
                .current_dir(&f.0)
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    assert_eq!(
        fs::read(f.0.join("published")).unwrap(),
        b"fixture only; never executed"
    );
    success(f.shell(&command));
    fs::remove_file(f.0.join("published")).unwrap();
    f.write("published", "untrusted existing winner");
    rejected(f.shell(&command), "publication rejected");
    assert_eq!(
        fs::read_to_string(f.0.join("published")).unwrap(),
        "untrusted existing winner"
    );
    fs::remove_file(f.0.join("published")).unwrap();
    fs::create_dir(f.0.join("published")).unwrap();
    rejected(f.shell(&command), "publication rejected");
    assert_eq!(fs::read_dir(f.0.join("published")).unwrap().count(), 0);
    fs::remove_dir(f.0.join("published")).unwrap();
    symlink(f.0.join("scripts"), f.0.join("published")).unwrap();
    rejected(f.shell(&command), "publication rejected");
    assert!(!f.0.join("scripts/candidate").exists());
}

#[test]
fn cargo_hot_cache_reproduces_untracked_change_and_tracks_new_environment_input() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname=\"maint003-cache-fixture\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n[lib]\npath=\"lib.rs\"\n");
    f.write("lib.rs", "pub fn fixture() {}\n");
    let build = r#"fn main() {
        println!("cargo:rerun-if-changed=build.rs");
        let n: u32 = std::fs::read_to_string("runs").unwrap_or_else(|_| "0".into()).parse().unwrap();
        std::fs::write("runs", (n+1).to_string()).unwrap();
    }"#;
    f.write("build.rs", build);
    let run = |value: &str| {
        success(
            Command::new(env!("CARGO"))
                .args(["check", "--offline"])
                .env("CARGO_TARGET_DIR", f.0.join("target"))
                .env("MENGXIA_TOOLCHAIN_FINGERPRINT", value)
                .current_dir(&f.0)
                .output()
                .unwrap(),
        );
        fs::read_to_string(f.0.join("runs")).unwrap()
    };
    assert_eq!(run("old"), "1");
    assert_eq!(
        run("new"),
        "1",
        "untracked external change must reproduce the stale-build gap"
    );
    for path in [
        "crates/mengxia-platform-fs/build.rs",
        "third_party/libsqlite3-sys-0.38.2/build.rs",
    ] {
        let source = fs::read_to_string(support::workspace_root().join(path)).unwrap();
        assert!(source.contains("cargo:rerun-if-env-changed=MENGXIA_TOOLCHAIN_FINGERPRINT"));
        assert!(!source.contains("cargo:rerun-if-changed=/var/db/xcode_select_link"));
    }
    f.write(
        "build.rs",
        &build.replace(
            "fn main() {",
            "fn main() { println!(\"cargo:rerun-if-env-changed=MENGXIA_TOOLCHAIN_FINGERPRINT\");",
        ),
    );
    assert_eq!(run("new"), "2");
    assert_eq!(
        run("new"),
        "2",
        "unchanged environment retains incremental work"
    );
    assert_eq!(run("different-bytes-same-version"), "3");
    assert_eq!(run("os-build-changed"), "4");
}

#[test]
fn security_events_fail_closed_and_unknown_coverage_is_explicit() {
    let f = Fixture::new();
    let path = "docs/provenance/toolchain-security-events-v1.tsv";
    let header = "# mengxia-toolchain-security-events-v1\n";
    let output = success(f.shell("/bin/sh scripts/toolchain-maintenance.sh report"));
    assert!(output.contains("TOOL_SECURITY_COVERAGE: PARTIAL"));
    assert!(output.contains("CONTINUOUS_REPAIR_AGENT: NOT_ENABLED"));
    for state in ["AFFECTED", "UNKNOWN"] {
        f.write(path, &format!("{header}CVE-TEST|rust|{state}|https://example.org/advisory|https://example.org/review|2026-09-13\n"));
        rejected(
            f.shell("/bin/sh scripts/toolchain-maintenance.sh gate"),
            "SECURITY_UPDATE_REQUIRED",
        );
    }
    for state in ["RESOLVED", "NOT_APPLICABLE"] {
        f.write(path, &format!("{header}CVE-TEST|rust|{state}|https://example.org/advisory|https://example.org/review|2026-09-13\n"));
        success(f.shell("/bin/sh scripts/toolchain-maintenance.sh gate"));
    }
    for row in [
        "bad",
        "CVE-TEST|rust|EXCEPTION|https://example.org/a|https://example.org/b|2026-09-13",
        "CVE-TEST|rust|RESOLVED|http://example.org/a|https://example.org/b|2026-09-13",
    ] {
        f.write(path, &format!("{header}{row}\n"));
        rejected(
            f.shell("/bin/sh scripts/toolchain-maintenance.sh gate"),
            "UNVERIFIABLE",
        );
    }
    for record in [
        String::new(),
        format!(
            "{header}CVE-TEST|rust|RESOLVED|https://example.org/a|https://example.org/b|2999-01-01"
        ),
        format!(
            "{header}CVE-TEST|rust|RESOLVED|https://example.org/a|https://example.org/b|2026-99-99\n"
        ),
        format!(
            "{header}{}",
            "CVE-TEST|rust|RESOLVED|https://example.org/a|https://example.org/b|2026-01-01\n"
                .repeat(2)
        ),
    ] {
        f.write(path, &record);
        rejected(
            f.shell("/bin/sh scripts/toolchain-maintenance.sh gate"),
            "UNVERIFIABLE",
        );
    }
}

#[test]
fn integration_keeps_existing_gates_and_tools_ignored_and_planning_authority_scoped() {
    let root = support::workspace_root();
    let read = |p: &str| fs::read_to_string(root.join(p)).unwrap();
    for script in ["scripts/verify-ci-fast.sh", "scripts/verify-repository.sh"] {
        assert!(read(script).contains("toolchain_environment"));
        assert_eq!(
            read(script)
                .matches("scripts/verify-toolchain-maintenance.sh")
                .count(),
            1
        );
    }
    let candidate = read("scripts/verify-toolchain-candidate.sh");
    for marker in [
        "mktemp -d",
        "CARGO_TARGET_DIR=$candidate_directory/build",
        "runtime::tests",
        "candidate ran no runtime tests",
        "environment changed during candidate",
    ] {
        assert!(candidate.contains(marker));
    }
    let deps = read(".github/dependabot.yml");
    let cargo = deps
        .split("package-ecosystem: \"github-actions\"")
        .next()
        .unwrap();
    assert!(cargo.contains("open-pull-requests-limit: 0"));
    assert!(cargo.contains("applies-to: security-updates"));
    assert!(!cargo.contains("ignore:"));
    let workflow = read(".github/workflows/ci.yml");
    for id in [
        "TEST-MAINT3-ENV-001",
        "TEST-MAINT3-INSTALL-001",
        "TEST-MAINT3-CACHE-001",
        "TEST-MAINT3-SECURITY-001",
        "TEST-MAINT3-INTEGRATION-001",
    ] {
        assert_eq!(workflow.matches(id).count(), 1);
        assert_eq!(read("scripts/verify-repository.sh").matches(id).count(), 1);
        assert_eq!(
            read("scripts/verify-toolchain-maintenance.sh")
                .matches(id)
                .count(),
            1
        );
    }
    assert!(workflow.contains("scripts/dev-toolchain.sh prepare --network"));
    assert!(workflow.contains("scripts/verify-task-003-formal-second-uid.sh component"));
    assert!(!workflow.contains("pull_request_target"));
    assert!(read("scripts/check-supply-chain.sh").contains("tool_resolve"));
    assert!(
        read("scripts/check-supply-chain.sh").contains("scripts/toolchain-maintenance.sh gate")
    );
    for path in [
        "target/mengxia-tools/cargo-deny",
        "target/toolchain-evidence/candidate.1/native.log",
    ] {
        assert!(
            Command::new("git")
                .args(["check-ignore", "--quiet", path])
                .current_dir(&root)
                .status()
                .unwrap()
                .success()
        );
    }
    let adr = read("docs/spec/adr/ADR-0016-toolchain-maintenance.md");
    assert!(adr.contains("Status: ACCEPTED"));
    assert!(adr.contains("No Cargo manifest/lock"));
    assert!(read("AGENTS.md").contains("MAINT003_PRODUCT_AUTHORITY: NONE"));
}
