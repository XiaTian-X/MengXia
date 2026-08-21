#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub dependencies: Vec<String>,
    pub manifest_path: String,
}

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("testkit must be nested under crates/")
        .to_path_buf()
}

pub fn cargo_metadata(manifest_path: &Path, locked: bool) -> Output {
    let mut command = Command::new(env!("CARGO"));
    command.args([
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--manifest-path",
    ]);
    command.arg(manifest_path);
    if locked {
        command.arg("--locked");
    }
    command.output().expect("cargo metadata must start")
}

pub fn parse_packages(metadata: &str) -> Result<Vec<Package>, String> {
    let packages = json_array_for_key(metadata, "packages")?;
    top_level_objects(packages)
        .into_iter()
        .map(|object| {
            let dependencies = json_array_for_key(object, "dependencies")?;
            let dependency_names = top_level_objects(dependencies)
                .into_iter()
                .map(|dependency| json_string_for_key(dependency, "name"))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Package {
                name: json_string_for_key(object, "name")?,
                dependencies: dependency_names,
                manifest_path: json_string_for_key(object, "manifest_path")?,
            })
        })
        .collect()
}

fn json_array_for_key<'a>(input: &'a str, key: &str) -> Result<&'a str, String> {
    let marker = format!("\"{key}\":");
    let marker_start = input
        .find(&marker)
        .ok_or_else(|| format!("missing JSON key {key}"))?;
    let array_start = input[marker_start + marker.len()..]
        .find('[')
        .map(|offset| marker_start + marker.len() + offset)
        .ok_or_else(|| format!("JSON key {key} is not an array"))?;
    let array_end = matching_delimiter(input, array_start, '[', ']')?;
    Ok(&input[array_start + 1..array_end])
}

fn json_string_for_key(input: &str, key: &str) -> Result<String, String> {
    let marker = format!("\"{key}\":");
    let marker_start = input
        .find(&marker)
        .ok_or_else(|| format!("missing JSON string key {key}"))?;
    let value = input[marker_start + marker.len()..].trim_start();
    if !value.starts_with('"') {
        return Err(format!("JSON key {key} is not a string"));
    }

    let mut escaped = false;
    let mut result = String::new();
    for character in value[1..].chars() {
        if escaped {
            match character {
                '"' | '\\' | '/' => result.push(character),
                'b' => result.push('\u{0008}'),
                'f' => result.push('\u{000c}'),
                'n' => result.push('\n'),
                'r' => result.push('\r'),
                't' => result.push('\t'),
                other => return Err(format!("unsupported JSON escape \\{other}")),
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Ok(result);
        } else {
            result.push(character);
        }
    }
    Err(format!("unterminated JSON string for {key}"))
}

fn top_level_objects(input: &str) -> Vec<&str> {
    let mut objects = Vec::new();
    let mut object_start = None;
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;

    for (index, character) in input.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
        } else if character == '{' {
            if depth == 0 {
                object_start = Some(index);
            }
            depth += 1;
        } else if character == '}' {
            depth -= 1;
            if depth == 0 {
                let start = object_start.expect("closing object must have a start");
                objects.push(&input[start..=index]);
                object_start = None;
            }
        }
    }
    objects
}

fn matching_delimiter(input: &str, start: usize, open: char, close: char) -> Result<usize, String> {
    let mut depth = 0_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, character) in input[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if character == '"' {
            in_string = true;
        } else if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Ok(start + offset);
            }
        }
    }
    Err(format!("unterminated {open}{close} JSON value"))
}
