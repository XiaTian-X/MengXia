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

    let decisions = documents
        .iter()
        .find(|document| document.path.ends_with("DECISIONS.md"))
        .expect("decisions document is present");
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

    let invalid_task_004_contract = "# TASK-004 accepted implementation contract\n\
        > Status: **ACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION v1.1.12**\n\
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
            (
                specification,
                "repository_state: \"TASK_001_AND_TASK_002_DONE; TASK_004_IMPLEMENTED_LOCAL_GATES_PASS_CI_ATTESTATION_PENDING\"",
            ),
            (review, "`TASK-002 DONE`"),
            (intake, "TASK-001/TASK-002 已完成"),
            (agents, "TASK-001/TASK-002 已完成"),
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

fn validate_task_004_active_contract(
    plan: &str,
    proposal: &str,
    definitions: &BTreeMap<String, PathBuf>,
) -> Result<(), String> {
    for marker in [
        "Status: **ACCEPTED / INCORPORATED BY CANONICAL SPECIFICATION v1.1.12**",
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
        "7def90dd8829726686213a747fc5bff1583df933dae5edc55d755479e0bfe00a",
        "9511f84f0abe1e108e10979900d4fea8567534aef78f0984f7050c49f6c29ff7",
        "229eb9d8027953d2aee0590f983eed587d52bdd1ebc21114a62ce693f77b03f1",
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

    if !plan.contains("| `TASK-004` SQLite/migration engine | `IN_PROGRESS` |") {
        return Err("accepted TASK-004 must be lifecycle-active as IN_PROGRESS".to_owned());
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
