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
    "mengxia-plugin-package",
    "mengxia-plugin-security",
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
        if matches!(
            *crate_name,
            "mengxia-plugin-package" | "mengxia-plugin-security"
        ) {
            let source_directory = std::path::Path::new(&package.manifest_path)
                .parent()
                .expect("manifest has a parent")
                .join("src");
            for entry in fs::read_dir(source_directory).expect("plugin source directory") {
                let path = entry.expect("plugin source entry").path();
                if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                    let source = fs::read_to_string(&path).expect("plugin source is readable");
                    assert_task_010_source_boundary(crate_name, &source)
                        .unwrap_or_else(|violation| panic!("{}: {violation}", path.display()));
                }
            }
        }
    }
}

#[test]
fn task_011_private_protocol_boundary_has_no_process_or_core_authority() {
    let root = workspace_root();
    let host_manifest =
        fs::read_to_string(root.join("crates/mengxia-plugin-host/Cargo.toml")).unwrap();
    let proto_manifest =
        fs::read_to_string(root.join("crates/mengxia-plugin-proto/Cargo.toml")).unwrap();
    for required in [
        "mengxia-plugin-package.workspace = true",
        "mengxia-plugin-proto.workspace = true",
        "mengxia-types.workspace = true",
        "tokio.workspace = true",
    ] {
        assert_eq!(host_manifest.matches(required).count(), 1);
    }
    for forbidden in [
        "mengxia-app",
        "mengxia-ports",
        "mengxia-core-proto",
        "mengxia-store-sqlite",
        "mengxia-storage-local",
        "mengxia-platform-fs",
        "mengxia-platform-sandbox",
        "rusqlite",
    ] {
        assert!(!host_manifest.contains(forbidden));
        assert!(!proto_manifest.contains(forbidden));
    }

    for directory in [
        root.join("crates/mengxia-plugin-host/src"),
        root.join("crates/mengxia-plugin-proto/src"),
    ] {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|value| value.to_str()) != Some("rs") {
                continue;
            }
            let source = fs::read_to_string(&path).unwrap();
            for forbidden in [
                "std::process",
                "tokio::process",
                "std::net::Tcp",
                "tokio::net::Tcp",
                "std::fs::",
                "Command::new",
                "rusqlite::",
                "mengxia_core_proto",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "{} contains unauthorized symbol {forbidden}",
                    path.display()
                );
            }
        }
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
fn broker_foundation_preserves_purity_and_private_non_authority_values() {
    fn validate(source: &str) -> Result<(), &'static str> {
        for forbidden in [
            "std::fs",
            "std::net",
            "std::process",
            "std::thread",
            "std::time",
            "std::env",
            "tokio::",
            "SystemTime",
            "getrandom",
            "try_new(",
            "Vec<",
            "Vec::",
            "String",
            "Box<",
            "Box::",
            "format!(",
            ".to_owned()",
            "unsafe",
            "serde",
            "Serialize",
            "Deserialize",
            "impl From<",
            "impl Into<",
        ] {
            if source.contains(forbidden) {
                return Err("Broker foundation effect/allocation/authority escape");
            }
        }
        for name in ["BrokerReadCandidate", "BrokerReadAuditCandidate"] {
            let body = source
                .split_once(&format!("pub struct {name} {{"))
                .ok_or("missing candidate")?
                .1
                .split_once('}')
                .ok_or("unclosed candidate")?
                .0;
            if body.contains("pub ") {
                return Err("candidate fields must stay private");
            }
        }
        for required in [
            "pub fn evaluate_broker_read",
            "fn assess",
            "BrokerMemberOrdinal",
            "PluginTrustDenied",
            "(REDACTED)",
        ] {
            if !source.contains(required) {
                return Err("missing Broker boundary");
            }
        }
        Ok(())
    }
    let source = fs::read_to_string(
        workspace_root().join("crates/mengxia-plugin-security/src/broker_foundation.rs"),
    )
    .unwrap();
    validate(&source).unwrap();
    for extra in [
        "std::fs",
        "getrandom",
        "Vec::new",
        "impl Into<CapabilityLease>",
    ] {
        assert!(validate(&format!("{source}\n{extra}")).is_err());
    }
    for name in ["BrokerReadCandidate", "BrokerReadAuditCandidate"] {
        let changed = source.replace(
            &format!("pub struct {name} {{"),
            &format!("pub struct {name} {{ pub injected: u64,"),
        );
        assert_ne!(changed, source);
        assert!(validate(&changed).is_err());
    }
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

    let package_with_runtime = Package {
        name: "mengxia-plugin-package".to_owned(),
        dependencies: vec!["tokio".to_owned()],
        manifest_path: "fixture/Cargo.toml".to_owned(),
    };
    let violation = assert_graph_allowed(&[package_with_runtime])
        .expect_err("package foundation -> runtime must be rejected");
    assert!(violation.contains("runtime and infrastructure free"));

    let package_with_security = Package {
        name: "mengxia-plugin-package".to_owned(),
        dependencies: vec!["mengxia-plugin-security".to_owned()],
        manifest_path: "fixture/Cargo.toml".to_owned(),
    };
    let security = Package {
        name: "mengxia-plugin-security".to_owned(),
        dependencies: Vec::new(),
        manifest_path: "fixture/Cargo.toml".to_owned(),
    };
    let violation = assert_graph_allowed(&[package_with_security, security])
        .expect_err("package -> security reverse edge must be rejected");
    assert!(violation.contains("plugin package may depend only on shared values"));

    for forbidden_source in [
        "use std::fs;",
        "use std::net::TcpStream;",
        "use std::process::Command;",
        "use std::path::PathBuf;",
        "use std::env;",
        "tokio::spawn(async {})",
        "rusqlite::Connection::open_in_memory()",
    ] {
        let violation =
            assert_task_010_source_boundary("mengxia-plugin-package-fixture", forbidden_source)
                .expect_err("TASK-010 runtime or authority-bearing source must be rejected");
        assert!(violation.contains("pure in-memory source boundary"));
    }
}

fn assert_task_010_source_boundary(crate_name: &str, source: &str) -> Result<(), String> {
    for forbidden in [
        "std::fs",
        "std::net",
        "std::process",
        "std::path",
        "std::env",
        "std::thread",
        "tokio::",
        "rusqlite::",
        "rustix::",
        "prost::",
        "reqwest::",
        "extern \"C\"",
    ] {
        if source.contains(forbidden) {
            return Err(format!(
                "TASK-010 package/security must retain a pure in-memory source boundary: {crate_name} contains {forbidden}"
            ));
        }
    }
    Ok(())
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
            ) && matches!(
                dependency.as_str(),
                "tokio" | "rusqlite" | "rustix" | "prost" | "reqwest"
            ) {
                return Err(format!(
                    "plugin package/security must remain runtime and infrastructure free: {} -> {}",
                    package.name, dependency
                ));
            }
            if package.name == "mengxia-plugin-package"
                && dependency.starts_with("mengxia-")
                && dependency != "mengxia-types"
            {
                return Err(format!(
                    "plugin package may depend only on shared values: {} -> {}",
                    package.name, dependency
                ));
            }
            if package.name == "mengxia-plugin-security"
                && dependency.starts_with("mengxia-")
                && !matches!(
                    dependency.as_str(),
                    "mengxia-plugin-package" | "mengxia-types"
                )
            {
                return Err(format!(
                    "plugin security may depend only on package and shared values: {} -> {}",
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
