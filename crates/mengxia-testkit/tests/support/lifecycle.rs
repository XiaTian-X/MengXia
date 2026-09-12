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
                _ => return Err("unknown lifecycle section".into()),
            };
            if !sections.insert(section) {
                return Err("duplicate lifecycle section".into());
            }
            continue;
        }
        let (key, value) = line.split_once('=').ok_or("missing scalar assignment")?;
        let key = format!("{section}.{}", key.trim());
        if !allowed.contains(&key.as_str()) {
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
    if result.len() != allowed.len() {
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
    Ok(result)
}
