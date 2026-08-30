mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use support::workspace_root;

const NAMESPACES: &[&str] = &[
    "G", "FUNC", "REQ", "DATA", "API", "SEC", "REL", "PERF", "OPS", "CFG", "AC", "TEST", "TASK",
    "OQ", "DEC", "RISK", "SRC", "ADR", "BASE", "CONFLICT", "REVIEW", "BASELINE",
];

#[derive(Debug)]
struct Document {
    path: PathBuf,
    text: String,
}

#[test]
fn canonical_documents_have_closed_stable_id_traceability() {
    let root = workspace_root();
    let documents = load_documents(&root.join("docs/spec"));
    let definitions =
        collect_definitions(&documents).expect("canonical definitions must be unique");

    let mut unknown = BTreeMap::<String, BTreeSet<PathBuf>>::new();
    for document in &documents {
        for id in extract_ids(&document.text) {
            if !definitions.contains_key(&id) {
                unknown.entry(id).or_default().insert(document.path.clone());
            }
        }
        validate_ranges(&document.text, &definitions)
            .unwrap_or_else(|error| panic!("{}: {error}", document.path.display()));
    }
    assert!(
        unknown.is_empty(),
        "unknown stable-ID references: {unknown:#?}"
    );

    let plan = documents
        .iter()
        .find(|document| document.path.ends_with("IMPLEMENTATION_PLAN.md"))
        .expect("implementation plan is present");
    validate_task_001_record(&plan.text, &definitions)
        .expect("TASK-001 lifecycle record must be complete");
    validate_task_002_record(&plan.text, &definitions)
        .expect("TASK-002 lifecycle record must be complete");
    validate_task_dependency_graph(&plan.text)
        .expect("task dependency graph and downstream invariants must remain valid");

    let specification = document_text(&documents, "IMPLEMENTATION_SPEC.md");
    validate_future_task_acceptance_alignment(specification, &plan.text)
        .expect("future task acceptance mappings must agree between specification and plan");
    let review = document_text(&documents, "IMPLEMENTATION_REVIEW.md");
    let intake = document_text(&documents, "PROJECT_INTAKE_REPORT.md");
    let agents = fs::read_to_string(root.join("AGENTS.md")).expect("AGENTS.md is readable");
    let proposal = fs::read_to_string(root.join("docs/proposals/TASK-002-GATE-PROPOSAL.md"))
        .expect("accepted TASK-002 proposal is readable");
    validate_task_002_current_state(
        &plan.text,
        specification,
        review,
        intake,
        &agents,
        &proposal,
    )
    .expect("TASK-002 current-state documents must agree");

    let task_004_proposal =
        fs::read_to_string(root.join("docs/proposals/TASK-004-GATE-PROPOSAL.md"))
            .expect("TASK-004 accepted contract is readable");
    validate_task_004_active_contract(&plan.text, &task_004_proposal, &definitions)
        .expect("TASK-004 accepted contract and active start record must agree");

    let task_003_proposal =
        fs::read_to_string(root.join("docs/proposals/TASK-003-GATE-PROPOSAL.md"))
            .expect("TASK-003 gate proposal is readable");
    let decisions = documents
        .iter()
        .find(|document| document.path.ends_with("DECISIONS.md"))
        .expect("decisions document is present");
    validate_post_task_005_document_consistency(
        specification,
        &plan.text,
        &decisions.text,
        review,
        intake,
    )
    .expect("post-TASK-005 current state, task dependencies and evidence ownership must agree");
    validate_task_003_gate_state(
        &plan.text,
        &task_003_proposal,
        specification,
        &decisions.text,
        review,
        intake,
        &agents,
    )
    .expect("TASK-003 draft/active lifecycle must agree with its gate proposal");
    validate_task_003_repository_gate_files(&root, &plan.text)
        .expect("DONE TASK-003 must retain executable repository gate mappings");

    let task_005_proposal =
        fs::read_to_string(root.join("docs/proposals/TASK-005-GATE-PROPOSAL.md"))
            .expect("TASK-005 accepted pre-start proposal is readable");
    let task_005_adr =
        fs::read_to_string(root.join("docs/spec/adr/ADR-0007-local-cas-custody-boundary.md"))
            .expect("ADR-0007 is readable");
    validate_task_005_prestart_gate(
        &plan.text,
        &task_005_proposal,
        specification,
        &decisions.text,
        review,
        intake,
        &agents,
        &task_005_adr,
    )
    .expect("TASK-005 accepted pre-start gate must remain synchronized and inactive");

    let task_006_proposal =
        fs::read_to_string(root.join("docs/proposals/TASK-006-GATE-PROPOSAL.md"))
            .expect("TASK-006 accepted contract is readable");
    let task_006_adr = fs::read_to_string(
        root.join("docs/spec/adr/ADR-0008-asset-persistence-and-command-ledger.md"),
    )
    .expect("ADR-0008 is readable");
    validate_task_006_start_gate(
        &plan.text,
        &task_006_proposal,
        specification,
        &decisions.text,
        review,
        intake,
        &agents,
        &task_006_adr,
        &definitions,
    )
    .expect("TASK-006 accepted start gate must remain synchronized and exclusive");

    let adr = documents
        .iter()
        .find(|document| {
            document
                .path
                .ends_with("adr/ADR-0003-foundation-toolchain-and-sqlite.md")
        })
        .expect("ADR-0003 is present");
    validate_task_001_dependencies(&decisions.text, &adr.text)
        .expect("TASK-001 dependencies must remain accepted");
}

#[test]
fn traceability_rules_reject_unknown_duplicate_range_and_dependency_failures() {
    let mut definitions = BTreeMap::new();
    definitions.insert("AC-001".to_owned(), PathBuf::from("spec.md"));
    assert!(validate_references("AC-001 AC-999", &definitions).is_err());

    let duplicate_documents = vec![
        Document {
            path: PathBuf::from("first.md"),
            text: "AC-001\n".to_owned(),
        },
        Document {
            path: PathBuf::from("second.md"),
            text: "AC-001\n".to_owned(),
        },
    ];
    assert!(collect_definitions_for_test(&duplicate_documents, "AC-001").is_err());

    assert!(validate_ranges("AC-001..002", &definitions).is_err());
    assert!(validate_task_001_dependencies("OQ-003 OPEN", "# ADR-0003\nStatus: ACCEPTED").is_err());
    assert!(
        validate_task_dependency_graph(
            "| `TASK-002` Base | `DONE` | x | none |\n\
             | `TASK-003` IPC | `PENDING` | x | TASK-004 |\n\
             | `TASK-004` Store | `PENDING` | x | TASK-003 |",
        )
        .is_err()
    );
    let accepted_graph = "| `TASK-002` Base | `DONE` | x | none |\n\
                          | `TASK-003` IPC | `PENDING` | x | TASK-002, TASK-004 |\n\
                          | `TASK-004` Store | `PENDING` | x | TASK-002 |\n\
                          | `TASK-005` CAS | `PENDING` | x | TASK-002 |\n\
                          | `TASK-006` Asset | `PENDING` | x | TASK-004, TASK-005 |\n\
                          | `TASK-007` Ingest | `PENDING` | x | TASK-003, TASK-006 |\n\
                          | `TASK-010` Package | `PENDING` | x | TASK-002 |\n\
                          | `TASK-011` Host | `PENDING` | x | TASK-003, TASK-010 |";
    assert!(validate_task_dependency_graph(accepted_graph).is_ok());
    let missing_downstream_edge = accepted_graph.replace(
        "| `TASK-006` Asset | `PENDING` | x | TASK-004, TASK-005 |",
        "| `TASK-006` Asset | `PENDING` | x | TASK-005 |",
    );
    assert!(
        validate_task_dependency_graph(&missing_downstream_edge)
            .unwrap_err()
            .contains("accepted downstream edge is missing: TASK-006 -> TASK-004")
    );
    let unknown_dependency = accepted_graph.replace(
        "| `TASK-004` Store | `PENDING` | x | TASK-002 |",
        "| `TASK-004` Store | `PENDING` | x | TASK-999 |",
    );
    assert!(
        validate_task_dependency_graph(&unknown_dependency)
            .unwrap_err()
            .contains("TASK-004 references unknown dependency TASK-999")
    );
    assert!(
        validate_task_002_current_state(
            "| `TASK-002` Core values/error baseline | `IN_PROGRESS` |",
            "implementation_stage: \"Implementation / TASK-002 in progress\"",
            "TASK-002 alone is authorized `IN_PROGRESS`",
            "status: \"TASK_002_IN_PROGRESS\"",
            "TASK-002 start gate is still missing",
            "Status: **ACCEPTED / INCORPORATED IN CANONICAL v1.1.6**",
        )
        .is_err()
    );

    let task_003_developer_ids = [
        "TEST-PROTO-001",
        "TEST-FRAME-001",
        "TEST-HANDSHAKE-001",
        "TEST-ENDPOINT-003",
        "TEST-CONFIG-003",
        "TEST-AUTH-001",
        "TEST-CLI-001",
        "TEST-ARCH-003",
        "TEST-SUPPLY-003",
        "TEST-DOC-003",
    ];
    let valid_task_003_developer_gate = task_003_developer_ids
        .iter()
        .map(|id| format!("task003_run {id} -- cargo test -p fixture --test {id}"))
        .collect::<Vec<_>>()
        .join("\n");
    let valid_task_003_formal_gate = "./scripts/verify-task-003.sh\n\
task003_run TEST-IPC-MACOS-001 -- ./scripts/run-task-003-second-uid.sh";
    validate_task_003_gate_script_mappings(
        &valid_task_003_developer_gate,
        valid_task_003_formal_gate,
    )
    .expect("exact executable TASK-003 mappings must pass");

    let comment_only_task_003_map = task_003_developer_ids
        .iter()
        .map(|id| format!("# task003_run {id} -- cargo test -p fixture --test {id}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        validate_task_003_gate_script_mappings(
            &comment_only_task_003_map,
            valid_task_003_formal_gate,
        )
        .is_err()
    );
    let duplicate_task_003_map = format!(
        "{valid_task_003_developer_gate}\ntask003_run TEST-PROTO-001 -- cargo test -p duplicate"
    );
    assert!(
        validate_task_003_gate_script_mappings(
            &duplicate_task_003_map,
            valid_task_003_formal_gate,
        )
        .is_err()
    );
    let nested_task_003_map = valid_task_003_developer_gate.replacen(
        "task003_run TEST-PROTO-001",
        "  task003_run TEST-PROTO-001",
        1,
    );
    assert!(
        validate_task_003_gate_script_mappings(&nested_task_003_map, valid_task_003_formal_gate,)
            .is_err()
    );
    let missing_argv_task_003_map = valid_task_003_developer_gate.replace(
        "task003_run TEST-PROTO-001 -- cargo test -p fixture --test TEST-PROTO-001",
        "task003_run TEST-PROTO-001 -- ",
    );
    assert!(
        validate_task_003_gate_script_mappings(
            &missing_argv_task_003_map,
            valid_task_003_formal_gate,
        )
        .is_err()
    );
    let shell_eval_task_003_map = valid_task_003_developer_gate.replace(
        "cargo test -p fixture --test TEST-PROTO-001",
        "/bin/sh -c cargo-test",
    );
    assert!(
        validate_task_003_gate_script_mappings(
            &shell_eval_task_003_map,
            valid_task_003_formal_gate,
        )
        .is_err()
    );
    let no_op_task_003_map = valid_task_003_developer_gate
        .replace("cargo test -p fixture --test TEST-PROTO-001", "true");
    assert!(
        validate_task_003_gate_script_mappings(&no_op_task_003_map, valid_task_003_formal_gate,)
            .is_err()
    );
    let formal_without_aggregate = valid_task_003_formal_gate.replace(
        "./scripts/verify-task-003.sh\n",
        "# ./scripts/verify-task-003.sh\n",
    );
    assert!(
        validate_task_003_gate_script_mappings(
            &valid_task_003_developer_gate,
            &formal_without_aggregate,
        )
        .is_err()
    );

    let invalid_task_004_contract = "# TASK-004 accepted implementation contract\n\
        > Status: **ACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION v1.1.13**\n\
        ## 11. Canonical start-record inputs\n\
        ```text\n\
        TASK: TASK-004\n\
        STATUS: IN_PROGRESS\n\
        REQUIREMENTS: DATA-001/005\n\
        ACCEPTANCE: AC-065..AC-071\n\
        ```";
    assert!(
        validate_task_004_active_contract(
            "| `TASK-004` SQLite/migration engine | `IN_PROGRESS` |",
            invalid_task_004_contract,
            &BTreeMap::new(),
        )
        .is_err()
    );

    let root = workspace_root();
    let specification = fs::read_to_string(root.join("docs/spec/IMPLEMENTATION_SPEC.md"))
        .expect("implementation specification is readable");
    let plan = fs::read_to_string(root.join("docs/spec/IMPLEMENTATION_PLAN.md"))
        .expect("implementation plan is readable");
    let decisions = fs::read_to_string(root.join("docs/spec/DECISIONS.md"))
        .expect("decisions document is readable");
    let review = fs::read_to_string(root.join("docs/spec/IMPLEMENTATION_REVIEW.md"))
        .expect("implementation review is readable");
    let intake = fs::read_to_string(root.join("docs/spec/PROJECT_INTAKE_REPORT.md"))
        .expect("project intake is readable");
    let agents = fs::read_to_string(root.join("AGENTS.md")).expect("AGENTS.md is readable");
    let stale_task_007 =
        specification.replace("Acceptance: AC-001..AC-009;", "Acceptance: AC-001..AC-006;");
    assert!(validate_future_task_acceptance_alignment(&stale_task_007, &plan).is_err());
    let stale_task_012 =
        specification.replace("Acceptance: AC-020..AC-023;", "Acceptance: AC-020..AC-027;");
    assert!(validate_future_task_acceptance_alignment(&stale_task_012, &plan).is_err());
    let stale_task_012_tests = specification.replace(
        "Tests: AC-020..AC-023 and per-OS attacks; AC-024..AC-026 remain owned by their later Broker/Lease/Secret tasks and AC-027 by TASK-010.",
        "Tests: AC-020..AC-027 and per-OS attacks.",
    );
    assert!(validate_future_task_acceptance_alignment(&stale_task_012_tests, &plan).is_err());

    let stale_task_005_current_state = specification.replace(
        "reviewed `macos-26` formal CI runs `33073580258` and `33257331689`",
        "reviewed `macos-26` formal CI run `33073580258` only; TASK-006 awaits CI",
    );
    assert!(
        validate_post_task_005_document_consistency(
            &stale_task_005_current_state,
            &plan,
            &decisions,
            &review,
            &intake,
        )
        .is_err()
    );
    let stale_task_013_dependency = specification.replace(
        "Dependencies: TASK-007, TASK-009, TASK-012; OQ-010 accepted for grant/revocation Admin operations.",
        "Dependencies: TASK-009, TASK-012; OQ-010 accepted for grant/revocation Admin operations.",
    );
    assert!(
        validate_post_task_005_document_consistency(
            &stale_task_013_dependency,
            &plan,
            &decisions,
            &review,
            &intake,
        )
        .is_err()
    );
    let stale_task_010_admin_gate = specification.replace(
        "Dependencies: TASK-001, TASK-002; OQ-010 accepted before install, approve, activate or revoke privileged flows.",
        "Dependencies: TASK-001, TASK-002.",
    );
    assert!(
        validate_post_task_005_document_consistency(
            &stale_task_010_admin_gate,
            &plan,
            &decisions,
            &review,
            &intake,
        )
        .is_err()
    );
    let circular_oq_005_plan = plan.replace(
        "TASK-016; closes OQ-005 through its accepted Provider-selection ADRs",
        "TASK-016; OQ-005",
    );
    assert!(
        validate_post_task_005_document_consistency(
            &specification,
            &circular_oq_005_plan,
            &decisions,
            &review,
            &intake,
        )
        .is_err()
    );
    let incomplete_task_010_plan = plan.replace(
        "OQ-010 before install/approve/activate/revoke",
        "OQ-010 for install/approve",
    );
    assert!(
        validate_post_task_005_document_consistency(
            &specification,
            &incomplete_task_010_plan,
            &decisions,
            &review,
            &intake,
        )
        .is_err()
    );
    let shifted_task_005_ac_mapping = plan.replace(
        "`AC-075`: `PASS` — Blob root, fixed Library binding, internal directories/files",
        "`AC-075`: `PASS` — only stable owner-only regular source handles",
    );
    assert!(
        validate_post_task_005_document_consistency(
            &specification,
            &shifted_task_005_ac_mapping,
            &decisions,
            &review,
            &intake,
        )
        .is_err()
    );
    let stale_task_004_intake = intake.replace(
        "`FACT / VERIFIED` | TASK-004 `DONE`",
        "`FACT / ACTIVE TASK-004 SLICES` | TASK-004 formal pending",
    );
    assert!(
        validate_post_task_005_document_consistency(
            &specification,
            &plan,
            &decisions,
            &review,
            &stale_task_004_intake,
        )
        .is_err()
    );
    let stale_review_disposition = review.replace(
        "TASK-001, TASK-002, TASK-004, TASK-003, TASK-005 and TASK-006 are complete",
        "TASK-001, TASK-002, TASK-004, TASK-003 and TASK-005 are complete",
    );
    assert!(
        validate_post_task_005_document_consistency(
            &specification,
            &plan,
            &decisions,
            &stale_review_disposition,
            &intake,
        )
        .is_err()
    );

    let task_003_proposal =
        fs::read_to_string(root.join("docs/proposals/TASK-003-GATE-PROPOSAL.md"))
            .expect("TASK-003 gate proposal is readable");
    let prematurely_active_task_003 = plan.replace(
        "### TASK-003 start record — 2026-08-25",
        "### removed TASK-003 start record",
    );
    assert!(
        validate_task_003_gate_state(
            &prematurely_active_task_003,
            &task_003_proposal,
            &specification,
            &decisions,
            &review,
            &intake,
            &agents,
        )
        .is_err()
    );

    let phrase_and_empty_heading_bypass = task_003_proposal.replace(
        "> Status: **ACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION v1.1.17**",
        "> Status: **REVIEWED**\n\nACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION",
    );
    let active_with_empty_start = format!(
        "{prematurely_active_task_003}\n\n### TASK-003 start record — invalid empty fixture\n"
    );
    assert!(
        validate_task_003_gate_state(
            &active_with_empty_start,
            &phrase_and_empty_heading_bypass,
            &specification,
            &decisions,
            &review,
            &intake,
            &agents,
        )
        .is_err()
    );

    let accepted_task_003_proposal = task_003_proposal.clone();
    let synchronized_specification = specification.clone();
    let synchronized_decisions = decisions.clone();
    let synchronized_review = review.clone();
    let synchronized_intake = intake.clone();
    let synchronized_agents = agents.clone();
    let valid_active_plan = plan.clone();
    validate_task_003_gate_state(
        &valid_active_plan,
        &accepted_task_003_proposal,
        &synchronized_specification,
        &synchronized_decisions,
        &synchronized_review,
        &synchronized_intake,
        &synchronized_agents,
    )
    .expect("an exact accepted TASK-003 gate and complete start record must activate");

    let task_005_proposal =
        fs::read_to_string(root.join("docs/proposals/TASK-005-GATE-PROPOSAL.md"))
            .expect("TASK-005 proposal is readable");
    let task_005_adr =
        fs::read_to_string(root.join("docs/spec/adr/ADR-0007-local-cas-custody-boundary.md"))
            .expect("ADR-0007 is readable");
    validate_task_005_prestart_gate(
        &plan,
        &task_005_proposal,
        &specification,
        &decisions,
        &review,
        &intake,
        &agents,
        &task_005_adr,
    )
    .expect("exact accepted TASK-005 pre-start state must pass");
    let pending_record = "TASK005_CANONICAL_GATE: ACCEPTED\n\
TASK005_SPECIFICATION_VERSION: 1.1.18\n\
TASK005_LIFECYCLE: PENDING_READY_FOR_START\n\
TASK005_IMPLEMENTATION_AUTHORITY: NONE\n\
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md";
    let active_record = "TASK005_CANONICAL_GATE: ACCEPTED\n\
TASK005_SPECIFICATION_VERSION: 1.1.18\n\
TASK005_LIFECYCLE: IN_PROGRESS\n\
TASK005_IMPLEMENTATION_AUTHORITY: TASK_005_ONLY\n\
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md";
    let done_record = "TASK005_CANONICAL_GATE: ACCEPTED\n\
TASK005_SPECIFICATION_VERSION: 1.1.18\n\
TASK005_LIFECYCLE: DONE\n\
TASK005_IMPLEMENTATION_AUTHORITY: NONE\n\
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md";
    let active_plan_source = if plan.contains("| `TASK-005` BlobStorage/CAS primitives | `DONE` |")
    {
        let completion_offset = plan
            .find("\n### TASK-005 completion record")
            .expect("DONE TASK-005 plan contains a completion record");
        let phase_offset = plan[completion_offset..]
            .find("\n## 6. Phases and gates")
            .map(|offset| completion_offset + offset)
            .expect("TASK-005 completion record ends before the phase section");
        format!("{}{}", &plan[..completion_offset], &plan[phase_offset..])
            .replacen(
                "| `TASK-005` BlobStorage/CAS primitives | `DONE` |",
                "| `TASK-005` BlobStorage/CAS primitives | `IN_PROGRESS` |",
                1,
            )
            .replace(done_record, active_record)
    } else {
        plan.clone()
    };
    let active_proposal_source = task_005_proposal.replace(
        "- Status: **ACCEPTED / COMPLETED TASK-005**",
        "- Status: **ACCEPTED / ACTIVE TASK-005**",
    );
    let active_specification_source = specification.replace(done_record, active_record);
    let active_decisions_source = decisions.replace(done_record, active_record);
    let active_review_source = review.replace(done_record, active_record);
    let active_intake_source = intake.replace(done_record, active_record);
    let active_agents_source = agents.replace(done_record, active_record);
    let start_body = task_005_proposal
        .split("```text\n### TASK-005 start record")
        .nth(1)
        .and_then(|tail| tail.split("```").next())
        .expect("TASK-005 proposal contains copy-ready start body");
    let start_offset = active_plan_source
        .find("\n### TASK-005 start record — 2026-08-26")
        .expect("active TASK-005 plan contains its exact start record");
    let phase_offset = active_plan_source[start_offset..]
        .find("\n## 6. Phases and gates")
        .map(|offset| start_offset + offset)
        .expect("TASK-005 start record ends before the phase section");
    let pending_plan = format!(
        "{}{}",
        &active_plan_source[..start_offset],
        &active_plan_source[phase_offset..]
    )
    .replacen(
        "| `TASK-005` BlobStorage/CAS primitives | `IN_PROGRESS` |",
        "| `TASK-005` BlobStorage/CAS primitives | `PENDING / READY FOR START` |",
        1,
    )
    .replace(active_record, pending_record);
    let pending_proposal = active_proposal_source.replace(
        "- Status: **ACCEPTED / ACTIVE TASK-005**",
        "- Status: **ACCEPTED / READY FOR EXPLICIT START ACTIVATION**",
    );
    let pending_specification = active_specification_source.replace(active_record, pending_record);
    let pending_decisions = active_decisions_source.replace(active_record, pending_record);
    let pending_review = active_review_source.replace(active_record, pending_record);
    let pending_intake = active_intake_source.replace(active_record, pending_record);
    let pending_agents = active_agents_source.replace(active_record, pending_record);
    validate_task_005_prestart_gate(
        &pending_plan,
        &pending_proposal,
        &pending_specification,
        &pending_decisions,
        &pending_review,
        &pending_intake,
        &pending_agents,
        &task_005_adr,
    )
    .expect("simulated accepted TASK-005 pending state must pass");
    let active_plan = format!(
        "{}\n\n### TASK-005 start record{}",
        pending_plan
            .replacen(
                "| `TASK-005` BlobStorage/CAS primitives | `PENDING / READY FOR START` |",
                "| `TASK-005` BlobStorage/CAS primitives | `IN_PROGRESS` |",
                1,
            )
            .replace(pending_record, active_record),
        start_body
    );
    let active_proposal = pending_proposal.replace(
        "- Status: **ACCEPTED / READY FOR EXPLICIT START ACTIVATION**",
        "- Status: **ACCEPTED / ACTIVE TASK-005**",
    );
    validate_task_005_prestart_gate(
        &active_plan,
        &active_proposal,
        &pending_specification.replace(pending_record, active_record),
        &pending_decisions.replace(pending_record, active_record),
        &pending_review.replace(pending_record, active_record),
        &pending_intake.replace(pending_record, active_record),
        &pending_agents.replace(pending_record, active_record),
        &task_005_adr,
    )
    .expect("copy-ready TASK-005 activation state must pass before production edits");
    let prematurely_started_task_005 = format!(
        "{pending_plan}\n\n### TASK-005 start record — invalid premature fixture\nSTATUS: IN_PROGRESS\n"
    );
    assert!(
        validate_task_005_prestart_gate(
            &prematurely_started_task_005,
            &pending_proposal,
            &pending_specification,
            &pending_decisions,
            &pending_review,
            &pending_intake,
            &pending_agents,
            &task_005_adr,
        )
        .is_err()
    );
    let stale_task_005_review = pending_review.replace(
        "TASK005_LIFECYCLE: PENDING_READY_FOR_START",
        "TASK005_LIFECYCLE: IN_PROGRESS",
    );
    assert!(
        validate_task_005_prestart_gate(
            &pending_plan,
            &pending_proposal,
            &pending_specification,
            &pending_decisions,
            &stale_task_005_review,
            &pending_intake,
            &pending_agents,
            &task_005_adr,
        )
        .is_err()
    );

    let missing_cross_task_owner = synchronized_specification.replace(
        "TASK003_AC_029_TERMINAL_OWNER: TASK-023",
        "TASK003_AC_029_TERMINAL_OWNER: UNKNOWN",
    );
    assert!(
        validate_task_003_gate_state(
            &valid_active_plan,
            &accepted_task_003_proposal,
            &missing_cross_task_owner,
            &synchronized_decisions,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let missing_error_taxonomy = synchronized_decisions.replace(
        "TASK003_ERROR_CODES_ADDED: IPC_TRANSPORT_ERROR; PROTOCOL_VERSION_UNSUPPORTED; DEADLINE_EXCEEDED",
        "TASK003_ERROR_CODES_ADDED: DEADLINE_EXCEEDED",
    );
    assert!(
        validate_task_003_gate_state(
            &valid_active_plan,
            &accepted_task_003_proposal,
            &synchronized_specification,
            &missing_error_taxonomy,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let missing_decisions_gate = synchronized_decisions.replace(
        "TASK003_CANONICAL_GATE: ACCEPTED",
        "TASK003_CANONICAL_GATE: REMOVED",
    );
    assert!(
        validate_task_003_gate_state(
            &valid_active_plan,
            &accepted_task_003_proposal,
            &synchronized_specification,
            &missing_decisions_gate,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let widened_authority = valid_active_plan.replace(
        "AUTHORIZED: exact §4 scope only",
        "AUTHORIZED: exact §4 scope only except Admin",
    );
    let stale_review =
        synchronized_review.replace("TASK003_LIFECYCLE: DONE", "TASK003_LIFECYCLE: PENDING");
    assert!(
        validate_task_003_gate_state(
            &widened_authority,
            &accepted_task_003_proposal,
            &synchronized_specification,
            &synchronized_decisions,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    assert!(
        validate_task_003_gate_state(
            &valid_active_plan,
            &accepted_task_003_proposal,
            &synchronized_specification,
            &synchronized_decisions,
            &stale_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let duplicate_blocker_heading = format!(
        "{accepted_task_003_proposal}\n\n### `TASK003-BLOCKER-001` — duplicate fixture\n\n- Status: **RESOLVED**\n"
    );
    assert!(
        validate_task_003_gate_state(
            &valid_active_plan,
            &duplicate_blocker_heading,
            &synchronized_specification,
            &synchronized_decisions,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let done_plan = valid_active_plan.clone();
    let done_specification = synchronized_specification.clone();
    let done_decisions = synchronized_decisions.clone();
    let done_review = synchronized_review.clone();
    let done_intake = synchronized_intake.clone();
    let done_agents = synchronized_agents.clone();
    validate_task_003_gate_state(
        &done_plan,
        &accepted_task_003_proposal,
        &done_specification,
        &done_decisions,
        &done_review,
        &done_intake,
        &done_agents,
    )
    .expect("exact TASK-003 completion evidence must permit DONE");

    let negated_pass = done_plan.replace(
        "`AC-060`: `PASS`; EVIDENCE: TEST-FRAME-001",
        "not `AC-060`: `PASS`; EVIDENCE: TEST-FRAME-001",
    );
    assert!(
        validate_task_003_gate_state(
            &negated_pass,
            &accepted_task_003_proposal,
            &done_specification,
            &done_decisions,
            &done_review,
            &done_intake,
            &done_agents,
        )
        .is_err()
    );
    let duplicate_pass = done_plan.replacen(
        "`AC-060`: `PASS`; EVIDENCE: TEST-FRAME-001",
        "`AC-060`: `PASS`; EVIDENCE: TEST-FRAME-001\n`AC-060`: `PASS`; EVIDENCE: TEST-FRAME-001",
        1,
    );
    assert!(
        validate_task_003_gate_state(
            &duplicate_pass,
            &accepted_task_003_proposal,
            &done_specification,
            &done_decisions,
            &done_review,
            &done_intake,
            &done_agents,
        )
        .is_err()
    );

    let arbitrary_evidence = done_plan.replace(
        "`AC-060`: `PASS`; EVIDENCE: TEST-FRAME-001",
        "`AC-060`: `PASS`; EVIDENCE: pass",
    );
    assert!(
        validate_task_003_gate_state(
            &arbitrary_evidence,
            &accepted_task_003_proposal,
            &done_specification,
            &done_decisions,
            &done_review,
            &done_intake,
            &done_agents,
        )
        .is_err()
    );

    let wrong_evidence_fragment = done_plan.replace(
        "scripts/verify-task-003.sh#TEST-PROTO-001",
        "scripts/verify-task-003.sh#TEST-FRAME-001",
    );
    assert!(
        validate_task_003_gate_state(
            &wrong_evidence_fragment,
            &accepted_task_003_proposal,
            &done_specification,
            &done_decisions,
            &done_review,
            &done_intake,
            &done_agents,
        )
        .is_err()
    );

    let wrong_ci_provenance = done_plan.replace(
        "FORMAL_SECOND_UID_CI_REPOSITORY: XiaTian-X/MengXia",
        "FORMAL_SECOND_UID_CI_REPOSITORY: another/repository",
    );
    assert!(
        validate_task_003_gate_state(
            &wrong_ci_provenance,
            &accepted_task_003_proposal,
            &done_specification,
            &done_decisions,
            &done_review,
            &done_intake,
            &done_agents,
        )
        .is_err()
    );

    let malformed_ci_commit = done_plan.replace(
        "FORMAL_SECOND_UID_CI_COMMIT: 4f7bf27855b05c5080790aae3221ee10ae662431",
        "FORMAL_SECOND_UID_CI_COMMIT: not-a-commit",
    );
    assert!(
        validate_task_003_gate_state(
            &malformed_ci_commit,
            &accepted_task_003_proposal,
            &done_specification,
            &done_decisions,
            &done_review,
            &done_intake,
            &done_agents,
        )
        .is_err()
    );

    let active_without_rel_001 = valid_active_plan.replace("REL-001; REL-006", "REL-006");
    assert!(
        validate_task_003_gate_state(
            &active_without_rel_001,
            &accepted_task_003_proposal,
            &synchronized_specification,
            &synchronized_decisions,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let active_with_duplicate_rel_001 =
        valid_active_plan.replace("REL-001; REL-006", "REL-001; REL-001; REL-006");
    assert!(
        validate_task_003_gate_state(
            &active_with_duplicate_rel_001,
            &accepted_task_003_proposal,
            &synchronized_specification,
            &synchronized_decisions,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let accepted_with_noncanonical_blocker_status =
        accepted_task_003_proposal.replacen("- Status: **RESOLVED**", "- Status: **CLOSED**", 1);
    assert!(
        validate_task_003_gate_state(
            &valid_active_plan,
            &accepted_with_noncanonical_blocker_status,
            &synchronized_specification,
            &synchronized_decisions,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let wrong_spec_version =
        accepted_task_003_proposal.replace("SPECIFICATION v1.1.17", "SPECIFICATION v9.9.9");
    assert!(
        validate_task_003_gate_state(
            &valid_active_plan,
            &wrong_spec_version,
            &synchronized_specification,
            &synchronized_decisions,
            &synchronized_review,
            &synchronized_intake,
            &synchronized_agents,
        )
        .is_err()
    );

    let duplicate_draft_status = format!(
        "{task_003_proposal}\n> Status: **DRAFT / BLOCKED — REVIEW REQUIRED; NO IMPLEMENTATION AUTHORITY**\n"
    );
    assert!(
        validate_task_003_gate_state(
            &plan,
            &duplicate_draft_status,
            &specification,
            &decisions,
            &review,
            &intake,
            &agents,
        )
        .is_err()
    );

    let missing_task_003_decode_depth_contract = task_003_proposal.replace(
        "| `MENGXIA_MAX_DECODE_DEPTH` | 64 |",
        "| `REMOVED_DECODE_DEPTH_CONTRACT` | 64 |",
    );
    assert!(
        validate_task_003_gate_state(
            &plan,
            &missing_task_003_decode_depth_contract,
            &specification,
            &decisions,
            &review,
            &intake,
            &agents,
        )
        .is_err()
    );

    let unsafe_partial_staging_cleanup = task_003_proposal.replace(
        "A zero/partial staging file is never deleted, truncated,\n  overwritten or recreated automatically",
        "A zero/partial staging file may be deleted and recreated automatically",
    );
    assert!(
        validate_task_003_gate_state(
            &plan,
            &unsafe_partial_staging_cleanup,
            &specification,
            &decisions,
            &review,
            &intake,
            &agents,
        )
        .is_err()
    );

    let missing_task_003_cli_contract = task_003_proposal.replace(
        "### 8.3 Exact TASK-003 CLI/daemon contract",
        "### 8.3 Removed TASK-003 CLI/daemon contract",
    );
    assert!(
        validate_task_003_gate_state(
            &plan,
            &missing_task_003_cli_contract,
            &specification,
            &decisions,
            &review,
            &intake,
            &agents,
        )
        .is_err()
    );

    let task_004_proposal =
        fs::read_to_string(root.join("docs/proposals/TASK-004-GATE-PROPOSAL.md"))
            .expect("TASK-004 accepted contract is readable");
    let missing_ci_scope = task_004_proposal.replace(
        concat!(
            "- `.github/workflows/ci.yml`, limited to selecting the arm64 `macos-26` runner,\n",
            "  executing §6.1's exact fail-closed platform preflight before Cargo, and adding the\n",
            "  TASK-004 gates while preserving all existing security and TASK-001 gates;\n",
        ),
        "",
    );
    let error = validate_task_004_active_contract(
        "| `TASK-004` SQLite/migration engine | `IN_PROGRESS` | CFG-001; CFG-003; BASE-015 |",
        &missing_ci_scope,
        &BTreeMap::new(),
    )
    .expect_err("TASK-004 contract without the narrowly bounded CI scope must fail");
    assert!(
        error.contains("implementation scope is missing CI boundary"),
        "unexpected validation error: {error}"
    );

    let missing_applications_exception = task_004_proposal.replace(
        "UID `0`, GID `80` (`admin`), mode exactly `0775`",
        "UID `0`, GID `80` (`admin`), mode exactly `0755`",
    );
    let error = validate_task_004_active_contract(
        "| `TASK-004` SQLite/migration engine | `IN_PROGRESS` | CFG-001; CFG-003; BASE-015 |",
        &missing_applications_exception,
        &BTreeMap::new(),
    )
    .expect_err("TASK-004 contract without the exact /Applications exception must fail");
    assert!(
        error.contains("UID `0`, GID `80` (`admin`), mode exactly `0775`"),
        "unexpected validation error: {error}"
    );
}

fn document_text<'a>(documents: &'a [Document], file_name: &str) -> &'a str {
    &documents
        .iter()
        .find(|document| document.path.ends_with(file_name))
        .unwrap_or_else(|| panic!("missing canonical document {file_name}"))
        .text
}

fn validate_task_003_repository_gate_files(root: &Path, plan: &str) -> Result<(), String> {
    let task_row = plan
        .lines()
        .find(|line| line.starts_with("| `TASK-003` IPC, framing, Client identity |"))
        .ok_or_else(|| "TASK-003 plan row is missing".to_owned())?;
    let status = task_row
        .split('|')
        .nth(2)
        .map(str::trim)
        .ok_or_else(|| "TASK-003 plan row has no status".to_owned())?;
    if status != "`DONE`" {
        return Ok(());
    }

    let developer_path = root.join("scripts/verify-task-003.sh");
    let formal_path = root.join("scripts/verify-task-003-formal-second-uid.sh");
    let privileged_runner_path = root.join("scripts/run-task-003-second-uid.sh");
    let developer = fs::read_to_string(&developer_path)
        .map_err(|error| format!("{} is missing: {error}", developer_path.display()))?;
    let formal = fs::read_to_string(&formal_path)
        .map_err(|error| format!("{} is missing: {error}", formal_path.display()))?;
    let privileged_runner = fs::read_to_string(&privileged_runner_path)
        .map_err(|error| format!("{} is missing: {error}", privileged_runner_path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        for path in [&developer_path, &formal_path, &privileged_runner_path] {
            let metadata = fs::symlink_metadata(path)
                .map_err(|error| format!("{} metadata is unreadable: {error}", path.display()))?;
            if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
                return Err(format!(
                    "{} is not a regular non-symlink file",
                    path.display()
                ));
            }
            let mode = metadata.permissions().mode();
            if mode & 0o111 == 0 {
                return Err(format!("{} is not executable", path.display()));
            }
        }
    }

    for required in [
        "mengxia-task003-ci",
        "/usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl",
        "trap task003_cleanup_second_uid EXIT",
        "cargo test -p mengxiad --bin mengxiad --locked --offline task_003_real_second_uid_peer_is_rejected_before_frame -- --exact --ignored --nocapture",
    ] {
        if !privileged_runner.contains(required) {
            return Err(format!(
                "TASK-003 privileged runner lacks required executable contract: {required}"
            ));
        }
    }
    if privileged_runner.contains("TEST-IPC-MACOS-001: PASS") {
        return Err(
            "TASK-003 privileged runner must not emit the formal verification PASS result"
                .to_owned(),
        );
    }

    validate_task_003_gate_script_mappings(&developer, &formal)
}

fn validate_task_003_gate_script_mappings(developer: &str, formal: &str) -> Result<(), String> {
    const DEVELOPER_IDS: &[&str] = &[
        "TEST-PROTO-001",
        "TEST-FRAME-001",
        "TEST-HANDSHAKE-001",
        "TEST-ENDPOINT-003",
        "TEST-CONFIG-003",
        "TEST-AUTH-001",
        "TEST-CLI-001",
        "TEST-ARCH-003",
        "TEST-SUPPLY-003",
        "TEST-DOC-003",
    ];

    fn parse(script_name: &str, script: &str) -> Result<BTreeMap<String, String>, String> {
        let mut mappings = BTreeMap::new();
        for raw_line in script.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.contains("eval ")
                || line.contains(" sh -c ")
                || line.starts_with("sh -c ")
                || line.contains(" bash -c ")
                || line.starts_with("bash -c ")
            {
                return Err(format!(
                    "TASK-003 gate {script_name} contains forbidden shell evaluation"
                ));
            }
            let Some(mapping) = line.strip_prefix("task003_run ") else {
                continue;
            };
            if raw_line != line {
                return Err(format!(
                    "TASK-003 gate {script_name} mapping is not one exact top-level statement: {raw_line}"
                ));
            }
            let (test_id, argv) = mapping.split_once(" -- ").ok_or_else(|| {
                format!("TASK-003 gate {script_name} has a malformed mapping: {line}")
            })?;
            if test_id.is_empty()
                || test_id.bytes().any(|byte| byte.is_ascii_whitespace())
                || argv.is_empty()
                || argv.len() > 512
                || argv.chars().any(char::is_control)
                || argv.starts_with('#')
                || argv.starts_with("sh -c ")
                || argv.starts_with("bash -c ")
                || argv.starts_with("/bin/sh -c ")
                || argv.starts_with("/bin/bash -c ")
                || argv.starts_with("/usr/bin/env sh -c ")
                || argv.starts_with("/usr/bin/env bash -c ")
                || !(argv.starts_with("cargo ") || argv.starts_with("./scripts/"))
            {
                return Err(format!(
                    "TASK-003 gate {script_name} has an unsafe mapping: {line}"
                ));
            }
            if mappings
                .insert(test_id.to_owned(), argv.to_owned())
                .is_some()
            {
                return Err(format!(
                    "TASK-003 gate {script_name} maps {test_id} more than once"
                ));
            }
        }
        Ok(mappings)
    }

    let developer_mappings = parse("developer", developer)?;
    let expected_developer: BTreeSet<_> = DEVELOPER_IDS.iter().map(|id| (*id).to_owned()).collect();
    let actual_developer: BTreeSet<_> = developer_mappings.keys().cloned().collect();
    if actual_developer != expected_developer {
        return Err(format!(
            "TASK-003 developer gate mapping differs: expected {expected_developer:?}, got {actual_developer:?}"
        ));
    }

    let formal_mappings = parse("formal", formal)?;
    if formal_mappings.len() != 1 || !formal_mappings.contains_key("TEST-IPC-MACOS-001") {
        return Err(
            "TASK-003 formal gate must map exactly TEST-IPC-MACOS-001 with non-empty argv"
                .to_owned(),
        );
    }
    if formal_mappings
        .get("TEST-IPC-MACOS-001")
        .map(String::as_str)
        != Some("./scripts/run-task-003-second-uid.sh")
    {
        return Err(
            "TASK-003 formal gate must map TEST-IPC-MACOS-001 to the exact privileged runner"
                .to_owned(),
        );
    }

    let aggregate_positions: Vec<_> = formal
        .lines()
        .enumerate()
        .filter(|(_, line)| *line == "./scripts/verify-task-003.sh")
        .map(|(index, _)| index)
        .collect();
    let formal_mapping_position = formal
        .lines()
        .position(|line| line.starts_with("task003_run TEST-IPC-MACOS-001 -- "))
        .ok_or_else(|| "TASK-003 formal mapping statement is missing".to_owned())?;
    if aggregate_positions.len() != 1 || aggregate_positions[0] >= formal_mapping_position {
        return Err(
            "TASK-003 formal gate must invoke the exact developer aggregate once before its owned mapping"
                .to_owned(),
        );
    }

    Ok(())
}

fn load_documents(spec_directory: &Path) -> Vec<Document> {
    let mut paths = Vec::new();
    collect_markdown_paths(spec_directory, &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| Document {
            text: fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display())),
            path,
        })
        .collect()
}

fn collect_markdown_paths(directory: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
    {
        let path = entry.expect("directory entry is readable").path();
        if path.is_dir() {
            collect_markdown_paths(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            paths.push(path);
        }
    }
}

fn collect_definitions(documents: &[Document]) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut definitions = BTreeMap::new();
    for document in documents {
        for line in document.text.lines() {
            if let Some(id) = canonical_definition(&document.path, line)
                && let Some(previous) = definitions.insert(id.clone(), document.path.clone())
            {
                return Err(format!(
                    "duplicate canonical definition {id}: {} and {}",
                    previous.display(),
                    document.path.display()
                ));
            }
        }
    }
    Ok(definitions)
}

fn collect_definitions_for_test(
    documents: &[Document],
    definition: &str,
) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut definitions = BTreeMap::new();
    for document in documents {
        for line in document.text.lines().filter(|line| *line == definition) {
            if let Some(previous) = definitions.insert(line.to_owned(), document.path.clone()) {
                return Err(format!(
                    "duplicate canonical definition {line}: {} and {}",
                    previous.display(),
                    document.path.display()
                ));
            }
        }
    }
    Ok(definitions)
}

fn canonical_definition(path: &Path, line: &str) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    let trimmed = line.trim();
    let first_id = extract_ids(trimmed).into_iter().next()?;
    let namespace = namespace(&first_id)?;

    if file_name == "IMPLEMENTATION_SPEC.md" {
        let table_definition = trimmed.starts_with(&format!("| `{first_id}` |"));
        let heading_definition = trimmed.starts_with(&format!("### `{first_id}`"));
        let bare_acceptance = namespace == "AC" && trimmed == first_id;
        let table_namespaces = [
            "G", "REQ", "DATA", "API", "SEC", "REL", "PERF", "OPS", "CFG", "TEST", "OQ", "DEC",
            "RISK", "SRC",
        ];
        if (table_definition && table_namespaces.contains(&namespace))
            || (heading_definition && ["TASK", "CONFLICT"].contains(&namespace))
            || bare_acceptance
        {
            return Some(first_id);
        }
    } else if file_name == "IMPLEMENTATION_REVIEW.md"
        && trimmed.starts_with(&format!("| `{first_id}` |"))
        && ["FUNC", "REVIEW"].contains(&namespace)
    {
        return Some(first_id);
    } else if file_name == "DECISIONS.md" {
        if namespace == "BASE" && trimmed.starts_with(&format!("| `{first_id}` |")) {
            return Some(first_id);
        }
        if (namespace == "BASELINE"
            || first_id.starts_with("REVIEW-CONFLICT-")
            || first_id.starts_with("REVIEW-GAP-"))
            && trimmed.starts_with(&format!("### `{first_id}`"))
        {
            return Some(first_id);
        }
    } else if file_name.starts_with("ADR-")
        && namespace == "ADR"
        && trimmed.starts_with(&format!("# {first_id}:"))
    {
        return Some(first_id);
    }
    None
}

fn validate_references(text: &str, definitions: &BTreeMap<String, PathBuf>) -> Result<(), String> {
    let unknown: Vec<_> = extract_ids(text)
        .into_iter()
        .filter(|id| !definitions.contains_key(id))
        .collect();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(format!("unknown references: {unknown:?}"))
    }
}

fn validate_ranges(text: &str, definitions: &BTreeMap<String, PathBuf>) -> Result<(), String> {
    let mut remaining = text;
    while let Some(separator) = remaining.find("..") {
        let left_text = &remaining[..separator];
        let right_text = &remaining[separator + 2..];
        let left_token = left_text
            .split(|character: char| {
                character.is_whitespace() || "`.,;:()[]{}|".contains(character)
            })
            .next_back()
            .unwrap_or_default();
        let right_token = right_text
            .split(|character: char| {
                character.is_whitespace() || "`.,;:()[]{}|".contains(character)
            })
            .next()
            .unwrap_or_default();
        let left = parse_exact_id(left_token);
        let right = parse_exact_id(right_token);
        let looks_like_bare_stable_number = |token: &str| {
            (3..=4).contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_digit())
        };
        if left.is_none()
            && right.is_none()
            && !looks_like_bare_stable_number(left_token)
            && !looks_like_bare_stable_number(right_token)
        {
            remaining = right_text;
            continue;
        }
        let left = left.ok_or_else(|| {
            format!("malformed range left endpoint near {left_token}..{right_token}")
        })?;
        let right = right.ok_or_else(|| {
            format!("malformed range right endpoint near {left_token}..{right_token}")
        })?;
        let (left_namespace, left_number) = split_id(&left)?;
        let (right_namespace, right_number) = split_id(&right)?;
        if left_namespace != right_namespace || left_number > right_number {
            return Err(format!("invalid stable-ID range {left}..{right}"));
        }
        let width = left.rsplit('-').next().expect("ID has number").len();
        for number in left_number..=right_number {
            let expanded = format!("{left_namespace}-{number:0width$}");
            if !definitions.contains_key(&expanded) {
                return Err(format!(
                    "range {left}..{right} expands to unknown {expanded}"
                ));
            }
        }
        remaining = right_text;
    }
    Ok(())
}

fn validate_task_001_record(
    plan: &str,
    definitions: &BTreeMap<String, PathBuf>,
) -> Result<(), String> {
    let heading = "### TASK-001 start record";
    let start = plan
        .find(heading)
        .ok_or_else(|| "missing TASK-001 start record".to_owned())?;
    let record = &plan[start..];
    let end = record
        .find("\n### ")
        .or_else(|| record.find("\n## "))
        .unwrap_or(record.len());
    let record = &record[..end];
    if record.contains("..") {
        return Err("task-start evidence must enumerate IDs, not ranges".to_owned());
    }
    validate_references(record, definitions)?;
    for required in [
        "FUNC-001",
        "SEC-020",
        "DATA-006",
        "AC-050",
        "AC-051",
        "AC-052",
        "AC-053",
        "AC-054",
        "TEST-BOOT-001",
        "TEST-BOOT-002",
        "TEST-ARCH-001",
        "TEST-NAME-001",
        "TEST-SUPPLY-001",
        "TEST-DOC-001",
    ] {
        if !extract_ids(record).iter().any(|id| id == required) {
            return Err(format!("TASK-001 start record is missing {required}"));
        }
    }
    let in_progress = plan.contains("| `TASK-001` Repository bootstrap | `IN_PROGRESS` |");
    let done = plan.contains("| `TASK-001` Repository bootstrap | `DONE` |");
    if !in_progress && !done {
        return Err("TASK-001 is not lifecycle-active as IN_PROGRESS or DONE".to_owned());
    }
    if done {
        let completion = plan
            .split("### TASK-001 completion record")
            .nth(1)
            .ok_or_else(|| "DONE task is missing its completion record".to_owned())?;
        for required in [
            "AC-050",
            "AC-051",
            "AC-052",
            "AC-053",
            "AC-054",
            "TEST-BOOT-001",
            "TEST-BOOT-002",
            "TEST-ARCH-001",
            "TEST-NAME-001",
            "TEST-SUPPLY-001",
            "TEST-DOC-001",
            "SEC-020",
        ] {
            let pass_evidence = format!("`{required}`: `PASS`");
            if !completion.contains(&pass_evidence) {
                return Err(format!("DONE task lacks PASS evidence for {required}"));
            }
        }
    }
    Ok(())
}

fn validate_task_001_dependencies(decisions: &str, adr: &str) -> Result<(), String> {
    let accepted_question = decisions
        .lines()
        .any(|line| line.starts_with("| `OQ-003` |") && line.contains("`ACCEPTED / ADR-0003`"));
    let accepted_adr = adr.starts_with("# ADR-0003:") && adr.contains("- Status: ACCEPTED");
    if accepted_question && accepted_adr {
        Ok(())
    } else {
        Err("OQ-003 and ADR-0003 must both remain accepted".to_owned())
    }
}

fn validate_task_dependency_graph(plan: &str) -> Result<(), String> {
    let mut graph = BTreeMap::<String, BTreeSet<String>>::new();
    for line in plan.lines().filter(|line| line.starts_with("| `TASK-")) {
        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        if cells.len() < 6 {
            return Err(format!("malformed task table row: {line}"));
        }
        let task_ids: Vec<_> = extract_ids(cells[1])
            .into_iter()
            .filter(|id| id.starts_with("TASK-"))
            .collect();
        if task_ids.len() != 1 {
            return Err(format!("task row must define exactly one task: {line}"));
        }
        let dependencies = extract_ids(cells[4])
            .into_iter()
            .filter(|id| id.starts_with("TASK-"))
            .collect();
        if graph.insert(task_ids[0].clone(), dependencies).is_some() {
            return Err(format!("duplicate task row: {}", task_ids[0]));
        }
    }

    for (task, dependencies) in &graph {
        for dependency in dependencies {
            if !graph.contains_key(dependency) {
                return Err(format!("{task} references unknown dependency {dependency}"));
            }
        }
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for task in graph.keys() {
        visit_task_dependency(task, &graph, &mut visiting, &mut visited)?;
    }

    const REQUIRED_EDGES: &[(&str, &[&str])] = &[
        ("TASK-003", &["TASK-004"]),
        ("TASK-004", &["TASK-002"]),
        ("TASK-006", &["TASK-004", "TASK-005"]),
        ("TASK-007", &["TASK-003", "TASK-006"]),
        ("TASK-011", &["TASK-003", "TASK-010"]),
    ];
    for (task, required_dependencies) in REQUIRED_EDGES {
        let dependencies = graph
            .get(*task)
            .ok_or_else(|| format!("missing required task row {task}"))?;
        for dependency in *required_dependencies {
            if !dependencies.contains(*dependency) {
                return Err(format!(
                    "accepted downstream edge is missing: {task} -> {dependency}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_future_task_acceptance_alignment(
    specification: &str,
    plan: &str,
) -> Result<(), String> {
    for (task, next_task, specification_marker, plan_marker) in [
        (
            "TASK-007",
            "TASK-008",
            "Acceptance: AC-001..AC-009;",
            "AC-001..AC-009",
        ),
        ("TASK-010", "TASK-011", "Acceptance: AC-027;", "AC-027"),
        (
            "TASK-012",
            "TASK-013",
            "Acceptance: AC-020..AC-023;",
            "AC-020..AC-023",
        ),
    ] {
        let section = task_section(specification, task, next_task)?;
        if !section.contains(specification_marker) {
            return Err(format!(
                "{task} specification section is missing exact acceptance mapping {specification_marker}"
            ));
        }
        let row = plan
            .lines()
            .find(|line| line.starts_with(&format!("| `{task}` ")))
            .ok_or_else(|| format!("{task} plan row is missing"))?;
        if !row.contains(plan_marker) {
            return Err(format!(
                "{task} plan row is missing exact acceptance mapping {plan_marker}"
            ));
        }
    }

    let task_012 = task_section(specification, "TASK-012", "TASK-013")?;
    let task_012_tests = "Tests: AC-020..AC-023 and per-OS attacks; AC-024..AC-026 remain owned by their later Broker/Lease/Secret tasks and AC-027 by TASK-010.";
    if !task_012.contains(task_012_tests) {
        return Err(
            "TASK-012 must not absorb the later Broker/Lease/Secret or TASK-010 acceptance IDs"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_post_task_005_document_consistency(
    specification: &str,
    plan: &str,
    decisions: &str,
    review: &str,
    intake: &str,
) -> Result<(), String> {
    let current_state = specification
        .split("### 0.4 Current task parameters")
        .nth(1)
        .and_then(|tail| tail.split("### 0.5 Stable verification identifiers").next())
        .ok_or_else(|| "Specification current-task parameter section is missing".to_owned())?;
    for required in [
        "TASK-001/TASK-002/TASK-004/TASK-003/TASK-005/TASK-006 已完成",
        "reviewed `macos-26` formal CI runs `33073580258` and `33257331689`",
        "当前 implementation authority 为 `NONE`",
    ] {
        if !current_state.contains(required) {
            return Err(format!(
                "Specification current state is missing completed TASK-005/TASK-006 evidence: {required}"
            ));
        }
    }
    if current_state.contains("等待 reviewed") {
        return Err(
            "Specification current state still says TASK-005 awaits reviewed CI".to_owned(),
        );
    }

    let conflict_004 = specification
        .split("### `CONFLICT-004`")
        .nth(1)
        .and_then(|tail| tail.split("## 24. Open Questions").next())
        .ok_or_else(|| "Specification CONFLICT-004 section is missing".to_owned())?;
    for required in [
        "reviewed `macos-26` formal CI run `33073580258`",
        "Status: RESOLVED / CONFIRMED BY ADR-0007 / IMPLEMENTED / VERIFIED.",
    ] {
        if !conflict_004.contains(required) {
            return Err(format!(
                "Specification CONFLICT-004 lacks verified completion disposition: {required}"
            ));
        }
    }
    if conflict_004.contains("IMPLEMENTATION EXPECTED_GAP") {
        return Err("Specification CONFLICT-004 retains a closed implementation gap".to_owned());
    }

    let specification_dependencies = specification_task_dependencies(specification)?;
    let plan_dependencies = plan_task_dependencies(plan)?;
    for task_number in 1..=23 {
        let task = format!("TASK-{task_number:03}");
        let specification_set = specification_dependencies
            .get(&task)
            .ok_or_else(|| format!("Specification is missing dependency set for {task}"))?;
        let plan_set = plan_dependencies
            .get(&task)
            .ok_or_else(|| format!("Plan is missing dependency set for {task}"))?;
        if specification_set != plan_set {
            return Err(format!(
                "{task} dependency mismatch: Specification={specification_set:?}, Plan={plan_set:?}"
            ));
        }
    }

    let task_010 = task_section(specification, "TASK-010", "TASK-011")?;
    if !task_010
        .contains("OQ-010 accepted before install, approve, activate or revoke privileged flows")
    {
        return Err("TASK-010 Specification body is missing its scoped OQ-010 gate".to_owned());
    }
    let task_010_plan_row = plan
        .lines()
        .find(|line| line.starts_with("| `TASK-010` Plugin package/Manifest |"))
        .ok_or_else(|| "Plan TASK-010 row is missing".to_owned())?;
    if !task_010_plan_row.contains("OQ-010 before install/approve/activate/revoke") {
        return Err("Plan TASK-010 row understates its scoped OQ-010 gate".to_owned());
    }
    let task_013 = task_section(specification, "TASK-013", "TASK-014")?;
    for required in [
        "Dependencies: TASK-007, TASK-009, TASK-012; OQ-010 accepted",
        "Acceptance: AC-024, AC-026, AC-028;",
        "ordinary-Client privileged-dispatch denial boundary",
    ] {
        if !task_013.contains(required) {
            return Err(format!(
                "TASK-013 Specification body lacks accepted synchronization: {required}"
            ));
        }
    }
    if task_013.contains("AC-029") {
        return Err("TASK-013 must not claim terminal AC-029 ownership".to_owned());
    }

    let task_017_row = plan
        .lines()
        .find(|line| line.starts_with("| `TASK-017` Provider selection gate |"))
        .ok_or_else(|| "Plan TASK-017 row is missing".to_owned())?;
    if !task_017_row.contains("closes OQ-005 through its accepted Provider-selection ADRs")
        || task_017_row.contains("TASK-016; OQ-005")
    {
        return Err("Plan must treat OQ-005 as TASK-017 output, not its prerequisite".to_owned());
    }
    let oq_005_row = decisions
        .lines()
        .find(|line| line.starts_with("| `OQ-005` |"))
        .ok_or_else(|| "Decisions OQ-005 row is missing".to_owned())?;
    if !oq_005_row.contains("TASK-018..TASK-020")
        || !oq_005_row.contains("由 TASK-017 的 accepted Provider-selection ADR 关闭")
    {
        return Err("Decisions must assign OQ-005 closure to TASK-017 before adapters".to_owned());
    }

    let task_005_completion = plan
        .split("### TASK-005 completion record — 2026-08-27")
        .nth(1)
        .and_then(|tail| tail.split("## 6. Phases and gates").next())
        .ok_or_else(|| "Plan TASK-005 completion record is missing".to_owned())?;
    for required in [
        "`AC-074`: `PASS` — source-free typed configuration",
        "`AC-075`: `PASS` — Blob root, fixed Library binding, internal directories/files",
        "`AC-076`: `PASS` — the retained regular-file handle, before/after identity",
        "`AC-077`: `PASS` — SHA-256 and exact length use one O(buffer) stream",
        "`AC-078`: `PASS` — one atomic logical/physical admission",
        "`AC-079`: `PASS` — exact-case no-replace promotion, rehash, sync ordering",
        "`AC-080`: `PASS` — every named crash/fault prefix",
        "`AC-081`: `PASS` — joined workers/channels",
    ] {
        if !task_005_completion.contains(required) {
            return Err(format!(
                "Plan TASK-005 completion evidence is not aligned to its canonical AC: {required}"
            ));
        }
    }

    let review_019 = review
        .lines()
        .find(|line| line.starts_with("| `REVIEW-019` |"))
        .ok_or_else(|| "Review REVIEW-019 disposition row is missing".to_owned())?;
    if !review_019
        .contains("TASK-001, TASK-002, TASK-004, TASK-003, TASK-005 and TASK-006 are complete")
    {
        return Err("Review current disposition omits completed TASK-005/TASK-006".to_owned());
    }

    let task_004_intake = intake
        .lines()
        .find(|line| line.starts_with("| TASK-001/TASK-002 已完成；workspace"))
        .ok_or_else(|| "Intake TASK-004 repository-fact row is missing".to_owned())?;
    if !task_004_intake.contains("reviewed runner-XIP formal CI run `32695815747`")
        || !task_004_intake.contains("`FACT / VERIFIED`")
        || !task_004_intake.contains("TASK-004 `DONE`")
        || task_004_intake.contains("ACTIVE TASK-004")
    {
        return Err("Intake retains stale TASK-004 active/formal-pending state".to_owned());
    }
    Ok(())
}

fn specification_task_dependencies(
    specification: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut dependencies = BTreeMap::new();
    for task_number in 1..=23 {
        let task = format!("TASK-{task_number:03}");
        let heading = format!("### `{task}`");
        let start = specification
            .find(&heading)
            .ok_or_else(|| format!("Specification task heading is missing: {task}"))?;
        let remainder = &specification[start + heading.len()..];
        let end = remainder
            .find("\n### `TASK-")
            .or_else(|| remainder.find("\n## 19. Acceptance Criteria"))
            .unwrap_or(remainder.len());
        let section = &remainder[..end];
        let dependency_lines: Vec<_> = section
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("Dependencies:"))
            .collect();
        if dependency_lines.len() != 1 {
            return Err(format!(
                "Specification {task} must have exactly one Dependencies line"
            ));
        }
        let mut task_dependencies: BTreeSet<_> = extract_ids(dependency_lines[0])
            .into_iter()
            .filter(|id| id.starts_with("TASK-"))
            .collect();
        task_dependencies.remove(&task);
        dependencies.insert(task, task_dependencies);
    }
    Ok(dependencies)
}

fn plan_task_dependencies(plan: &str) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut dependencies = BTreeMap::new();
    for line in plan.lines().filter(|line| line.starts_with("| `TASK-")) {
        let cells: Vec<_> = line.split('|').map(str::trim).collect();
        if cells.len() < 6 {
            return Err(format!("malformed Plan task row: {line}"));
        }
        let task_ids: Vec<_> = extract_ids(cells[1])
            .into_iter()
            .filter(|id| id.starts_with("TASK-"))
            .collect();
        if task_ids.len() != 1 {
            return Err(format!(
                "Plan task row must define exactly one task: {line}"
            ));
        }
        let mut task_dependencies: BTreeSet<_> = extract_ids(cells[4])
            .into_iter()
            .filter(|id| id.starts_with("TASK-"))
            .collect();
        task_dependencies.remove(&task_ids[0]);
        dependencies.insert(task_ids[0].clone(), task_dependencies);
    }
    Ok(dependencies)
}

fn task_section<'a>(
    specification: &'a str,
    task: &str,
    next_task: &str,
) -> Result<&'a str, String> {
    let heading = format!("### `{task}`");
    let start = specification
        .find(&heading)
        .ok_or_else(|| format!("{task} specification section is missing"))?;
    let remainder = &specification[start..];
    let next_heading = format!("### `{next_task}`");
    let end = remainder
        .find(&next_heading)
        .ok_or_else(|| format!("{task} specification section has no {next_task} boundary"))?;
    Ok(&remainder[..end])
}

fn visit_task_dependency(
    task: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<(), String> {
    if visited.contains(task) {
        return Ok(());
    }
    if !visiting.insert(task.to_owned()) {
        return Err(format!("task dependency cycle contains {task}"));
    }
    for dependency in graph
        .get(task)
        .ok_or_else(|| format!("missing task node {task}"))?
    {
        visit_task_dependency(dependency, graph, visiting, visited)?;
    }
    visiting.remove(task);
    visited.insert(task.to_owned());
    Ok(())
}

fn validate_task_002_record(
    plan: &str,
    definitions: &BTreeMap<String, PathBuf>,
) -> Result<(), String> {
    let heading = "### TASK-002 start record";
    let start = plan
        .find(heading)
        .ok_or_else(|| "missing TASK-002 start record".to_owned())?;
    let record = &plan[start..];
    let end = record
        .find("\n### ")
        .or_else(|| record.find("\n## "))
        .unwrap_or(record.len());
    let record = &record[..end];
    if record.contains("..") {
        return Err("TASK-002 start evidence must enumerate IDs, not ranges".to_owned());
    }
    validate_references(record, definitions)?;
    for required in [
        "FUNC-001",
        "REQ-001",
        "API-010",
        "DATA-012",
        "SEC-017",
        "SEC-020",
        "AC-055",
        "AC-056",
        "AC-057",
        "AC-058",
        "AC-059",
        "TEST-TYPE-001",
        "TEST-PARSE-001",
        "TEST-TIME-001",
        "TEST-ERROR-001",
        "TEST-ARCH-002",
        "TEST-SUPPLY-002",
        "TEST-DOC-002",
    ] {
        if !extract_ids(record).iter().any(|id| id == required) {
            return Err(format!("TASK-002 start record is missing {required}"));
        }
    }

    let in_progress = plan.contains("| `TASK-002` Core values/error baseline | `IN_PROGRESS` |");
    let done = plan.contains("| `TASK-002` Core values/error baseline | `DONE` |");
    if !in_progress && !done {
        return Err("TASK-002 is not lifecycle-active as IN_PROGRESS or DONE".to_owned());
    }
    if done {
        let completion = plan
            .split("### TASK-002 completion record")
            .nth(1)
            .ok_or_else(|| "DONE TASK-002 is missing its completion record".to_owned())?;
        for required in [
            "AC-055",
            "AC-056",
            "AC-057",
            "AC-058",
            "AC-059",
            "TEST-TYPE-001",
            "TEST-PARSE-001",
            "TEST-TIME-001",
            "TEST-ERROR-001",
            "TEST-ARCH-002",
            "TEST-SUPPLY-002",
            "TEST-DOC-002",
            "SEC-017",
            "SEC-020",
        ] {
            if !completion.contains(&format!("`{required}`: `PASS`")) {
                return Err(format!("DONE TASK-002 lacks PASS evidence for {required}"));
            }
        }
    }
    Ok(())
}

fn validate_task_002_current_state(
    plan: &str,
    specification: &str,
    review: &str,
    intake: &str,
    agents: &str,
    proposal: &str,
) -> Result<(), String> {
    let in_progress = plan.contains("| `TASK-002` Core values/error baseline | `IN_PROGRESS` |");
    let done = plan.contains("| `TASK-002` Core values/error baseline | `DONE` |");
    let required = if in_progress {
        [
            (
                specification,
                "implementation_stage: \"Implementation / TASK-002 in progress\"",
            ),
            (review, "TASK-002 alone is authorized"),
            (intake, "status: \"TASK_002_IN_PROGRESS\""),
            (agents, "当前授权范围：仅 TASK-002"),
            (
                proposal,
                "Status: **ACCEPTED / INCORPORATED IN CANONICAL v1.1.6**",
            ),
        ]
    } else if done {
        [
            (specification, "TASK003_LIFECYCLE: DONE"),
            (review, "`TASK-002 DONE`"),
            (intake, "TASK-001/TASK-002 已完成"),
            (agents, "TASK-001/TASK-002/TASK-004/TASK-003 已完成"),
            (
                proposal,
                "Status: **ACCEPTED / INCORPORATED IN CANONICAL v1.1.6**",
            ),
        ]
    } else {
        return Err("TASK-002 plan state is neither IN_PROGRESS nor DONE".to_owned());
    };

    for (document, marker) in required {
        if !document.contains(marker) {
            return Err(format!(
                "TASK-002 current-state marker is missing: {marker}"
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_task_005_prestart_gate(
    plan: &str,
    proposal: &str,
    specification: &str,
    decisions: &str,
    review: &str,
    intake: &str,
    agents: &str,
    adr: &str,
) -> Result<(), String> {
    const READY_STATUS: &str = "- Status: **ACCEPTED / READY FOR EXPLICIT START ACTIVATION**";
    const ACTIVE_STATUS: &str = "- Status: **ACCEPTED / ACTIVE TASK-005**";
    const DONE_STATUS: &str = "- Status: **ACCEPTED / COMPLETED TASK-005**";
    const ACCEPTANCE: &[&str] = &[
        "AC-074", "AC-075", "AC-076", "AC-077", "AC-078", "AC-079", "AC-080", "AC-081",
    ];
    const TESTS: &[&str] = &[
        "TEST-CONFIG-005",
        "TEST-NAMESPACE-005",
        "TEST-PATH-005",
        "TEST-SOURCE-005",
        "TEST-STREAM-005",
        "TEST-CONTROL-005",
        "TEST-RESOURCE-005",
        "TEST-PROMOTE-005",
        "TEST-LOCATION-005",
        "TEST-RECOVERY-005",
        "TEST-ORPHAN-005",
        "TEST-CONCURRENCY-005",
        "TEST-ERROR-005",
        "TEST-LIFECYCLE-005",
        "TEST-ARCH-005",
        "TEST-SUPPLY-005",
        "TEST-DOC-005",
    ];

    let proposal_statuses: Vec<_> = proposal
        .lines()
        .filter(|line| line.starts_with("- Status: **ACCEPTED /"))
        .collect();
    if proposal_statuses.len() != 1 || proposal.contains("DRAFT / BLOCKED") {
        return Err("TASK-005 proposal lacks one exact accepted status".to_owned());
    }
    let rows: Vec<_> = plan
        .lines()
        .filter(|line| line.starts_with("| `TASK-005` BlobStorage/CAS primitives |"))
        .collect();
    if rows.len() != 1 {
        return Err("TASK-005 plan must contain exactly one lifecycle row".to_owned());
    }
    let task_status = rows[0]
        .split('|')
        .nth(2)
        .map(str::trim)
        .ok_or_else(|| "TASK-005 plan row lacks status".to_owned())?;
    let start_count = plan
        .lines()
        .filter(|line| line.starts_with("### TASK-005 start record"))
        .count();
    let completion_count = plan
        .lines()
        .filter(|line| line.starts_with("### TASK-005 completion record"))
        .count();
    let (proposal_status, lifecycle, authority) = match task_status {
        "`PENDING / READY FOR START`" if start_count == 0 && completion_count == 0 => {
            (READY_STATUS, "PENDING_READY_FOR_START", "NONE")
        }
        "`IN_PROGRESS`" if start_count == 1 && completion_count == 0 => {
            (ACTIVE_STATUS, "IN_PROGRESS", "TASK_005_ONLY")
        }
        "`DONE`" if start_count == 1 && completion_count == 1 => (DONE_STATUS, "DONE", "NONE"),
        _ => {
            return Err("TASK-005 lifecycle/status/start-record combination is invalid".to_owned());
        }
    };
    if proposal_statuses[0] != proposal_status {
        return Err("TASK-005 proposal status disagrees with Plan lifecycle".to_owned());
    }
    let record = format!(
        "TASK005_CANONICAL_GATE: ACCEPTED\n\
TASK005_SPECIFICATION_VERSION: 1.1.18\n\
TASK005_LIFECYCLE: {lifecycle}\n\
TASK005_IMPLEMENTATION_AUTHORITY: {authority}\n\
TASK005_PROPOSAL: docs/proposals/TASK-005-GATE-PROPOSAL.md"
    );
    for (name, document) in [
        ("Specification", specification),
        ("Decisions", decisions),
        ("Review", review),
        ("Plan", plan),
        ("Intake", intake),
        ("AGENTS", agents),
    ] {
        if document.match_indices(&record).count() != 1 {
            return Err(format!(
                "TASK-005 pre-start gate lacks one synchronized canonical record in {name}"
            ));
        }
    }
    if !adr.starts_with("# ADR-0007:") || !adr.contains("- Status: ACCEPTED") {
        return Err("TASK-005 pre-start gate requires accepted ADR-0007".to_owned());
    }
    for id in ACCEPTANCE.iter().chain(TESTS) {
        for (name, document) in [
            ("Specification", specification),
            ("Plan", plan),
            ("Proposal", proposal),
        ] {
            if !extract_ids(document).iter().any(|observed| observed == id) {
                return Err(format!("TASK-005 {name} is missing {id}"));
            }
        }
    }
    if lifecycle == "IN_PROGRESS" || lifecycle == "DONE" {
        let start_record = plan
            .split("### TASK-005 start record")
            .nth(1)
            .ok_or_else(|| "TASK-005 active state lacks start record body".to_owned())?;
        for required in ACCEPTANCE.iter().chain(TESTS) {
            if !extract_ids(start_record)
                .iter()
                .any(|observed| observed == required)
            {
                return Err(format!(
                    "TASK-005 active start record is missing {required}"
                ));
            }
        }
        for required in [
            "STATUS: IN_PROGRESS",
            "FORMAL_COMPLETION_GATE: scripts/verify-task-005.sh formal",
            "FORBIDDEN: proposal §3.2; TASK-006 and later remain unauthorized",
        ] {
            if !start_record.contains(required) {
                return Err(format!(
                    "TASK-005 active start record is missing {required}"
                ));
            }
        }
    }
    if lifecycle == "DONE" {
        let completion_record = plan
            .split("### TASK-005 completion record")
            .nth(1)
            .and_then(|tail| tail.split("\n## 6. Phases and gates").next())
            .ok_or_else(|| "DONE TASK-005 lacks a bounded completion record".to_owned())?;
        for required in ACCEPTANCE.iter().chain(TESTS) {
            if !extract_ids(completion_record)
                .iter()
                .any(|observed| observed == required)
            {
                return Err(format!("TASK-005 completion record is missing {required}"));
            }
        }
        for required in [
            "Evidence commit: `f516faafe50707b88f51f25c03be07f917f8943f`",
            "reviewed GitHub Actions run `33073580258`",
            "Required unexecuted tests: `NONE`",
        ] {
            if !completion_record.contains(required) {
                return Err(format!(
                    "TASK-005 completion record is missing exact evidence: {required}"
                ));
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_task_006_start_gate(
    plan: &str,
    proposal: &str,
    specification: &str,
    decisions: &str,
    review: &str,
    intake: &str,
    agents: &str,
    adr: &str,
    definitions: &BTreeMap<String, PathBuf>,
) -> Result<(), String> {
    const ACCEPTANCE: &[&str] = &[
        "AC-082", "AC-083", "AC-084", "AC-085", "AC-086", "AC-087", "AC-088", "AC-089", "AC-090",
    ];
    const TESTS: &[&str] = &[
        "TEST-DOMAIN-006",
        "TEST-MAPPER-006",
        "TEST-MIGRATION-006",
        "TEST-SCHEMA-006",
        "TEST-COMMAND-006",
        "TEST-CONCURRENCY-006",
        "TEST-EVENT-006",
        "TEST-CUSTODY-006",
        "TEST-ERROR-006",
        "TEST-RECOVERY-006",
        "TEST-LIFECYCLE-006",
        "TEST-ARCH-006",
        "TEST-SUPPLY-006",
        "TEST-DOC-006",
    ];
    if !proposal.contains("status: \"ACCEPTED_INCORPORATED_BY_CANONICAL_SPECIFICATION_1_1_22\"")
        || !proposal.contains("TASK006_PROPOSAL_VERSION: 0.2.2")
        || proposal.contains("BLOCKED_PENDING_USER_ACCEPTANCE")
    {
        return Err("TASK-006 proposal is not the accepted v0.2.2 contract".to_owned());
    }
    let rows: Vec<_> = plan
        .lines()
        .filter(|line| line.starts_with("| `TASK-006` Asset domain/persistence |"))
        .collect();
    if rows.len() != 1 {
        return Err("TASK-006 Plan must contain exactly one lifecycle row".to_owned());
    }
    let task_status = rows[0]
        .split('|')
        .nth(2)
        .map(str::trim)
        .ok_or_else(|| "TASK-006 Plan row lacks status".to_owned())?;
    let start_count = plan
        .lines()
        .filter(|line| line.starts_with("### TASK-006 start record"))
        .count();
    let completion_count = plan
        .lines()
        .filter(|line| line.starts_with("### TASK-006 completion record"))
        .count();
    let (lifecycle, authority) = match task_status {
        "`IN_PROGRESS`" if start_count == 1 && completion_count == 0 => {
            ("IN_PROGRESS", "TASK_006_ONLY")
        }
        "`DONE`" if start_count == 1 && completion_count == 1 => ("DONE", "NONE"),
        _ => {
            return Err(
                "TASK-006 lifecycle/status/start/completion-record combination is invalid"
                    .to_owned(),
            );
        }
    };
    if start_count != 1 {
        return Err("TASK-006 must have exactly one start record".to_owned());
    }
    let record = format!(
        "TASK006_CANONICAL_GATE: ACCEPTED\n\
TASK006_SPECIFICATION_VERSION: 1.1.22\n\
TASK006_LIFECYCLE: {lifecycle}\n\
TASK006_IMPLEMENTATION_AUTHORITY: {authority}"
    );
    let proposal_lifecycle = format!(
        "TASK006_LIFECYCLE: {lifecycle}\n\
TASK006_IMPLEMENTATION_AUTHORITY: {authority}"
    );
    if proposal.match_indices(&proposal_lifecycle).count() != 1 {
        return Err("TASK-006 proposal lifecycle disagrees with the Plan".to_owned());
    }
    for (name, document) in [
        ("Specification", specification),
        ("Decisions", decisions),
        ("Review", review),
        ("Plan", plan),
        ("Intake", intake),
        ("AGENTS", agents),
    ] {
        if document.match_indices(&record).count() != 1 {
            return Err(format!(
                "TASK-006 start gate lacks one synchronized authority record in {name}"
            ));
        }
    }
    if !adr.starts_with("# ADR-0008:") || !adr.contains("- Status: `ACCEPTED`") {
        return Err("TASK-006 requires accepted ADR-0008".to_owned());
    }
    let start_record = plan
        .split("### TASK-006 start record")
        .nth(1)
        .and_then(|tail| {
            tail.split("\n### TASK-006 completion record")
                .next()
                .and_then(|body| body.split("\n## 6. Phases and gates").next())
        })
        .ok_or_else(|| "TASK-006 start record body is missing".to_owned())?;
    for id in ACCEPTANCE.iter().chain(TESTS) {
        if !definitions.contains_key(*id) {
            return Err(format!(
                "TASK-006 stable ID lacks canonical definition: {id}"
            ));
        }
        for (name, document) in [
            ("Specification", specification),
            ("Plan start record", start_record),
            ("Proposal", proposal),
        ] {
            if !extract_ids(document).iter().any(|observed| observed == id) {
                return Err(format!("TASK-006 {name} is missing {id}"));
            }
        }
    }
    for required in [
        "STATUS: IN_PROGRESS",
        "DEVELOPER_GATE: scripts/verify-task-006.sh developer",
        "FORMAL_COMPLETION_GATE: scripts/verify-task-006.sh formal",
        "FORBIDDEN: proposal §3.1; TASK-007 and later remain unauthorized",
    ] {
        if !start_record.contains(required) {
            return Err(format!("TASK-006 start record is missing {required}"));
        }
    }
    if !proposal.contains("candidate byte length: 12733")
        || !proposal.contains(
            "candidate SHA-256: 91c76e615fe248abd852860dcd42b32a01f6f024e91ac8387f34069be2435db1",
        )
    {
        return Err("TASK-006 proposal migration identity changed".to_owned());
    }
    if lifecycle == "DONE" {
        let completion_record = plan
            .split("### TASK-006 completion record")
            .nth(1)
            .and_then(|tail| tail.split("\n## 6. Phases and gates").next())
            .ok_or_else(|| "DONE TASK-006 lacks a bounded completion record".to_owned())?;
        for required in ACCEPTANCE.iter().chain(TESTS) {
            if !extract_ids(completion_record)
                .iter()
                .any(|observed| observed == required)
            {
                return Err(format!("TASK-006 completion record is missing {required}"));
            }
        }
        for required in [
            "Evidence commit: `60b6616c20d677632ca25b8b72340fc3a639db54`",
            "Review correction commit: `10455605556984e48def16efc27fb52338109944`",
            "reviewed GitHub Actions run `33257331689`",
            "Required unexecuted tests: `NONE`",
            "`SEC-017`: `PASS`",
            "`SEC-020`: `PASS`",
            "`SEC-021`: `PASS`",
        ] {
            if !completion_record.contains(required) {
                return Err(format!(
                    "TASK-006 completion record is missing exact evidence: {required}"
                ));
            }
        }
        for required in [
            "## 19. Formal completion evidence",
            "`33257331689`",
            "TASK-006 is `DONE`, its implementation",
            "authority is `NONE`",
        ] {
            if !proposal.contains(required) {
                return Err(format!(
                    "DONE TASK-006 proposal is missing completion evidence: {required}"
                ));
            }
        }
    }
    Ok(())
}

fn validate_task_003_gate_state(
    plan: &str,
    proposal: &str,
    specification: &str,
    decisions: &str,
    review: &str,
    intake: &str,
    agents: &str,
) -> Result<(), String> {
    const DRAFT_STATUS: &str =
        "> Status: **DRAFT / BLOCKED — REVIEW REQUIRED; NO IMPLEMENTATION AUTHORITY**";
    const ACCEPTED_STATUS_PREFIX: &str =
        "> Status: **ACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION v";
    const BLOCKERS: &[&str] = &[
        "TASK003-BLOCKER-001",
        "TASK003-BLOCKER-002",
        "TASK003-BLOCKER-003",
        "TASK003-BLOCKER-004",
        "TASK003-BLOCKER-005",
        "TASK003-BLOCKER-006",
        "TASK003-BLOCKER-007",
        "TASK003-BLOCKER-008",
        "TASK003-BLOCKER-009",
        "TASK003-BLOCKER-010",
        "TASK003-BLOCKER-011",
        "TASK003-BLOCKER-012",
    ];

    let task_rows: Vec<_> = plan
        .lines()
        .filter(|line| line.starts_with("| `TASK-003` IPC, framing, Client identity |"))
        .collect();
    if task_rows.len() != 1 {
        return Err("TASK-003 plan must contain exactly one lifecycle row".to_owned());
    }
    let task_status = task_rows[0]
        .split('|')
        .nth(2)
        .map(str::trim)
        .ok_or_else(|| "TASK-003 plan row has no status cell".to_owned())?;

    let proposal_statuses: Vec<_> = proposal
        .lines()
        .filter(|line| line.starts_with("> Status: **"))
        .collect();
    if proposal_statuses.len() != 1 {
        return Err("TASK-003 proposal must contain exactly one top-level status".to_owned());
    }
    let proposal_status = proposal_statuses[0];
    let start_headings: Vec<_> = plan
        .lines()
        .filter(|line| line.starts_with("### TASK-003 start record"))
        .collect();
    let completion_headings: Vec<_> = plan
        .lines()
        .filter(|line| line.starts_with("### TASK-003 completion record"))
        .collect();
    let mut blocker_statuses = BTreeMap::new();
    for blocker in BLOCKERS {
        let heading = format!("### `{blocker}`");
        if proposal
            .lines()
            .filter(|line| line.starts_with(&heading))
            .count()
            != 1
        {
            return Err(format!(
                "TASK-003 proposal must contain exactly one blocker heading for {blocker}"
            ));
        }
        let section = proposal
            .split(&heading)
            .nth(1)
            .and_then(|tail| tail.split("\n### ").next())
            .ok_or_else(|| format!("TASK-003 proposal is missing blocker section {blocker}"))?;
        let statuses: Vec<_> = section
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("- Status:"))
            .collect();
        if statuses.len() != 1
            || !matches!(statuses[0], "- Status: **OPEN**" | "- Status: **RESOLVED**")
        {
            return Err(format!(
                "TASK-003 blocker {blocker} must have one exact OPEN/RESOLVED status"
            ));
        }
        blocker_statuses.insert(*blocker, statuses[0]);
    }

    match task_status {
        "`PENDING`" => {
            if proposal_status != DRAFT_STATUS {
                return Err("PENDING TASK-003 requires the exact draft status".to_owned());
            }
            if !start_headings.is_empty() || !completion_headings.is_empty() {
                return Err("PENDING TASK-003 must not have start or completion records".to_owned());
            }
            for (name, document) in [
                ("Specification", specification),
                ("Decisions", decisions),
                ("Review", review),
                ("Plan", plan),
                ("Intake", intake),
                ("AGENTS", agents),
            ] {
                if document.contains("TASK003_CANONICAL_GATE: ACCEPTED") {
                    return Err(format!(
                        "PENDING TASK-003 must not have an accepted gate record in {name}"
                    ));
                }
            }
        }
        "`IN_PROGRESS`" | "`DONE`" => {
            let version = proposal_status
                .strip_prefix(ACCEPTED_STATUS_PREFIX)
                .and_then(|value| value.strip_suffix("**"))
                .filter(|value| {
                    !value.is_empty()
                        && value
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || byte == b'.')
                })
                .ok_or_else(|| {
                    "active TASK-003 requires one exact versioned accepted status".to_owned()
                })?;
            let recorded_version = specification
                .lines()
                .find_map(|line| line.strip_prefix("TASK003_SPECIFICATION_VERSION: "));
            if recorded_version != Some(version) {
                return Err(format!(
                    "TASK-003 accepted status disagrees with its canonical historical Specification v{version} record"
                ));
            }
            let lifecycle = task_status.trim_matches('`');
            let canonical_record = format!(
                "TASK003_CANONICAL_GATE: ACCEPTED\n\
TASK003_SPECIFICATION_VERSION: {version}\n\
TASK003_LIFECYCLE: {lifecycle}\n\
TASK003_PROPOSAL: docs/proposals/TASK-003-GATE-PROPOSAL.md"
            );
            for (name, document) in [
                ("Specification", specification),
                ("Decisions", decisions),
                ("Review", review),
                ("Plan", plan),
                ("Intake", intake),
                ("AGENTS", agents),
            ] {
                if document.match_indices(&canonical_record).count() != 1 {
                    return Err(format!(
                        "active TASK-003 requires one exact synchronized gate record in {name}"
                    ));
                }
            }
            let error_taxonomy_acceptance_record = "TASK003_ERROR_TAXONOMY_CONFLICT: ACCEPTED\n\
TASK003_ERROR_CODES_ADDED: IPC_TRANSPORT_ERROR; PROTOCOL_VERSION_UNSUPPORTED; DEADLINE_EXCEEDED\n\
TASK003_STORAGE_IO_SOURCE_PRESERVED: filesystem/backend\n\
TASK003_UNSUPPORTED_CAPABILITY_SOURCE_PRESERVED: declared Provider/Plugin capability contract";
            for (name, document) in [("Specification", specification), ("Decisions", decisions)] {
                if document
                    .match_indices(error_taxonomy_acceptance_record)
                    .count()
                    != 1
                {
                    return Err(format!(
                        "active TASK-003 requires one exact error-taxonomy decision in {name}"
                    ));
                }
            }
            let cross_task_acceptance_record = "TASK003_AC_OWNERSHIP_CONFLICT: ACCEPTED\n\
TASK003_AC_028_CONTRIBUTORS: TASK-003; TASK-007\n\
TASK003_AC_028_TERMINAL_OWNER: TASK-013\n\
TASK003_AC_029_CONTRIBUTORS: TASK-003; TASK-013; TASK-016; TASK-022\n\
TASK003_AC_029_TASK013_BRANCHES: PLUGIN_GRANT; AUDIT_EXPORT; MANUAL_MIGRATION_ADMIN\n\
TASK003_AC_029_TASK016_BRANCHES: CREDENTIAL\n\
TASK003_AC_029_TASK022_BRANCHES: PURGE\n\
TASK003_AC_029_TERMINAL_OWNER: TASK-023";
            for (name, document) in [
                ("Specification", specification),
                ("Decisions", decisions),
                ("Plan", plan),
            ] {
                if document.match_indices(cross_task_acceptance_record).count() != 1 {
                    return Err(format!(
                        "active TASK-003 requires one exact cross-task AC owner map in {name}"
                    ));
                }
            }
            for (name, document, stale) in [
                ("Specification", specification, "TASK_003_PENDING_OWN_GATE"),
                (
                    "Review",
                    review,
                    "status: \"TASK_004_COMPLETE_WITH_LATER_GATES\"",
                ),
                (
                    "Plan",
                    plan,
                    "status: \"TASK_004_DONE_TASK_003_PENDING_GATE\"",
                ),
                (
                    "Intake",
                    intake,
                    "status: \"TASK_004_DONE_TASK_003_PENDING_GATE\"",
                ),
                ("AGENTS", agents, "TASK-003 pending own gate"),
            ] {
                if document.contains(stale) {
                    return Err(format!(
                        "active TASK-003 retains a stale PENDING current-state marker in {name}"
                    ));
                }
            }
            let agents_lifecycle = if lifecycle == "IN_PROGRESS" {
                "TASK-003 in progress"
            } else {
                "TASK-003 complete"
            };
            if !agents.contains(agents_lifecycle) {
                return Err(format!(
                    "active TASK-003 AGENTS state lacks {agents_lifecycle}"
                ));
            }
            if task_rows[0].contains("AC-028")
                || task_rows[0].contains("AC-029")
                || !task_rows[0].contains("AC-060, AC-061, AC-062, AC-063, AC-064")
            {
                return Err(
                    "active TASK-003 plan row must use the exact handshake-only AC-060..AC-064 set without redefining AC-028/AC-029"
                        .to_owned(),
                );
            }
            let later_rows = [
                ("TASK-013", "| `TASK-013` Lease/Asset Broker/audit |"),
                ("TASK-016", "| `TASK-016` Secret/Network Brokers |"),
                ("TASK-023", "| `TASK-023` release gate |"),
            ]
            .into_iter()
            .map(|(task, prefix)| {
                let rows: Vec<_> = plan
                    .lines()
                    .filter(|line| line.starts_with(prefix))
                    .collect();
                if rows.len() != 1 {
                    return Err(format!(
                        "active TASK-003 requires one canonical Plan row for {task}"
                    ));
                }
                Ok((task, rows[0]))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
            let task_013 = later_rows["TASK-013"];
            if !task_013.contains("TASK-007, TASK-009, TASK-012")
                || !task_013.contains("AC-024, AC-026, AC-028")
                || task_013.contains("AC-029")
            {
                return Err(
                    "active TASK-003 must synchronize TASK-013's AC-028 terminal ownership and TASK-007 dependency"
                        .to_owned(),
                );
            }
            if later_rows["TASK-016"].contains("AC-029") {
                return Err(
                    "active TASK-003 must remove premature AC-029 completion from TASK-016"
                        .to_owned(),
                );
            }
            if !later_rows["TASK-023"].contains("AC-029") {
                return Err(
                    "active TASK-003 must assign terminal AC-029 completion to TASK-023".to_owned(),
                );
            }
            for (blocker, status) in &blocker_statuses {
                if *status != "- Status: **RESOLVED**" {
                    return Err(format!(
                        "active TASK-003 requires RESOLVED status for {blocker}"
                    ));
                }
            }
            if start_headings.len() != 1 {
                return Err("active TASK-003 requires exactly one start record".to_owned());
            }

            let start_tail = plan
                .split(start_headings[0])
                .nth(1)
                .ok_or_else(|| "TASK-003 start record is unreadable".to_owned())?;
            let start_end = start_tail.find("\n### ").unwrap_or(start_tail.len());
            let start_section = &start_tail[..start_end];
            if start_section
                .lines()
                .filter(|line| line.trim() == "```text")
                .count()
                != 1
                || start_section
                    .lines()
                    .filter(|line| line.trim() == "```")
                    .count()
                    != 1
            {
                return Err("TASK-003 start record must contain exactly one text block".to_owned());
            }
            let start = start_section
                .split("```text")
                .nth(1)
                .and_then(|section| section.split("```").next())
                .ok_or_else(|| "TASK-003 start record text block is unreadable".to_owned())?;
            let proposal_start = proposal
                .split("### Accepted start record — active")
                .nth(1)
                .and_then(|section| section.split("```text").nth(1))
                .and_then(|section| section.split("```").next())
                .ok_or_else(|| "TASK-003 proposal start template is unreadable".to_owned())?;
            if start.trim() != proposal_start.trim() {
                return Err(
                    "TASK-003 canonical start record must exactly equal the accepted template"
                        .to_owned(),
                );
            }
            for field in [
                "TASK: TASK-003",
                "STATUS: IN_PROGRESS",
                "AUTHORIZED: exact §4 scope only",
                "FORBIDDEN: exact §4 forbidden list",
            ] {
                if start.lines().filter(|line| *line == field).count() != 1 {
                    return Err(format!(
                        "TASK-003 start record lacks one exact field: {field}"
                    ));
                }
            }
            for field in ["DEPENDENCIES:", "REQUIREMENTS:", "ACCEPTANCE:", "TESTS:"] {
                if start.lines().filter(|line| line.starts_with(field)).count() != 1 {
                    return Err(format!(
                        "TASK-003 start record lacks one exact list field: {field}"
                    ));
                }
            }
            if start.contains("..") {
                return Err("TASK-003 start record must enumerate every stable ID".to_owned());
            }

            let actual_id_list = extract_ids(start);
            let actual_ids: BTreeSet<_> = actual_id_list.iter().cloned().collect();
            if actual_id_list.len() != actual_ids.len() {
                return Err("TASK-003 start record contains a duplicate stable ID".to_owned());
            }
            let expected_ids: BTreeSet<_> = [
                "TASK-003",
                "TASK-002",
                "TASK-004",
                "BASE-007",
                "BASE-008",
                "BASE-012",
                "BASE-013",
                "BASE-014",
                "BASE-016",
                "BASE-017",
                "DEC-007",
                "DEC-012",
                "DEC-016",
                "DEC-017",
                "DEC-019",
                "DEC-021",
                "DEC-022",
                "ADR-0001",
                "ADR-0004",
                "ADR-0005",
                "FUNC-001",
                "API-001",
                "API-002",
                "API-003",
                "API-008",
                "API-009",
                "API-010",
                "SEC-005",
                "SEC-013",
                "SEC-014",
                "SEC-017",
                "SEC-020",
                "SEC-021",
                "REL-001",
                "REL-006",
                "CFG-001",
                "CFG-003",
                "AC-060",
                "AC-061",
                "AC-062",
                "AC-063",
                "AC-064",
                "TEST-PROTO-001",
                "TEST-FRAME-001",
                "TEST-HANDSHAKE-001",
                "TEST-IPC-MACOS-001",
                "TEST-ENDPOINT-003",
                "TEST-CONFIG-003",
                "TEST-AUTH-001",
                "TEST-CLI-001",
                "TEST-ARCH-003",
                "TEST-SUPPLY-003",
                "TEST-DOC-003",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect();
            if actual_ids != expected_ids {
                return Err(format!(
                    "TASK-003 start record ID set differs: expected {expected_ids:?}, got {actual_ids:?}"
                ));
            }

            if task_status == "`IN_PROGRESS`" && !completion_headings.is_empty() {
                return Err("IN_PROGRESS TASK-003 must not have a completion record".to_owned());
            }
            if task_status == "`DONE`" {
                if completion_headings.len() != 1 {
                    return Err("DONE TASK-003 requires exactly one completion record".to_owned());
                }
                let completion_tail = plan
                    .split(completion_headings[0])
                    .nth(1)
                    .ok_or_else(|| "DONE TASK-003 completion record is unreadable".to_owned())?;
                let completion_end = completion_tail
                    .match_indices("\n##")
                    .map(|(index, _)| index)
                    .min()
                    .unwrap_or(completion_tail.len());
                let completion = &completion_tail[..completion_end];
                for (required, expected_evidence) in [
                    ("AC-060", "TEST-FRAME-001"),
                    ("AC-061", "TEST-PROTO-001+TEST-HANDSHAKE-001"),
                    ("AC-062", "TEST-IPC-MACOS-001"),
                    ("AC-063", "TEST-AUTH-001+TEST-CLI-001+TEST-ARCH-003"),
                    (
                        "AC-064",
                        "TEST-HANDSHAKE-001+TEST-ENDPOINT-003+TEST-CONFIG-003",
                    ),
                    (
                        "TEST-PROTO-001",
                        "scripts/verify-task-003.sh#TEST-PROTO-001",
                    ),
                    (
                        "TEST-FRAME-001",
                        "scripts/verify-task-003.sh#TEST-FRAME-001",
                    ),
                    (
                        "TEST-HANDSHAKE-001",
                        "scripts/verify-task-003.sh#TEST-HANDSHAKE-001",
                    ),
                    (
                        "TEST-IPC-MACOS-001",
                        "scripts/verify-task-003-formal-second-uid.sh#TEST-IPC-MACOS-001",
                    ),
                    (
                        "TEST-ENDPOINT-003",
                        "scripts/verify-task-003.sh#TEST-ENDPOINT-003",
                    ),
                    (
                        "TEST-CONFIG-003",
                        "scripts/verify-task-003.sh#TEST-CONFIG-003",
                    ),
                    ("TEST-AUTH-001", "scripts/verify-task-003.sh#TEST-AUTH-001"),
                    ("TEST-CLI-001", "scripts/verify-task-003.sh#TEST-CLI-001"),
                    ("TEST-ARCH-003", "scripts/verify-task-003.sh#TEST-ARCH-003"),
                    (
                        "TEST-SUPPLY-003",
                        "scripts/verify-task-003.sh#TEST-SUPPLY-003",
                    ),
                    ("TEST-DOC-003", "scripts/verify-task-003.sh#TEST-DOC-003"),
                ] {
                    let evidence_prefix = format!("`{required}`: `PASS`; EVIDENCE: ");
                    let exact_line = format!("`{required}`: `PASS`; EVIDENCE: {expected_evidence}");
                    let evidence_lines: Vec<_> = completion
                        .lines()
                        .map(str::trim)
                        .filter(|line| line.starts_with(&evidence_prefix))
                        .collect();
                    if evidence_lines.len() != 1 || evidence_lines[0] != exact_line {
                        return Err(format!(
                            "DONE TASK-003 requires exact mapped PASS evidence for {required}"
                        ));
                    }
                }
                if ["`SKIP`", "`UNVERIFIABLE`", "`PARTIAL`", "`FAIL`"]
                    .iter()
                    .any(|status| completion.contains(status))
                {
                    return Err(
                        "DONE TASK-003 completion evidence contains a non-PASS status".to_owned(),
                    );
                }
                for (prefix, exact_provenance) in [
                    (
                        "FORMAL_SECOND_UID_CI_REPOSITORY: ",
                        "FORMAL_SECOND_UID_CI_REPOSITORY: XiaTian-X/MengXia",
                    ),
                    (
                        "FORMAL_SECOND_UID_CI_WORKFLOW: ",
                        "FORMAL_SECOND_UID_CI_WORKFLOW: .github/workflows/ci.yml",
                    ),
                    (
                        "FORMAL_SECOND_UID_CI_JOB: ",
                        "FORMAL_SECOND_UID_CI_JOB: task-003-second-uid",
                    ),
                    (
                        "FORMAL_SECOND_UID_CI_RUNNER: ",
                        "FORMAL_SECOND_UID_CI_RUNNER: macos-26",
                    ),
                ] {
                    let provenance_lines: Vec<_> = completion
                        .lines()
                        .map(str::trim)
                        .filter(|line| line.starts_with(prefix))
                        .collect();
                    if provenance_lines.len() != 1 || provenance_lines[0] != exact_provenance {
                        return Err(format!(
                            "DONE TASK-003 requires exact formal CI provenance: {exact_provenance}"
                        ));
                    }
                }
                let formal_commits: Vec<_> = completion
                    .lines()
                    .map(str::trim)
                    .filter_map(|line| line.strip_prefix("FORMAL_SECOND_UID_CI_COMMIT: "))
                    .collect();
                if formal_commits.len() != 1
                    || formal_commits[0].len() != 40
                    || !formal_commits[0]
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(
                        "DONE TASK-003 requires one 40-character lowercase formal CI commit"
                            .to_owned(),
                    );
                }
                let formal_runs: Vec<_> = completion
                    .lines()
                    .map(str::trim)
                    .filter_map(|line| line.strip_prefix("FORMAL_SECOND_UID_CI_RUN: "))
                    .collect();
                if formal_runs.len() != 1
                    || formal_runs[0].is_empty()
                    || !formal_runs[0].bytes().all(|byte| byte.is_ascii_digit())
                    || formal_runs[0].bytes().all(|byte| byte == b'0')
                {
                    return Err(
                        "DONE TASK-003 requires one positive formal second-UID CI run ID"
                            .to_owned(),
                    );
                }
                let formal_results: Vec<_> = completion
                    .lines()
                    .map(str::trim)
                    .filter(|line| line.starts_with("FORMAL_SECOND_UID_CI_RESULT: "))
                    .collect();
                if formal_results.len() != 1
                    || formal_results[0] != "FORMAL_SECOND_UID_CI_RESULT: PASS"
                {
                    return Err(
                        "DONE TASK-003 requires one exact formal second-UID CI PASS result"
                            .to_owned(),
                    );
                }
            }
        }
        _ => return Err("TASK-003 has an unsupported lifecycle status".to_owned()),
    }

    for required in [
        "TASK003-BLOCKER-001",
        "TASK003-BLOCKER-002",
        "TASK003-BLOCKER-003",
        "TASK003-BLOCKER-004",
        "TASK003-BLOCKER-005",
        "TASK003-BLOCKER-006",
        "TASK003-BLOCKER-007",
        "TASK003-BLOCKER-008",
        "TASK003-BLOCKER-009",
        "TASK003-BLOCKER-010",
        "TASK003-BLOCKER-011",
        "TASK003-BLOCKER-012",
        "## 5. Proposed exact wire contract",
        "### 5.1 Decode-depth enforcement",
        "## 6. Proposed bounded lifecycle",
        "## 7. Proposed opened-Library composition API",
        "## 8. Proposed configuration and runtime endpoint contract",
        "## 9. Proposed dependencies and reproducible code generation",
        "## 11. Proposed stable registry and start-record template",
        "| `MENGXIA_MAX_DECODE_DEPTH` | 64 |",
        "`prost` keeps its default recursion guard as defense in depth",
        "`safe_details` MUST be empty",
        "a peer negotiated to\n  1.0 always retains this terminal-close behavior",
        "every unfinished Tokio\n  task is explicitly aborted",
        "`TMPDIR` is an\n  explicitly declared untrusted source",
        "fixed `.mengxia.runtime-owner-v1.staging`",
        "A zero/partial staging file is never deleted, truncated,\n  overwritten or recreated automatically",
        "preserved operator-visible fail-closed state",
        "### 8.3 Exact TASK-003 CLI/daemon contract",
        "mengxiad serve [--library-root PATH] [--client-endpoint PATH]",
        "mengxia handshake [--client-endpoint PATH] [--max-frame-bytes ASCII_U64]",
        "MENGXIA_HANDSHAKE_OK protocol=1.0 request_id=<canonical UUIDv7>",
        "MENGXIA_ERROR code=<ERROR_CODE>",
        "connect/write/flush/read/reset or EOF before a complete response to generic\n  `IPC_TRANSPORT_ERROR`",
        "the server uses\n  `PROTOCOL_VERSION_UNSUPPORTED` only when a valid `ClientHello` has no common version",
        "Expiry of the one absolute client deadline is `DEADLINE_EXCEEDED`",
        "`IPC_TRANSPORT_ERROR`; source = local IPC connect/write/flush/read/close transport",
        "`PROTOCOL_VERSION_UNSUPPORTED`; source = authenticated local IPC version",
        "The first SIGINT or SIGTERM\n  starts exactly §6's",
        "`--help` is side-effect free",
        "finally-style error aggregation",
        "private `#[cfg(test)]` pure authorization seam",
        "private `#[cfg(test)]` listener fixture",
        "production-path connection fails at the OS boundary",
        "MENGXIA_TASK003_TEST_ROLE=second_uid_client",
        "MENGXIA_TASK003_TEST_ENDPOINT=<exact fixture path>",
        "task_003_real_second_uid_peer_is_rejected_before_frame",
        "mengxia-task003-ci",
        "/usr/bin/sudo -n -u mengxia-task003-ci --",
        "first unused decimal UID in the closed range 600..=699",
        "eUID 0 follows the same equality checks but receives no containment",
        "/bin/test -x <absolute-current-test-executable>",
        "cargo test -p mengxiad --bin mengxiad --locked --offline task_003_real_second_uid_peer_is_rejected_before_frame -- --exact --ignored --nocapture",
        "CONFLICT:\nSource A: Specification v1.1.14 says Option A",
        "CONFLICT:\nSource A: Specification v1.1.14 limits STORAGE_IO_ERROR",
        "TASK003_ERROR_TAXONOMY_CONFLICT: ACCEPTED",
        "manual/destructive Library migration\n  administration",
        "TASK003_AC_OWNERSHIP_CONFLICT: ACCEPTED",
        "TASK003_AC_029_TASK013_BRANCHES: PLUGIN_GRANT; AUDIT_EXPORT; MANUAL_MIGRATION_ADMIN",
        "TASK003_AC_028_TERMINAL_OWNER: TASK-013",
        "TASK003_AC_029_TERMINAL_OWNER: TASK-023",
        "`scripts/verify-task-003-formal-second-uid.sh` and\n`scripts/run-task-003-second-uid.sh`",
        "task003_run TEST-IPC-MACOS-001 -- ./scripts/run-task-003-second-uid.sh",
        "task003_run TEST-PROTO-001 -- cargo test",
        "comment-only fake map and a failing mapped command",
        "FORMAL_SECOND_UID_CI_REPOSITORY: XiaTian-X/MengXia",
        "REL-001; REL-006; CFG-001; CFG-003",
    ] {
        if !proposal.contains(required) {
            return Err(format!(
                "TASK-003 gate is missing required draft evidence: {required}"
            ));
        }
    }

    Ok(())
}

fn validate_task_004_active_contract(
    plan: &str,
    proposal: &str,
    definitions: &BTreeMap<String, PathBuf>,
) -> Result<(), String> {
    for marker in [
        "Status: **ACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION v1.1.13**",
        "authorizes only the exact TASK-004 start record",
        "no custom SQLite VFS",
        "SQLITE_OPEN_NOFOLLOW",
        "third_party/libsqlite3-sys-0.38.2",
        "repository-explicit SQLite C defines are exactly",
        "SQLITE_OMIT_LOAD_EXTENSION",
        "sqlite-compile-options-allowlist.txt",
        "exact migration filename is `0000_store_bootstrap.sql`",
        "`migration_name` is `0000_store_bootstrap`",
        "ORDER BY migration_sequence ASC",
        ".mengxia.bootstrap-intent",
        "Names, file type, UID and mode alone never grant cleanup permission",
        "Complete recovery state table",
        "Lock only",
        "may be created only while descriptor-relative enumeration proves",
        "would permit split-brain",
        "`fsync` the lock file; `fsync` the",
        "post-lock snapshot",
        "owned by the daemon effective UID",
        "must be local APFS",
        "Every retained prefix directory",
        "MNT_IGNORE_OWNERSHIP",
        "`mengxia-platform-fs`",
        "accepted eighteenth canonical",
        "`mengxia-store-sqlite` keeps",
        "`#![forbid(unsafe_code)]`",
        "Apple ACL calls are absent from `libc` 0.2.189",
        "include/mengxia_acl_shim.h",
        "src/macos_acl_shim.c",
        "src/macos_acl_abi_probe.c",
        "No `cc` crate, shell, PATH lookup or response file participates",
        "macos-acl-ffi-toolchain-v1.toml",
        "Xcode `26.6` build `17F113`",
        "d2e4bf622758eee1bf7267c060497fb2c41e098d37b0fca8be73898dc7e14eda",
        "9511f84f0abe1e108e10979900d4fea8567534aef78f0984f7050c49f6c29ff7",
        "0d41e97fd26c5dd2a268ddb1a5c07b7f8f9e6f0cd28922d92b5b19aec7c42849",
        "`TASK004-BLOCKER-008`",
        "`TASK004-BLOCKER-009`",
        "current `.github/workflows/ci.yml` uses `runs-on: macos-15`",
        "`runs-on: macos-26`",
        "/usr/bin/sudo /usr/bin/xcode-select --switch\n  /Applications/Xcode_26.6.app/Contents/Developer",
        "runner label is discovery evidence, not proof",
        "`ImageOS`, `ImageVersion`",
        "before every Cargo invocation",
        "`/Applications` is the sole writable-\n  ancestor exception",
        "| `/Applications` | real directory",
        "UID `0`, GID `80` (`admin`), mode exactly `0775`",
        "OS root and members of GID 80 are privileged",
        "`/usr/bin/id -u`, `/usr/bin/id -g` and `/usr/bin/id -G`",
        "every repository/dependency subprocess\nlaunched with the build account's credentials",
        "malicious/concurrent root, admin\nor same-credential build process is out of scope",
        "`developer` is the default local build class",
        "`attested` is selected only by the formal CI/release verification command",
        "owner is UID `0` or the recorded build eUID",
        "same pure metadata-policy function",
        "never chmod, chown or replace real `/Applications`",
        "post-archive mutation",
        "Command::env_clear()",
        "`CC`, `CFLAGS`, `CPPFLAGS`, `CPATH`",
        "`MENGXIA_ACL_TESTING` is always rejected",
        "mengxia-acl-build-command-v1.json",
        "__MAC_OS_X_VERSION_MAX_ALLOWED < 101300",
        "struct mengxia_acl_summary_v1",
        "exactly 40 bytes with alignment 4",
        "0=OK",
        "6=ENTRY_LIMIT_EXCEEDED",
        "7=UNKNOWN_FLAG_BITS",
        "ACL_MAX_ENTRIES==128",
        "ACL_ENTRY_ONLY_INHERIT",
        "extern acl_t acl_get_fd_np",
        "extern acl_t acl_dup",
        "extern ssize_t acl_copy_ext",
        "acl_get_fd_np",
        "acl_get_tag_type",
        "acl_get_flagset_np",
        "acl_get_flag_np",
        "iterate entries separately",
        "`acl_flags` is read by calling `acl_get_flagset_np` on the `acl_t` itself",
        "`entry_flags_or` is\nproduced by a separate call on every `acl_entry_t`",
        "portable big-endian `acl_copy_ext`",
        "after 128 accepted entries",
        "16,384 bytes",
        "neither the ACL-object flag word nor any entry flag word contained",
        "private hidden C\nbackend table",
        "there is no production/test preprocessor branch",
        "Zero ACL\nentries is accepted",
        "ACL-object `DEFER_INHERIT` is\nforbidden",
        "the only permitted ACL-object bit is `NO_INHERIT`",
        "`entry_count == 0`, `acl_flags == 0`",
        "no ACE can be copied to the new root",
        "one private `macos_ffi` module",
        "no public unchecked constructor",
        "`FixedSqliteChildPath<'_>`",
        "This trait **does permit** any Rust holder to copy or\nformat the path",
        "Repository policy—not the Rust\ntype system",
        "must compile successfully and\nthen fail this repository architecture lint",
        "stock_sqlite_open",
        "Separate compile-fail fixtures cover only private",
        "`O_NOFOLLOW` on one `open` protects only",
        "creates a `ValidatedAbsolutePath` by walking from",
        "`ValidatedAbsolutePath::revalidate_chain()`",
        "Merely `fstat`-ing the old handles",
        "`SQLITE_OPEN_NOFOLLOW` is defense in",
        "root process or arbitrary process already running",
        "Invalid/truncated intent",
        ".library.sqlite3.bootstrap",
        "linkat",
        "exactly 256-byte",
        "SHA-256 over bytes `[0, 224)` only",
        "61d3132622fa1ef1e69b1062be3b1a0eb4af990ce36153a041f7a4dce8a180f7",
        "encode-to-golden",
        "STORAGE_BUSY",
        "StorageBusy",
        "storage_busy_total",
        "STORAGE_CONFIGURATION_ERROR",
        "StorageConfigurationError",
        "storage_configuration_errors_total",
        "shared-cache mode is disabled",
        "sqlite3_busy_timeout",
        "not a guaranteed minimum",
        "ID_GENERATION_UNAVAILABLE",
        "`NOMEM`; filesystem",
        "unknown primary code maps to `INTERNAL_ERROR`",
        "linearization point is successful insertion",
        "Shutdown is the sole",
        "Typed `StoreConfig` DTO and pure validation boundary",
        "precedence—CLI flag, then\nenvironment",
        "`MENGXIA_LIBRARY_ROOT`",
        "`ResolvedStoreConfig`",
        "never reads CLI arguments",
        "does not implement or test a substitute production resolver",
        "contains no layered-resolver\nfixture",
        "TASK-003 canonicalization must allocate and trace the IDs",
        "TASK-004 may complete its\nstore-only scope",
        "TEST-CONFIG-004",
        "MENGXIA_DB_BUSY_TIMEOUT_MS",
        "every\nfailure preceding any Library-root entry",
        "complete `sqlite_schema` row set exactly matches this allowlist",
        "no view, trigger, manual index",
        "BootstrapClock",
        "seconds and nanos must be byte-for-byte equal",
        "worker panic/join failure or invariant defect",
        "WAL and SHM expected-result matrix",
        "SHM is never treated as canonical database content",
        "Required WAL-reset concurrency regression",
        "writer A, writer B",
        "standalone test-only engine database",
        "wal_reset_probe",
        "production code cannot",
        "Every checkpoint call has a phase-specific allowed result",
        "SQLITE_CHECKPOINT_FULL",
        "SQLITE_CHECKPOINT_RESTART",
        "SQLITE_CHECKPOINT_TRUNCATE",
        "16 fixed deterministic schedule seeds",
        "full intent record written, before intent-file fsync",
        "| 23 | valid canonical",
        "SIGKILL visibility",
        "sealed `BootstrapFsOps` fault seam",
        "absence or rollback of a returned",
        "process-termination recovery only",
        "does not enable SQLite `fullfsync`",
        "`F_FULLFSYNC`",
        "power-loss or physical-media ordering claim",
        "`library_meta.created_at_seconds/nanos` exactly equal",
        "intent UUID/migration/timestamp mismatch",
        "TEST-WAL-004",
        "TEST-CORRUPTION-004",
        "AC-073",
        "TEST-PATH-004",
        "Required whole-prefix and FFI-boundary matrix",
        "SQLite-open seam is not called",
        "a conforming non-inheritable deny-only ancestor/final-parent ACL is not an error",
        "explicitly no production precedence PASS claim",
    ] {
        if !proposal.contains(marker) {
            return Err(format!("TASK-004 accepted contract is missing {marker}"));
        }
    }

    let task_done = plan.contains("| `TASK-004` SQLite/migration engine | `DONE` |");
    if !task_done && !plan.contains("| `TASK-004` SQLite/migration engine | `IN_PROGRESS` |") {
        return Err("accepted TASK-004 must be IN_PROGRESS or DONE".to_owned());
    }

    let implementation_scope = proposal
        .split("## 8. Exact authorized implementation scope")
        .nth(1)
        .and_then(|section| section.split("## 9. Stable acceptance registry").next())
        .ok_or_else(|| "TASK-004 exact implementation scope is missing".to_owned())?;
    for marker in [
        "`.github/workflows/ci.yml`",
        "arm64 `macos-26` runner",
        "fail-closed platform preflight before Cargo",
        "preserving all existing security and TASK-001 gates",
    ] {
        if !implementation_scope.contains(marker) {
            return Err(format!(
                "TASK-004 implementation scope is missing CI boundary: {marker}"
            ));
        }
    }

    let task_004_row = plan
        .lines()
        .find(|line| line.starts_with("| `TASK-004` SQLite/migration engine |"))
        .ok_or_else(|| "TASK-004 plan row is missing".to_owned())?;
    for required in ["CFG-001", "CFG-003", "BASE-015"] {
        if !extract_ids(task_004_row).iter().any(|id| id == required) {
            return Err(format!("TASK-004 plan row is missing {required}"));
        }
    }
    if !plan.contains("### TASK-004 start record") {
        return Err("active TASK-004 must have a canonical start record".to_owned());
    }
    if plan.contains("no blocker applies to TASK-001..TASK-005") {
        return Err("Phase 0 must not erase TASK-004's task-local blocker".to_owned());
    }
    if proposal.contains("## 11. Proposed start-record template")
        || proposal.contains("The proposed compile-time set is exactly:")
        || proposal.contains("SQLite `BUSY`/`LOCKED` after the configured busy budget")
        || proposal.contains("elapsed never exceeds busy upper bound")
        || proposal.contains("Every case fails before\nwriter/read admission")
        || proposal.contains("production constructor snapshots only")
        || proposal.contains("three TASK-004 database environment keys are captured")
        || proposal.contains("it adds no unsafe code")
        || proposal.contains("`macos_fs_security_ffi.rs` module in `mengxia-store-sqlite`")
        || proposal.contains("The crate changes from `forbid(unsafe_code)`")
        || proposal.contains("serialization or raw-path conversion")
        || proposal.contains("test-only layered\nresolver fixture")
        || proposal.contains("cc = { version = \"=1.2.62\"")
        || proposal.contains("macro-injected deterministic fakes")
        || proposal.contains("conversion to an owned/raw path, persistence, copying")
        || proposal.contains("Compile-fail and architecture tests reject token construction")
        || proposal.contains("parent/root/file allow, deny or inherited ACL")
        || proposal.contains("ownership-disabled volume or extended ACL fail")
        || proposal.contains("opens that parent as a directory\nwith no-follow semantics")
        || proposal.contains("| 3 | root empty or lock only")
        || proposal.contains("| 8 | intent absent")
        || proposal.contains("| 19 | staging only")
        || proposal.contains("restart-visibility uncertain")
        || proposal.contains("intent present or absent")
        || proposal.contains("selected bundle and every\n  retained ancestor must be root-owned")
    {
        return Err("TASK-004 contract contains a superseded contract".to_owned());
    }

    let checklist = proposal
        .split("## 11. Canonical start-record inputs")
        .nth(1)
        .and_then(|section| section.split("```text").nth(1))
        .and_then(|section| section.split("```").next())
        .ok_or_else(|| "TASK-004 canonical start inputs are missing".to_owned())?;
    if checklist.contains("..") || checklist.contains('/') {
        return Err("TASK-004 checklist must enumerate full stable IDs".to_owned());
    }

    for required in [
        "TASK-004",
        "TASK-002",
        "BASE-011",
        "BASE-013",
        "BASE-014",
        "BASE-015",
        "BASE-017",
        "DEC-017",
        "DEC-020",
        "DEC-021",
        "DEC-022",
        "ADR-0001",
        "ADR-0003",
        "ADR-0004",
        "ADR-0005",
        "ADR-0006",
        "FUNC-001",
        "DATA-001",
        "DATA-005",
        "DATA-006",
        "DATA-007",
        "DATA-011",
        "REL-001",
        "SEC-017",
        "SEC-020",
        "SEC-021",
        "CFG-001",
        "CFG-003",
        "AC-065",
        "AC-066",
        "AC-067",
        "AC-068",
        "AC-069",
        "AC-070",
        "AC-071",
        "AC-072",
        "AC-073",
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
        if !extract_ids(checklist).iter().any(|id| id == required) {
            return Err(format!("TASK-004 checklist is missing full ID {required}"));
        }
    }

    let start = plan
        .split("### TASK-004 start record")
        .nth(1)
        .and_then(|section| section.split("```text").nth(1))
        .and_then(|section| section.split("```").next())
        .ok_or_else(|| "TASK-004 canonical start record is missing".to_owned())?;
    if start.contains("..") {
        return Err("TASK-004 start record must enumerate full stable IDs".to_owned());
    }
    validate_references(start, definitions)?;
    for id in extract_ids(checklist) {
        if namespace(&id).is_some() && !extract_ids(start).iter().any(|active| active == &id) {
            return Err(format!(
                "TASK-004 start record is missing accepted input {id}"
            ));
        }
    }

    if task_done {
        let completion = plan
            .split("### TASK-004 completion record")
            .nth(1)
            .and_then(|section| section.split("## 6. Phases and gates").next())
            .ok_or_else(|| "DONE TASK-004 lacks a completion record".to_owned())?;
        for required in [
            "AC-065",
            "AC-066",
            "AC-067",
            "AC-068",
            "AC-069",
            "AC-070",
            "AC-071",
            "AC-072",
            "AC-073",
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
            "SEC-017",
            "SEC-020",
            "SEC-021",
        ] {
            if !completion.contains(&format!("`{required}`: `PASS`")) {
                return Err(format!("DONE TASK-004 lacks PASS evidence for {required}"));
            }
        }
    }

    Ok(())
}

fn extract_ids(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut ids = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index].is_ascii_uppercase()
            && (index == 0 || !is_id_character(bytes[index.saturating_sub(1)]))
        {
            let start = index;
            while index < bytes.len() && is_id_character(bytes[index]) {
                index += 1;
            }
            if let Ok(candidate) = std::str::from_utf8(&bytes[start..index])
                && let Some(id) = parse_exact_id(candidate)
            {
                ids.push(id);
            }
        } else {
            index += 1;
        }
    }
    ids
}

fn is_id_character(byte: u8) -> bool {
    byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-'
}

fn parse_exact_id(candidate: &str) -> Option<String> {
    let candidate = candidate.trim_matches('-');
    let (prefix, number) = candidate.rsplit_once('-')?;
    if !(3..=4).contains(&number.len()) || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if prefix
        .split('-')
        .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_uppercase()))
    {
        return None;
    }
    let first_namespace = prefix.split('-').next()?;
    NAMESPACES
        .contains(&first_namespace)
        .then(|| candidate.to_owned())
}

fn namespace(id: &str) -> Option<&str> {
    id.split('-').next()
}

fn split_id(id: &str) -> Result<(&str, u32), String> {
    let (prefix, number) = id
        .rsplit_once('-')
        .ok_or_else(|| format!("malformed stable ID {id}"))?;
    let number = number
        .parse()
        .map_err(|_| format!("malformed numeric suffix in {id}"))?;
    Ok((prefix, number))
}
