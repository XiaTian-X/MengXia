use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

pub fn validate_task_011_supply(root: &Path) -> Result<(), String> {
    let root_manifest = fs::read_to_string(root.join("Cargo.toml")).map_err(display)?;
    let host =
        fs::read_to_string(root.join("crates/mengxia-plugin-host/Cargo.toml")).map_err(display)?;
    let proto =
        fs::read_to_string(root.join("crates/mengxia-plugin-proto/Cargo.toml")).map_err(display)?;
    let testkit =
        fs::read_to_string(root.join("crates/mengxia-testkit/Cargo.toml")).map_err(display)?;

    for dependency in [
        "mengxia-plugin-host",
        "mengxia-plugin-package",
        "mengxia-plugin-proto",
    ] {
        let marker =
            format!("{dependency} = {{ path = \"crates/{dependency}\", version = \"=0.1.0\" }}");
        require_once(&root_manifest, &marker)?;
    }
    for marker in [
        "mengxia-plugin-package.workspace = true",
        "mengxia-plugin-proto.workspace = true",
        "mengxia-types.workspace = true",
        "tokio.workspace = true",
    ] {
        require_once(&host, marker)?;
    }
    for forbidden in [
        "mengxia-app",
        "mengxia-ports",
        "rusqlite",
        "serde",
        "std::process",
    ] {
        if host.contains(forbidden) {
            return Err(format!("host contains forbidden edge: {forbidden}"));
        }
    }
    for marker in [
        "mengxia-framing.workspace = true",
        "prost.workspace = true",
        "tokio.workspace = true",
        "prost-build.workspace = true",
        "prost-types.workspace = true",
        "sha2.workspace = true",
    ] {
        if !proto.contains(marker) {
            return Err(format!("proto edge missing: {marker}"));
        }
    }
    for marker in [
        "mengxia-plugin-host.workspace = true",
        "mengxia-plugin-proto.workspace = true",
        "prost.workspace = true",
        "prost-types.workspace = true",
        "tokio.workspace = true",
    ] {
        if !testkit.contains(marker) {
            return Err(format!("testkit dev edge missing: {marker}"));
        }
    }

    let current =
        third_party_lock_blocks(&fs::read_to_string(root.join("Cargo.lock")).map_err(display)?);
    let historical = third_party_lock_blocks(
        &fs::read_to_string(
            root.join("crates/mengxia-testkit/tests/fixtures/task_011/Cargo.task-010.lock"),
        )
        .map_err(display)?,
    );
    if current != historical {
        return Err("TASK-011 introduced a third-party package inventory change".to_owned());
    }
    Ok(())
}

fn require_once(text: &str, marker: &str) -> Result<(), String> {
    if text.matches(marker).count() != 1 {
        return Err(format!("expected exactly one manifest edge: {marker}"));
    }
    Ok(())
}

fn third_party_lock_blocks(lock: &str) -> BTreeSet<String> {
    lock.split("[[package]]")
        .skip(1)
        .filter(|block| block.lines().any(|line| line.starts_with("source = ")))
        .map(|block| block.trim().to_owned())
        .collect()
}

fn display(error: std::io::Error) -> String {
    error.to_string()
}
