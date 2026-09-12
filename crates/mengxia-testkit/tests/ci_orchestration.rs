mod support;

use std::fs;
use std::process::Command;

use support::workspace_root;

fn classify(paths: &[&str]) -> String {
    let root = workspace_root();
    let output = Command::new(root.join("scripts/classify-ci-change.sh"))
        .arg("--paths")
        .args(paths)
        .current_dir(&root)
        .output()
        .expect("CI classifier must start");
    assert!(
        output.status.success(),
        "classifier failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn documentation_classification_is_exact_and_fail_closed() {
    assert_eq!(classify(&["docs/spec/IMPLEMENTATION_SPEC.md"]), "docs");
    assert_eq!(
        classify(&["docs/proposals/TASK-007-GATE-PROPOSAL.md"]),
        "docs"
    );
    assert_eq!(
        classify(&["AGENTS.md", "docs/spec/adr/ADR-0010.md"]),
        "docs"
    );

    for paths in [
        vec![],
        vec!["docs"],
        vec!["README.md"],
        vec![".github/workflows/ci.yml"],
        vec!["scripts/verify-repository.sh"],
        vec!["docs/provenance/macos-acl-ffi-toolchain-v1.toml"],
        vec!["docs/future-subtree/file.md"],
        vec!["docs/spec/IMPLEMENTATION_SPEC.md", "Cargo.toml"],
        vec!["docs/../Cargo.toml"],
    ] {
        assert_eq!(classify(&paths), "code", "must fail closed for {paths:?}");
    }

    let root = workspace_root();
    let output = Command::new(root.join("scripts/classify-ci-change.sh"))
        .args(["definitely-not-a-commit", "also-not-a-commit"])
        .current_dir(&root)
        .output()
        .expect("CI classifier must start");
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "code");
}

#[test]
fn workflow_trigger_and_evidence_matrix_is_layered() {
    let root = workspace_root();
    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();

    for required in [
        "name: Layered TASK-001 through TASK-010 repository gates",
        "pull_request:",
        "push:\n    branches:\n      - main",
        "workflow_dispatch:",
        "schedule:",
        "cron: '17 3 * * 1'",
        "github.event.pull_request.number || github.sha",
        "cancel-in-progress: true",
        "scripts/classify-ci-change.sh",
        "schedule) scope=code",
        "/bin/sh scripts/verify-macos-acl-toolchain.sh --select-attested",
        "run: scripts/verify-repository.sh docs",
        "run: scripts/verify-repository.sh developer",
        "run: scripts/verify-repository.sh formal",
        "task-003-second-uid:",
        "run: scripts/verify-task-003-formal-second-uid.sh component",
        "dependency-review:",
        "uses: actions/dependency-review-action@a1d282b36b6f3519aa1f3fc636f609c47dddb294",
        "uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1",
        "merge-gate:",
        "name: Merge gate",
        "if: always() && github.event_name == 'pull_request'",
        "MENGXIA_ACL_BUILD_CLASS: attested",
        "runs-on: macos-26",
    ] {
        assert!(
            workflow.contains(required),
            "workflow is missing {required}"
        );
    }
    assert_eq!(
        workflow
            .matches("scripts/verify-repository.sh formal")
            .count(),
        1
    );
    assert_eq!(workflow.matches("cargo install cargo-deny").count(), 2);
    assert_eq!(
        workflow
            .matches("actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1")
            .count(),
        4
    );
    assert!(!workflow.contains("actions/checkout@11bd71901"));
    assert_eq!(
        workflow
            .matches("scripts/verify-macos-acl-toolchain.sh --select-attested")
            .count(),
        3
    );
    assert!(!workflow.contains("xcode-select --switch /Applications/"));
    assert!(!workflow.contains("run: scripts/verify-task-007.sh formal"));
    assert!(!workflow.contains("pull_request_target"));
    assert!(workflow.contains("permissions:\n  contents: read"));
    assert!(workflow.contains("FORMAL_RESULT: ${{ needs.repository-gates.result }}"));
    assert!(workflow.contains("SECOND_UID_RESULT: ${{ needs.task-003-second-uid.result }}"));
    assert!(workflow.contains("DEPENDENCY_RESULT: ${{ needs.dependency-review.result }}"));
    assert!(workflow.contains("PR_VALIDATION_RESULT: ${{ needs.pull-request-validation.result }}"));
}

#[test]
fn public_repository_review_files_are_bounded_and_review_only() {
    let root = workspace_root();
    let dependabot = fs::read_to_string(root.join(".github/dependabot.yml")).unwrap();
    for exact in [
        "package-ecosystem: \"cargo\"",
        "package-ecosystem: \"github-actions\"",
        "interval: \"monthly\"",
        "applies-to: security-updates",
        "open-pull-requests-limit: 2",
    ] {
        assert!(
            dependabot.contains(exact),
            "dependabot config lacks {exact}"
        );
    }
    for forbidden in ["automerge", "target-branch:"] {
        assert!(!dependabot.contains(forbidden));
    }

    let owners = fs::read_to_string(root.join(".github/CODEOWNERS")).unwrap();
    for protected in [
        "/.github/",
        "/docs/provenance/",
        "/migrations/",
        "/proto/",
        "/third_party/",
        "/crates/mengxia-platform-fs/",
    ] {
        assert!(owners.lines().any(|line| line.starts_with(protected)));
    }
    assert!(
        owners.lines().all(|line| {
            line.is_empty() || line.starts_with('#') || line.ends_with(" @XiaTian-X")
        })
    );

    let security = fs::read_to_string(root.join("SECURITY.md")).unwrap();
    assert!(security.contains("security/advisories/new"));
    assert!(security.contains("Do not open a public issue"));
}

#[test]
fn maintenance_evidence_ids_have_one_executable_mapping() {
    let root = workspace_root();
    let driver = fs::read_to_string(root.join("scripts/verify-maint-001.sh")).unwrap();
    for test_id in [
        "TEST-MAINT-CI-001",
        "TEST-MAINT-TOOLCHAIN-001",
        "TEST-MAINT-SUPPLY-001",
        "TEST-MAINT-DOC-001",
    ] {
        assert_eq!(
            driver.matches(test_id).count(),
            1,
            "maintenance mapping for {test_id} must be unique"
        );
    }
    assert_eq!(
        driver.matches("TEST-MAINT-PROTO-001").count(),
        2,
        "proto evidence must have exactly one developer and one formal mapping"
    );
    assert!(driver.contains("maint_run TEST-MAINT-PROTO-001 ./scripts/verify-proto-artifacts.sh"));
    assert!(driver.contains(
        "maint_run TEST-MAINT-PROTO-001 cargo test --locked --offline -p mengxia-testkit --test task_003_foundation descriptor_and_offline_generator_inputs_are_source_pinned"
    ));
}

#[test]
fn repository_driver_has_one_baseline_and_one_component_per_task() {
    let root = workspace_root();
    let driver = fs::read_to_string(root.join("scripts/verify-repository.sh")).unwrap();
    for exact in [
        "scripts/verify-task-001.sh",
        "scripts/verify-task-002.sh",
        "scripts/verify-task-004.sh --component",
        "scripts/verify-task-003.sh component",
        "scripts/verify-task-005.sh \"$mode\" component",
        "scripts/verify-task-006.sh \"$mode\" component",
        "scripts/verify-task-007.sh \"$mode\" component",
        "scripts/verify-task-008.sh \"$mode\" component",
        "scripts/verify-task-009.sh \"$mode\" component",
        "scripts/verify-task-010.sh \"$mode\" component",
        "scripts/verify-maint-001.sh \"$mode\"",
    ] {
        assert_eq!(driver.matches(exact).count(), 1, "driver mapping {exact}");
    }
    assert!(!driver.contains("verify-repository.sh \"$mode\""));

    let task_003 = fs::read_to_string(root.join("scripts/verify-task-003.sh")).unwrap();
    let task_004 = fs::read_to_string(root.join("scripts/verify-task-004.sh")).unwrap();
    let task_005 = fs::read_to_string(root.join("scripts/verify-task-005.sh")).unwrap();
    let task_006 = fs::read_to_string(root.join("scripts/verify-task-006.sh")).unwrap();
    let task_007 = fs::read_to_string(root.join("scripts/verify-task-007.sh")).unwrap();
    let task_008 = fs::read_to_string(root.join("scripts/verify-task-008.sh")).unwrap();
    let task_009 = fs::read_to_string(root.join("scripts/verify-task-009.sh")).unwrap();
    let task_010 = fs::read_to_string(root.join("scripts/verify-task-010.sh")).unwrap();
    for script in [
        &task_003, &task_004, &task_005, &task_006, &task_007, &task_008, &task_009, &task_010,
    ] {
        assert!(script.contains("component=0"));
        assert!(script.contains("[ \"$component\" -eq 0 ]"));
    }

    for retained_formal in [
        "task_005_generated_scaling_evidence",
        "asset_migration_sigkill_before_and_after_commit_recovers_exactly",
        "MENGXIA_TASK007_STRESS_ITERATIONS=100",
    ] {
        assert!(
            format!("{task_005}\n{task_006}\n{task_007}\n{task_008}\n{task_009}\n{task_010}")
                .contains(retained_formal),
            "formal component lost {retained_formal}"
        );
    }
}
