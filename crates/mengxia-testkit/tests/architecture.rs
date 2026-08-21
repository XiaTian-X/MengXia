mod support;

use std::collections::{HashMap, HashSet};
use std::fs;

use support::{Package, cargo_metadata, parse_packages, workspace_root};

const PURE_CRATES: &[&str] = &[
    "mengxia-types",
    "mengxia-domain",
    "mengxia-events",
    "mengxia-ports",
    "mengxia-app",
];

#[test]
fn allowed_workspace_graph_obeys_dependency_direction() {
    let root = workspace_root();
    let output = cargo_metadata(&root.join("Cargo.toml"), true);
    assert!(
        output.status.success(),
        "locked workspace metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packages = parse_packages(&String::from_utf8(output.stdout).expect("metadata is UTF-8"))
        .expect("metadata package JSON is valid");
    assert_graph_allowed(&packages).expect("declared workspace graph must obey Specification §5.3");

    for crate_name in PURE_CRATES {
        let package = packages
            .iter()
            .find(|package| package.name == *crate_name)
            .unwrap_or_else(|| panic!("missing pure crate {crate_name}"));
        let source = fs::read_to_string(
            std::path::Path::new(&package.manifest_path)
                .parent()
                .expect("manifest has a parent")
                .join("src/lib.rs"),
        )
        .expect("pure crate source must be readable");
        assert!(
            source.contains("#![forbid(unsafe_code)]"),
            "{crate_name} must explicitly forbid unsafe code"
        );
    }
}

#[test]
fn representative_forbidden_edge_is_rejected() {
    let root = workspace_root();
    let fixture =
        root.join("crates/mengxia-testkit/tests/fixtures/architecture/forbidden-edge/Cargo.toml");
    let output = cargo_metadata(&fixture, true);
    assert!(
        output.status.success(),
        "negative fixture metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let packages = parse_packages(&String::from_utf8(output.stdout).expect("metadata is UTF-8"))
        .expect("fixture metadata package JSON is valid");
    let violation = assert_graph_allowed(&packages).expect_err("domain -> application must fail");
    assert!(
        violation.contains("mengxia-domain-fixture -> mengxia-app-fixture"),
        "unexpected violation: {violation}"
    );
}

#[test]
fn explicit_forbidden_infrastructure_edges_are_rejected() {
    let events_with_domain = Package {
        name: "mengxia-events".to_owned(),
        dependencies: vec!["mengxia-domain".to_owned()],
        manifest_path: "fixture/Cargo.toml".to_owned(),
    };
    let domain = Package {
        name: "mengxia-domain".to_owned(),
        dependencies: Vec::new(),
        manifest_path: "fixture/Cargo.toml".to_owned(),
    };
    let violation = assert_graph_allowed(&[events_with_domain, domain])
        .expect_err("events -> domain must be rejected to keep domain -> events possible");
    assert!(violation.contains("events must not depend on the domain layer"));

    let domain_with_runtime = Package {
        name: "mengxia-domain".to_owned(),
        dependencies: vec!["tokio".to_owned()],
        manifest_path: "fixture/Cargo.toml".to_owned(),
    };
    let violation = assert_graph_allowed(&[domain_with_runtime])
        .expect_err("domain -> Tokio must be rejected even when Tokio is external");
    assert!(violation.contains("infrastructure-neutral"));

    let cli_with_store = Package {
        name: "mengxia".to_owned(),
        dependencies: vec!["mengxia-store-sqlite".to_owned()],
        manifest_path: "fixture/Cargo.toml".to_owned(),
    };
    let store = Package {
        name: "mengxia-store-sqlite".to_owned(),
        dependencies: Vec::new(),
        manifest_path: "fixture/Cargo.toml".to_owned(),
    };
    let violation = assert_graph_allowed(&[cli_with_store, store])
        .expect_err("CLI -> SQLite store must be rejected");
    assert!(violation.contains("concrete persistence"));
}

fn assert_graph_allowed(packages: &[Package]) -> Result<(), String> {
    let workspace_names: HashSet<&str> = packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    let ranks: HashMap<&str, u8> = packages
        .iter()
        .map(|package| architecture_rank(&package.name).map(|rank| (package.name.as_str(), rank)))
        .collect::<Result<_, _>>()?;

    for package in packages {
        let package_rank = ranks[package.name.as_str()];
        for dependency in &package.dependencies {
            if package.name.starts_with("mengxia-domain")
                && (matches!(
                    dependency.as_str(),
                    "tokio" | "rusqlite" | "prost" | "reqwest"
                ) || dependency.contains("provider")
                    || dependency.contains("-sdk"))
            {
                return Err(format!(
                    "domain must remain infrastructure-neutral: {} -> {}",
                    package.name, dependency
                ));
            }
            if package.name.starts_with("mengxia-events")
                && dependency.starts_with("mengxia-domain")
            {
                return Err(format!(
                    "events must not depend on the domain layer: {} -> {}",
                    package.name, dependency
                ));
            }
            if package.name.starts_with("mengxia-app")
                && dependency.contains("provider-")
                && dependency != "mengxia-ports"
            {
                return Err(format!(
                    "application must not depend on a provider adapter: {} -> {}",
                    package.name, dependency
                ));
            }
            if package.name == "mengxia"
                && matches!(
                    dependency.as_str(),
                    "rusqlite" | "mengxia-store-sqlite" | "mengxia-storage-local"
                )
            {
                return Err(format!(
                    "CLI must not depend on concrete persistence: {} -> {}",
                    package.name, dependency
                ));
            }
            if package.name.contains("provider")
                && matches!(
                    dependency.as_str(),
                    "mengxia-core-proto" | "mengxia-store-sqlite"
                )
            {
                return Err(format!(
                    "provider plugin must not receive Core or SQLite authority: {} -> {}",
                    package.name, dependency
                ));
            }
            if matches!(
                package.name.as_str(),
                "mengxia-plugin-package" | "mengxia-plugin-security"
            ) && !dependency.starts_with("mengxia-")
                && (dependency.contains("provider") || dependency.contains("-sdk"))
            {
                return Err(format!(
                    "plugin package/security must not depend on a provider SDK: {} -> {}",
                    package.name, dependency
                ));
            }
            if !workspace_names.contains(dependency.as_str()) {
                continue;
            }
            let dependency_rank = ranks[dependency.as_str()];
            if dependency_rank > package_rank {
                return Err(format!(
                    "forbidden dependency direction: {} -> {}",
                    package.name, dependency
                ));
            }
            if package.name == "mengxia-domain"
                && ["mengxia", "mengxiad"].contains(&dependency.as_str())
            {
                return Err(format!(
                    "domain must not depend on a composition root: {} -> {}",
                    package.name, dependency
                ));
            }
        }
    }
    Ok(())
}

fn architecture_rank(name: &str) -> Result<u8, String> {
    if name.starts_with("mengxia-types")
        || name.starts_with("mengxia-domain")
        || name.starts_with("mengxia-events")
    {
        Ok(0)
    } else if name.starts_with("mengxia-ports") {
        Ok(1)
    } else if name.starts_with("mengxia-app") {
        Ok(2)
    } else if name == "mengxia" || name == "mengxiad" {
        Ok(4)
    } else if name.starts_with("mengxia-") {
        Ok(3)
    } else {
        Err(format!("non-canonical workspace package name: {name}"))
    }
}
