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
        "pull_request:",
        "push:\n    branches:\n      - main",
        "workflow_dispatch:",
        "github.event.pull_request.number || github.sha",
        "cancel-in-progress: true",
        "scripts/classify-ci-change.sh",
        "run: scripts/verify-repository.sh docs",
        "run: scripts/verify-repository.sh developer",
        "run: scripts/verify-repository.sh formal",
        "task-003-second-uid:",
        "run: scripts/verify-task-003-formal-second-uid.sh component",
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
    assert!(!workflow.contains("run: scripts/verify-task-007.sh formal"));
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
    ] {
        assert_eq!(driver.matches(exact).count(), 1, "driver mapping {exact}");
    }
    assert!(!driver.contains("verify-repository.sh \"$mode\""));

    let task_003 = fs::read_to_string(root.join("scripts/verify-task-003.sh")).unwrap();
    let task_004 = fs::read_to_string(root.join("scripts/verify-task-004.sh")).unwrap();
    let task_005 = fs::read_to_string(root.join("scripts/verify-task-005.sh")).unwrap();
    let task_006 = fs::read_to_string(root.join("scripts/verify-task-006.sh")).unwrap();
    let task_007 = fs::read_to_string(root.join("scripts/verify-task-007.sh")).unwrap();
    for script in [&task_003, &task_004, &task_005, &task_006, &task_007] {
        assert!(script.contains("component=0"));
        assert!(script.contains("[ \"$component\" -eq 0 ]"));
    }

    for retained_formal in [
        "task_005_generated_scaling_evidence",
        "asset_migration_sigkill_before_and_after_commit_recovers_exactly",
        "MENGXIA_TASK007_STRESS_ITERATIONS=100",
    ] {
        assert!(
            format!("{task_005}\n{task_006}\n{task_007}").contains(retained_formal),
            "formal component lost {retained_formal}"
        );
    }
}
