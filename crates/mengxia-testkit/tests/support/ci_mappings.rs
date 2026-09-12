use std::collections::BTreeMap;

// Only the two explicit top-level forms; not a shell evaluator or automatic
// inference of equivalent commands. Comments never count as executable maps.
pub fn parse(script: &str) -> Result<BTreeMap<String, String>, String> {
    let mut mappings = BTreeMap::new();
    for line in script.lines() {
        let (ids, command) = if let Some(tail) = line.strip_prefix("run ") {
            tail.split_once(' ').ok_or("missing mapped command")?
        } else if let Some(tail) = line.strip_prefix("ci_run_group '") {
            tail.split_once("' ").ok_or("malformed group mapping")?
        } else {
            continue;
        };
        if ids.trim().is_empty()
            || command.trim().is_empty()
            || command.trim_start().starts_with('#')
            || command.chars().any(|c| matches!(c, ';' | '|' | '&' | '`'))
        {
            return Err("empty or nonliteral mapped command".into());
        }
        for id in ids.split_whitespace() {
            if !id.starts_with("TEST-")
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
            {
                return Err("invalid mapped identifier".into());
            }
            if mappings.insert(id.to_owned(), command.to_owned()).is_some() {
                return Err(format!("duplicate mapping for {id}"));
            }
        }
    }
    Ok(mappings)
}
