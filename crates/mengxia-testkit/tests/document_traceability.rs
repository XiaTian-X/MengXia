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
