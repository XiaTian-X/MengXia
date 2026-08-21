mod support;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use support::{cargo_metadata, parse_packages, workspace_root};

const EXPECTED_PACKAGES: &[&str] = &[
    "mengxia",
    "mengxia-app",
    "mengxia-core-proto",
    "mengxia-domain",
    "mengxia-events",
    "mengxia-framing",
    "mengxia-platform-sandbox",
    "mengxia-plugin-host",
    "mengxia-plugin-package",
    "mengxia-plugin-proto",
    "mengxia-plugin-security",
    "mengxia-ports",
    "mengxia-storage-local",
    "mengxia-store-sqlite",
    "mengxia-testkit",
    "mengxia-types",
    "mengxiad",
];

#[test]
fn package_and_binary_inventory_is_canonical() {
    let root = workspace_root();
    let output = cargo_metadata(&root.join("Cargo.toml"), true);
    assert!(
        output.status.success(),
        "locked metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packages = parse_packages(&String::from_utf8(output.stdout).expect("metadata is UTF-8"))
        .expect("metadata package JSON is valid");
    let actual: BTreeSet<&str> = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    let expected: BTreeSet<&str> = EXPECTED_PACKAGES.iter().copied().collect();
    assert_eq!(actual, expected, "workspace package inventory drifted");
}

#[test]
fn repository_candidate_inventory_has_no_environment_or_generated_files() {
    let root = workspace_root();
    let inventory = repository_candidate_inventory(&root);
    let forbidden = inventory.lines().filter(|path| is_unintended(path));
    let forbidden: Vec<_> = forbidden.collect();
    assert!(
        forbidden.is_empty(),
        "unintended generated/environment files: {forbidden:?}"
    );
}

#[test]
fn repository_candidate_inventory_uses_only_approved_project_paths() {
    let root = workspace_root();
    let inventory = repository_candidate_inventory(&root);
    let unexpected: Vec<_> = inventory
        .lines()
        .filter(|path| !is_approved_project_path(path))
        .collect();
    assert!(
        unexpected.is_empty(),
        "candidate inventory contains files outside code/document/config paths: {unexpected:?}"
    );

    for allowed in [
        "Cargo.toml",
        "README.md",
        "docs/spec/example.md",
        "crates/example/src/lib.rs",
        "proto/core/v1/service.proto",
        ".github/workflows/ci.yml",
    ] {
        assert!(is_approved_project_path(allowed), "must allow {allowed}");
    }
    for rejected in ["local-notes.txt", "screenshot.png", "downloads/archive.zip"] {
        assert!(
            !is_approved_project_path(rejected),
            "must reject unrelated candidate path {rejected}"
        );
    }
}

#[test]
fn ignore_rules_cover_common_generated_environment_and_editor_files() {
    let root = workspace_root();
    let paths = [
        "target/debug/mengxia",
        "crates/example/target/debug/example",
        ".DS_Store",
        "docs/.DS_Store",
        ".idea/workspace.xml",
        ".vscode/settings.json",
        ".fleet/settings.json",
        "debug.log",
        "coverage/lcov.info",
        ".env.local",
        ".direnv/allow",
        ".cache/tool/state",
        "scratch.tmp",
        "src/lib.rs.bk",
        "node_modules/tool/index.js",
        ".venv/bin/python",
        "__pycache__/tool.cpython-398.pyc",
    ];
    for ignored_path in paths {
        let output = Command::new("git")
            .args(["check-ignore", "--no-index", "--quiet", "--", ignored_path])
            .current_dir(&root)
            .output()
            .expect("git check-ignore must start");
        assert!(
            output.status.success(),
            "required path is not ignored: {ignored_path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    for allowed_example in [".env.example", ".env.test.example"] {
        let output = Command::new("git")
            .args([
                "check-ignore",
                "--no-index",
                "--quiet",
                "--",
                allowed_example,
            ])
            .current_dir(&root)
            .output()
            .expect("git check-ignore must start");
        assert_eq!(
            output.status.code(),
            Some(1),
            "documented environment example must remain committable: {allowed_example}"
        );
    }
}

fn is_unintended(path: &str) -> bool {
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let has_component = |component: &str| path.split('/').any(|part| part == component);
    [
        "target",
        ".idea",
        ".vscode",
        ".fleet",
        ".direnv",
        ".cache",
        "coverage",
        "test-results",
        "tmp",
        "temp",
        "node_modules",
        ".venv",
        "venv",
        "__pycache__",
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
    ]
    .iter()
    .any(|component| has_component(component))
        || file_name == ".DS_Store"
        || file_name == "Thumbs.db"
        || file_name == "Desktop.ini"
        || file_name.starts_with("._")
        || ((file_name == ".env" || file_name.starts_with(".env."))
            && !file_name.ends_with(".example"))
        || file_name == ".envrc"
        || file_name == "lcov.info"
        || file_name == "tarpaulin-report.html"
        || file_name == "junit.xml"
        || [
            ".log",
            ".swp",
            ".swo",
            ".tmp",
            ".temp",
            ".bak",
            ".orig",
            ".rej",
            ".pid",
            ".rs.bk",
            ".pyc",
            ".pyo",
            ".profraw",
            ".profdata",
        ]
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
}

fn repository_candidate_inventory(root: &Path) -> String {
    let output = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(root)
        .output()
        .expect("git ls-files must start");
    assert!(
        output.status.success(),
        "git inventory failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("Git paths are UTF-8")
}

fn is_approved_project_path(path: &str) -> bool {
    if let Some((top_level, _)) = path.split_once('/') {
        return [
            ".github",
            "bins",
            "crates",
            "docs",
            "integrations",
            "migrations",
            "plugins",
            "proto",
            "schemas",
            "scripts",
            "tests",
        ]
        .contains(&top_level);
    }

    matches!(
        path,
        ".editorconfig"
            | ".gitattributes"
            | ".gitignore"
            | ".mailmap"
            | "AGENTS.md"
            | "Cargo.lock"
            | "Cargo.toml"
            | "CHANGELOG.md"
            | "CODE_OF_CONDUCT.md"
            | "CONTRIBUTING.md"
            | "Makefile"
            | "README.md"
            | "SECURITY.md"
            | "clippy.toml"
            | "deny.toml"
            | "justfile"
            | "rust-toolchain.toml"
            | "rustfmt.toml"
    ) || path == "LICENSE"
        || path.starts_with("LICENSE-")
}
