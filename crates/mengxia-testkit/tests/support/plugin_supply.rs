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

    validate_task_011_lock_inventory(
        &fs::read_to_string(root.join("Cargo.lock")).map_err(display)?,
        &fs::read_to_string(
            root.join("crates/mengxia-testkit/tests/fixtures/task_011/Cargo.task-010.lock"),
        )
        .map_err(display)?,
    )
}

pub fn validate_task_011_lock_inventory(current: &str, historical: &str) -> Result<(), String> {
    let current = third_party_lock_blocks(current);
    let mut expected = third_party_lock_blocks(historical);
    // Preserve the immutable TASK-010 evidence. The only later accepted delta is
    // RUSTSEC-2026-0386: Tokio 1.53.1 -> 1.53.3, with the same dependency edges.
    // See docs/development/tokio-security-update-2026-10-30.md.
    let old = expected
        .iter()
        .find(|block| block.starts_with("name = \"tokio\"\n"))
        .ok_or("historical Tokio record missing")?
        .clone();
    let old_version = "\nversion = \"1.53.1\"\n";
    let old_checksum =
        "\nchecksum = \"202caea871b69668250d242070849eb495be178ed697a3e98aebce5bc81a0bed\"\n";
    if old.matches(old_version).count() != 1 || old.matches(old_checksum).count() != 1 {
        return Err("historical Tokio identity changed".to_owned());
    }
    let patched = old
        .replace(old_version, "\nversion = \"1.53.3\"\n")
        .replace(
            old_checksum,
            "\nchecksum = \"ce3335fa71841cda333a58d7615b03901380ecf09d59b3296d21f8bbac0dde4e\"\n",
        );
    expected.remove(&old);
    expected.insert(patched);
    if current != expected {
        return Err(
            "third-party inventory differs from TASK-011 plus the exact Tokio security update"
                .to_owned(),
        );
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
