#[path = "support/ci_mappings.rs"]
mod ci_mappings;
#[path = "support/lifecycle.rs"]
mod lifecycle;
mod support;

use std::collections::BTreeSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "mengxia-ci-evidence-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("scripts")).unwrap();
        for name in ["ci-evidence.sh", "ci-baseline-mappings.txt"] {
            fs::copy(
                support::workspace_root().join("scripts").join(name),
                path.join("scripts").join(name),
            )
            .unwrap();
        }
        Self(path)
    }

    fn script(&self, name: &str, body: &str) {
        let path = self.0.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    fn shell(&self, body: &str) -> Output {
        Command::new("/bin/sh")
            .args(["-eu", "-c", body])
            .current_dir(&self.0)
            .env(
                "PATH",
                format!("{}:{}", self.0.display(), std::env::var("PATH").unwrap()),
            )
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn shared_groups_execute_once_and_never_attribute_failed_or_unknown_checks() {
    let fixture = Fixture::new();
    let body = "native=1; . scripts/ci-evidence.sh; check() { echo executed; }; ci_run_group 'TEST-INGEST-007 TEST-CUSTODY-007 TEST-COMMAND-007' check";
    let output = fixture.shell(body);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(text.matches("executed").count(), 1);
    assert_eq!(text.matches(": COMPONENT_PASS").count(), 3);
    assert!(!text.contains(": PASS"));
    for body in [
        "native=1; . scripts/ci-evidence.sh; check() { false; echo incorrectly-continued; }; ci_run_group 'TEST-INGEST-007 TEST-CUSTODY-007' check",
        "native=1; . scripts/ci-evidence.sh; ci_run_group 'TEST-UNKNOWN-999' true",
        "native=1; . scripts/ci-evidence.sh; ci_run_group '' true",
        "native=1; . scripts/ci-evidence.sh; ci_run_group '  ' true",
        "native=1; . scripts/ci-evidence.sh; ci_run_group 'TEST-INGEST-007 TEST-INGEST-007' true",
    ] {
        let output = fixture.shell(body);
        assert!(!output.status.success(), "{body}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(!text.contains("PASS"), "{text}");
        assert!(!text.contains("incorrectly-continued"));
    }
    // No cache: a second invocation, changed mode or a failed command must run.
    let output = fixture.shell("native=1; . scripts/ci-evidence.sh; ci_run_group 'TEST-INGEST-007' true; native=0; mode=developer; ci_run_group 'TEST-INGEST-007' false");
    assert!(!output.status.success());
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("FAST_PASS")
    );
}

#[test]
fn standalone_labels_and_mapping_inventory_retain_exact_obligations() {
    let fixture = Fixture::new();
    for task in 5..=10 {
        let script = fs::read_to_string(
            support::workspace_root().join(format!("scripts/verify-task-{task:03}.sh")),
        )
        .unwrap();
        let body = script
            .split_once("run() {\n")
            .unwrap()
            .1
            .split_once("\n}\n")
            .unwrap()
            .0;
        let output = fixture.shell(&format!("native=1; mode=formal; . scripts/ci-evidence.sh; run() {{\n{body}\n}}\nrun TEST-BOOT-002"));
        assert!(
            !output.status.success(),
            "TASK-{task:03} must reject a missing command"
        );
        assert!(!String::from_utf8(output.stdout).unwrap().contains("PASS"));
    }
    for (mode, label) in [("developer", "FAST_PASS"), ("formal", "PASS")] {
        let output = fixture.shell(&format!(
            "native=0; mode={mode}; . scripts/ci-evidence.sh; ci_run_group 'TEST-INGEST-007' true"
        ));
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            format!("TEST-INGEST-007: {label}")
        );
    }
    let mappings = ci_mappings::parse("# run TEST-A-001 fake\nrun TEST-B-001 cargo test x\nci_run_group 'TEST-C-001 TEST-D-001' check\n").unwrap();
    assert_eq!(mappings.len(), 3);
    assert!(!mappings.contains_key("TEST-A-001"));
    assert_eq!(mappings["TEST-C-001"], mappings["TEST-D-001"]);
    for invalid in [
        "run TEST-A-001 ",
        "run TEST-A-001 # cargo test",
        "run TEST-A-001   # cargo test",
        "ci_run_group '' check",
        "ci_run_group 'TEST-A-001 TEST-A-001' check",
        "ci_run_group 'TEST-A-001' check\nrun TEST-A-001 other",
        "ci_run_group 'invalid' check",
        "run TEST-A-001 true; cargo test",
    ] {
        assert!(ci_mappings::parse(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn cargo_evidence_rejects_zero_matches_ignored_only_and_failed_processes() {
    let fixture = Fixture::new();
    for (summary, status, accepted) in [
        ("test result: ok. 0 passed; 0 failed; 4 ignored;", 0, false),
        ("test result: ok. 1 passed; 0 failed; 0 ignored;", 1, false),
        (
            "test result: FAILED. 0 passed; 1 failed; 0 ignored;",
            1,
            false,
        ),
        ("no test harness output", 0, false),
        ("test result: ok. 1 passed; 0 failed; 0 ignored;", 0, true),
    ] {
        fixture.script(
            "cargo",
            &format!("#!/bin/sh\nprintf '%s\\n' '{summary}'\nexit {status}\n"),
        );
        let output = fixture.shell("native=1; . scripts/ci-evidence.sh; ci_run_group 'TEST-INGEST-007' cargo test --locked --offline --exact missing");
        assert_eq!(output.status.success(), accepted, "{summary}");
        assert_eq!(
            String::from_utf8(output.stdout)
                .unwrap()
                .contains("COMPONENT_PASS"),
            accepted
        );
    }
    fixture.script("cargo", "#!/bin/sh\nprintf '%s\\n' \"$*\" \"${MATRIX-}\"\necho 'test result: ok. 1 passed; 0 failed; 0 ignored;'\n");
    let output = fixture.shell("native=1; . scripts/ci-evidence.sh; export MATRIX=100; cargo test --release --features stress -- --ignored --exact crash");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(text.contains("test --release --features stress -- --ignored --exact crash\n100\n"));
}

#[test]
fn supply_is_explicit_and_standalone_never_inherits_a_skip() {
    let fixture = Fixture::new();
    fixture.script("scripts/check-supply-chain.sh", "#!/bin/sh\nif [ \"${1-}\" = --simulate-advisory-unavailable ]; then echo UNVERIFIABLE: fixture; exit 2; fi\necho supply-executed\n");
    let native =
        fixture.shell("native=1; . scripts/ci-evidence.sh; ci_supply_unavailable; ci_supply");
    assert!(native.status.success());
    let text = String::from_utf8(native.stdout).unwrap();
    assert!(text.contains("REQUIRED_BY_AGGREGATE"));
    assert!(!text.contains("supply-executed"));
    let standalone = fixture.shell("export SKIP_SUPPLY=1; native=0; . scripts/ci-evidence.sh; ci_supply_unavailable; ci_supply");
    assert!(standalone.status.success());
    let text = String::from_utf8(standalone.stdout).unwrap();
    assert_eq!(text.matches("supply-executed").count(), 1);
    assert!(text.contains("UNVERIFIABLE:"));
    fixture.script(
        "scripts/check-supply-chain.sh",
        "#!/bin/sh\necho UNVERIFIABLE: fixture\nexit 2\n",
    );
    let output =
        fixture.shell("native=0; . scripts/ci-evidence.sh; ci_supply; ci_result TEST-SUPPLY-001");
    assert!(!output.status.success());
    assert!(!String::from_utf8(output.stdout).unwrap().contains("PASS"));
    // A broken negative probe returning success cannot be mistaken for rejection.
    fixture.script(
        "scripts/check-supply-chain.sh",
        "#!/bin/sh\necho UNVERIFIABLE: fixture\nexit 0\n",
    );
    assert!(
        !fixture
            .shell("native=0; . scripts/ci-evidence.sh; ci_supply_unavailable")
            .status
            .success()
    );
}

fn merge(event: &str, scope: &str, overrides: &[(&str, &str)]) -> bool {
    let sha = "0123456789abcdef0123456789abcdef01234567";
    let code = scope == "code";
    let mut command =
        Command::new(support::workspace_root().join("scripts/check-ci-merge-gate.sh"));
    command
        .env("EVENT_NAME", event)
        .env("SCOPE", scope)
        .env("CLASSIFY_RESULT", "success")
        .env("EXPECTED_SHA", sha)
        .env("FORMAL_SHA", sha)
        .env("SECOND_UID_SHA", sha)
        .env("SUPPLY_SHA", sha)
        .env("FORMAL_RESULT", if code { "success" } else { "skipped" })
        .env(
            "SECOND_UID_RESULT",
            if code { "success" } else { "skipped" },
        )
        .env("SUPPLY_RESULT", if code { "success" } else { "skipped" })
        .env(
            "PR_VALIDATION_RESULT",
            if !code || event == "pull_request" {
                "success"
            } else {
                "skipped"
            },
        )
        .env(
            "DEPENDENCY_RESULT",
            if event == "pull_request" {
                "success"
            } else {
                "skipped"
            },
        );
    for (key, value) in overrides {
        command.env(key, value);
    }
    command.output().unwrap().status.success()
}

#[test]
fn merge_gate_executes_event_scope_result_and_identity_failure_matrix() {
    for event in ["pull_request", "push", "schedule", "workflow_dispatch"] {
        assert!(merge(event, "code", &[]), "{event}");
        for key in [
            "CLASSIFY_RESULT",
            "FORMAL_RESULT",
            "SECOND_UID_RESULT",
            "SUPPLY_RESULT",
        ] {
            for bad in [
                "failure",
                "cancelled",
                "timed_out",
                "skipped",
                "neutral",
                "",
                "unknown",
            ] {
                assert!(!merge(event, "code", &[(key, bad)]), "{event}/{key}/{bad}");
            }
        }
        for key in ["FORMAL_SHA", "SECOND_UID_SHA", "SUPPLY_SHA", "EXPECTED_SHA"] {
            for bad in ["", "old-sha", "ffffffffffffffffffffffffffffffffffffffff"] {
                assert!(!merge(event, "code", &[(key, bad)]), "{event}/{key}/{bad}");
            }
        }
        assert!(!merge(event, "unknown", &[]));
        assert!(!merge(event, "", &[]));
        let docs = matches!(event, "pull_request" | "push");
        assert_eq!(merge(event, "docs", &[]), docs);
        if docs {
            assert!(!merge(
                event,
                "docs",
                &[("PR_VALIDATION_RESULT", "skipped")]
            ));
            for key in ["FORMAL_RESULT", "SECOND_UID_RESULT", "SUPPLY_RESULT"] {
                assert!(!merge(event, "docs", &[(key, "success")]));
                assert!(!merge(event, "docs", &[(key, "failure")]));
            }
        }
    }
    for key in ["PR_VALIDATION_RESULT", "DEPENDENCY_RESULT"] {
        for bad in ["skipped", "failure", "cancelled", "neutral", ""] {
            assert!(!merge("pull_request", "code", &[(key, bad)]));
        }
    }
    assert!(!merge("pull_request_target", "code", &[]));
    assert!(!merge(
        "push",
        "code",
        &[("PR_VALIDATION_RESULT", "success")]
    ));
    assert!(!merge("push", "code", &[("DEPENDENCY_RESULT", "success")]));
}

fn identifiers(text: &str) -> BTreeSet<&str> {
    text.split(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'))
        .filter(|word| word.starts_with("TEST-"))
        .collect()
}

#[test]
fn retained_ids_are_exact_and_deduplication_is_only_four_explicit_groups() {
    let root = support::workspace_root();
    let baseline = fs::read_to_string(root.join("scripts/ci-baseline-mappings.txt")).unwrap();
    let expected: BTreeSet<_> = baseline.lines().collect();
    assert_eq!(
        expected.len(),
        baseline.lines().count(),
        "duplicate mapping"
    );
    let mut source = String::new();
    for task in 1..=10 {
        source.push_str(
            &fs::read_to_string(root.join(format!("scripts/verify-task-{task:03}.sh"))).unwrap(),
        );
    }
    source.push_str(&fs::read_to_string(root.join("scripts/verify-maint-001.sh")).unwrap());
    let mut actual = identifiers(&source);
    actual.insert("TEST-IPC-MACOS-001"); // Separate real-UID job, never local aggregate.
    assert_eq!(actual, expected, "every stable ID needs its owned mapping");
    assert_eq!(source.matches("ci_run_group '").count(), 4);
    for group in [
        "ci_run_group 'TEST-INGEST-007 TEST-CUSTODY-007 TEST-COMMAND-007' application_tests",
        "ci_run_group 'TEST-VERIFY-008 TEST-CORRUPTION-008' verification_tests",
        "ci_run_group 'TEST-PROJECT-009 TEST-SUBJECT-009' project_subject_tests",
        "ci_run_group 'TEST-WORK-009 TEST-TAKE-009' work_take_tests",
    ] {
        assert!(source.contains(group), "missing explicit group {group}");
    }
    let native = fs::read_to_string(root.join("scripts/verify-repository.sh")).unwrap();
    assert_eq!(native.matches("scripts/verify-ci-supply.sh").count(), 1);
    assert!(!native.contains("SKIP_SUPPLY"));
    assert!(native.contains("if [ \"$repository_mode\" = formal-native ]; then"));
    assert!(source.contains("MENGXIA_TASK007_STRESS_ITERATIONS=100"));
    assert!(source.contains("--release"));
    assert!(source.contains("--ignored"));
}

#[test]
fn lifecycle_record_is_closed_and_done_requires_evidence_without_rule_changes() {
    let root = support::workspace_root();
    let current = fs::read_to_string(root.join("docs/spec/task-lifecycle-records.toml")).unwrap();
    let record = lifecycle::parse(&current).unwrap();
    for file in ["AGENTS.md", "docs/spec/IMPLEMENTATION_SPEC.md"] {
        let text = fs::read_to_string(root.join(file)).unwrap();
        assert!(text.contains(&format!(
            "MAINT002_LIFECYCLE: {}",
            record["maintenance.status"]
        )));
        assert!(text.contains(&format!(
            "MAINT002_IMPLEMENTATION_AUTHORITY: {}",
            record["maintenance.authority"]
        )));
    }
    let start = "[versions]\nspecification=\"1.1.45\"\ndecisions=\"0.3.46\"\nreview=\"1.1.56\"\nplan=\"0.3.56\"\nintake=\"1.3.51\"\n[maintenance]\nid=\"MAINT-002\"\nstatus=\"IN_PROGRESS\"\nauthority=\"MAINT_002_CI_ONLY\"\npr_head=\"PENDING\"\npr_run=\"PENDING\"\nmain_head=\"PENDING\"\nmain_run=\"PENDING\"\n";
    assert!(lifecycle::parse(start).is_ok());
    let done = start
        .replace("IN_PROGRESS", "DONE")
        .replace("MAINT_002_CI_ONLY", "NONE")
        .replace(
            "head=\"PENDING\"",
            "head=\"0123456789abcdef0123456789abcdef01234567\"",
        )
        .replace("run=\"PENDING\"", "run=\"123\"");
    assert!(lifecycle::parse(&done).is_ok());
    for invalid in [
        start.replace("IN_PROGRESS", "DONE"),
        start.replace("MAINT_002_CI_ONLY", "PRODUCT_ALL"),
        start.replace("1.1.45", "01.1.45"),
        start.replace("1.1.45", "v1.1"),
        start.replace("id=\"MAINT-002\"\n", ""),
        format!("{start}status=\"DONE\"\n"),
        format!("{start}[maintenance]\n"),
        format!("{start}command=\"skip\"\n"),
        done.replace("run=\"123\"", "run=\"0\""),
        done.replace("0123456789abcdef0123456789abcdef01234567", "0123"),
        done.replace("NONE", "MAINT_002_CI_ONLY"),
        "x".repeat(4097),
    ] {
        assert!(lifecycle::parse(&invalid).is_err(), "{invalid}");
    }
}
