use std::collections::BTreeMap;

// Closed subset: sections and quoted ASCII scalar values; no escapes, arrays,
// commands, paths or executable conditions. Deliberately no TOML dependency.
pub fn parse(text: &str) -> Result<BTreeMap<String, String>, String> {
    if text.len() > 4096 || !text.is_ascii() {
        return Err("lifecycle record is oversized or non-ASCII".into());
    }
    let allowed = [
        "versions.specification",
        "versions.decisions",
        "versions.review",
        "versions.plan",
        "versions.intake",
        "maintenance.id",
        "maintenance.status",
        "maintenance.authority",
        "maintenance.pr_head",
        "maintenance.pr_run",
        "maintenance.main_head",
        "maintenance.main_run",
    ];
    let reviewed_fields = [
        "id",
        "owner",
        "status",
        "authority",
        "product_authority",
        "gate",
        "task010",
        "task011",
        "features",
        "requirements",
        "acceptance_contribution",
        "test",
        "parent_completion",
        "local_evidence",
        "pr_head",
        "pr_run",
        "main_head",
        "main_run",
    ];
    let mut section = "";
    let mut sections = std::collections::BTreeSet::new();
    let mut result = BTreeMap::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            section = match line {
                "[versions]" => "versions",
                "[maintenance]" => "maintenance",
                "[reviewed_native_foundation]" => "reviewed_native_foundation",
                _ => return Err("unknown lifecycle section".into()),
            };
            if !sections.insert(section) {
                return Err("duplicate lifecycle section".into());
            }
            continue;
        }
        let (key, value) = line.split_once('=').ok_or("missing scalar assignment")?;
        let key = format!("{section}.{}", key.trim());
        let reviewed_key = key.strip_prefix("reviewed_native_foundation.");
        if !allowed.contains(&key.as_str())
            && !reviewed_key.is_some_and(|key| reviewed_fields.contains(&key))
        {
            return Err(format!("unknown lifecycle field {key}"));
        }
        let value = value
            .trim()
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .ok_or("lifecycle value must be quoted")?;
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err("invalid lifecycle scalar".into());
        }
        if result.insert(key, value.to_owned()).is_some() {
            return Err("duplicate lifecycle field".into());
        }
    }
    let has_reviewed = sections.contains("reviewed_native_foundation");
    let expected_len = allowed.len()
        + if has_reviewed {
            reviewed_fields.len()
        } else {
            0
        };
    if result.len() != expected_len || allowed.iter().any(|key| !result.contains_key(*key)) {
        return Err("missing lifecycle fields".into());
    }
    for key in &allowed[..5] {
        let parts: Vec<_> = result[*key].split('.').collect();
        if parts.len() != 3
            || parts.iter().any(|p| {
                p.is_empty()
                    || !p.bytes().all(|b| b.is_ascii_digit())
                    || (p.len() > 1 && p.starts_with('0'))
            })
        {
            return Err("invalid document version".into());
        }
    }
    if result["maintenance.id"] != "MAINT-002" {
        return Err("wrong maintenance identity".into());
    }
    let expected_authority = match result["maintenance.status"].as_str() {
        "PENDING" | "DONE" => "NONE",
        "IN_PROGRESS" => "MAINT_002_CI_ONLY",
        _ => return Err("invalid maintenance status".into()),
    };
    if result["maintenance.authority"] != expected_authority {
        return Err("over-broad or stale authority".into());
    }
    for key in ["maintenance.pr_head", "maintenance.main_head"] {
        let v = &result[key];
        if v == "PENDING" && result["maintenance.status"] != "DONE" {
            continue;
        }
        if v.len() != 40
            || !v
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("missing or invalid evidence commit".into());
        }
    }
    for key in ["maintenance.pr_run", "maintenance.main_run"] {
        let v = &result[key];
        if v == "PENDING" && result["maintenance.status"] != "DONE" {
            continue;
        }
        if v.starts_with('0') || v.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
            return Err("missing or invalid evidence run".into());
        }
    }
    if has_reviewed {
        validate_reviewed_foundation(&result)?;
    }
    Ok(result)
}

fn validate_reviewed_foundation(record: &BTreeMap<String, String>) -> Result<(), String> {
    let value = |key: &str| {
        record
            .get(&format!("reviewed_native_foundation.{key}"))
            .map(String::as_str)
            .ok_or_else(|| format!("missing reviewed foundation {key}"))
    };
    for (key, expected) in [
        ("id", "REVIEWED_NATIVE_FOUNDATION"),
        ("owner", "TASK-012"),
        ("product_authority", "NONE"),
        ("gate", "ACCEPTED"),
        ("task010", "DONE"),
        ("task011", "DONE"),
        ("features", "FUNC-006.FUNC-007"),
        (
            "requirements",
            "SEC-003.SEC-008.SEC-010.SEC-016.SEC-017.SEC-022",
        ),
        ("acceptance_contribution", "AC-104.AC-105.AC-106"),
        ("test", "TEST-REVIEWED-CONTRACT-001"),
        ("parent_completion", "NOT_CLAIMED"),
    ] {
        if value(key)? != expected {
            return Err(format!("reviewed foundation {key} must be {expected}"));
        }
    }
    let done = match value("status")? {
        "IN_PROGRESS" => false,
        "DONE" => true,
        _ => return Err("invalid reviewed foundation status".into()),
    };
    let authority = if done {
        "NONE"
    } else {
        "REVIEWED_NATIVE_FOUNDATION_ONLY"
    };
    if value("authority")? != authority {
        return Err("reviewed foundation authority does not match lifecycle".into());
    }
    match value("local_evidence")? {
        "LOCAL_PASS" => {}
        "PENDING" if !done => {}
        _ => return Err("reviewed foundation local evidence missing".into()),
    }
    for key in ["pr_head", "main_head"] {
        let digest = value(key)?;
        if digest == "PENDING" && !done {
            continue;
        }
        if digest.len() != 40
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("reviewed foundation needs an exact evidence commit".into());
        }
    }
    for key in ["pr_run", "main_run"] {
        let run = value(key)?;
        if run == "PENDING" && !done {
            continue;
        }
        if run.starts_with('0') || run.parse::<u64>().ok().filter(|n| *n > 0).is_none() {
            return Err("reviewed foundation needs a positive evidence run".into());
        }
    }
    // These fields reference reviewed evidence; syntax alone cannot attest a CI run.
    Ok(())
}
