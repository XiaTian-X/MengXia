use std::cmp::Ordering;

use mengxia_plugin_package::{InspectedPluginPackage, RuntimeDependencyDeclaration};

/// Total fail-closed ordering of permission evidence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PermissionDiffClassification {
    Unchanged,
    Contraction,
    Expansion,
    IncomparableDeny,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IncomparableReason {
    None,
    PackageIdentityMismatch,
    ComparatorUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PermissionChangeNamespace {
    Permission,
    Capability,
    RuntimeDependency,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PermissionChangeDisposition {
    Added,
    Removed,
    Changed,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PermissionChange {
    namespace: PermissionChangeNamespace,
    identifier: Box<str>,
    disposition: PermissionChangeDisposition,
}

impl PermissionChange {
    #[must_use]
    pub const fn namespace(&self) -> PermissionChangeNamespace {
        self.namespace
    }

    #[must_use]
    pub const fn disposition(&self) -> PermissionChangeDisposition {
        self.disposition
    }

    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PermissionDiff {
    classification: PermissionDiffClassification,
    reason: IncomparableReason,
    changes: Box<[PermissionChange]>,
}

impl PermissionDiff {
    #[must_use]
    pub const fn classification(&self) -> PermissionDiffClassification {
        self.classification
    }

    #[must_use]
    pub const fn reason(&self) -> IncomparableReason {
        self.reason
    }

    #[must_use]
    pub fn changes(&self) -> &[PermissionChange] {
        &self.changes
    }
}

/// Compute bounded evidence only. No grant, state transition or side effect is possible.
#[must_use]
pub fn diff_permissions(
    old: &InspectedPluginPackage,
    candidate: &InspectedPluginPackage,
) -> PermissionDiff {
    if old.publisher() != candidate.publisher() || old.plugin_id() != candidate.plugin_id() {
        return PermissionDiff {
            classification: PermissionDiffClassification::IncomparableDeny,
            reason: IncomparableReason::PackageIdentityMismatch,
            changes: Box::new([]),
        };
    }

    let mut changes = Vec::with_capacity(193);
    diff_sorted(
        old.requested_permissions(),
        candidate.requested_permissions(),
        PermissionChangeNamespace::Permission,
        |value| value.canonical_identifier(),
        &mut changes,
    );
    diff_sorted(
        old.capabilities(),
        candidate.capabilities(),
        PermissionChangeNamespace::Capability,
        |value| value.as_str().to_owned(),
        &mut changes,
    );
    diff_dependencies(
        old.runtime_dependencies(),
        candidate.runtime_dependencies(),
        &mut changes,
    );
    changes.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then_with(|| left.identifier.cmp(&right.identifier))
            .then_with(|| left.disposition.cmp(&right.disposition))
    });

    let classification = if changes.iter().any(|change| {
        change.disposition == PermissionChangeDisposition::Added
            || change.disposition == PermissionChangeDisposition::Changed
    }) {
        PermissionDiffClassification::Expansion
    } else if changes.is_empty() {
        PermissionDiffClassification::Unchanged
    } else {
        PermissionDiffClassification::Contraction
    };
    PermissionDiff {
        classification,
        reason: IncomparableReason::None,
        changes: changes.into_boxed_slice(),
    }
}

fn diff_sorted<T: Ord>(
    old: &[T],
    candidate: &[T],
    namespace: PermissionChangeNamespace,
    identifier: impl Fn(&T) -> String,
    changes: &mut Vec<PermissionChange>,
) {
    let (mut old_index, mut candidate_index) = (0, 0);
    while old_index < old.len() || candidate_index < candidate.len() {
        match (old.get(old_index), candidate.get(candidate_index)) {
            (Some(left), Some(right)) => match left.cmp(right) {
                Ordering::Less => {
                    push_change(
                        changes,
                        namespace,
                        PermissionChangeDisposition::Removed,
                        identifier(left),
                    );
                    old_index += 1;
                }
                Ordering::Equal => {
                    old_index += 1;
                    candidate_index += 1;
                }
                Ordering::Greater => {
                    push_change(
                        changes,
                        namespace,
                        PermissionChangeDisposition::Added,
                        identifier(right),
                    );
                    candidate_index += 1;
                }
            },
            (Some(left), None) => {
                push_change(
                    changes,
                    namespace,
                    PermissionChangeDisposition::Removed,
                    identifier(left),
                );
                old_index += 1;
            }
            (None, Some(right)) => {
                push_change(
                    changes,
                    namespace,
                    PermissionChangeDisposition::Added,
                    identifier(right),
                );
                candidate_index += 1;
            }
            (None, None) => break,
        }
    }
}

fn diff_dependencies(
    old: &[RuntimeDependencyDeclaration],
    candidate: &[RuntimeDependencyDeclaration],
    changes: &mut Vec<PermissionChange>,
) {
    let (mut old_index, mut candidate_index) = (0, 0);
    while old_index < old.len() || candidate_index < candidate.len() {
        match (old.get(old_index), candidate.get(candidate_index)) {
            (Some(left), Some(right)) => match left.dependency_id().cmp(right.dependency_id()) {
                Ordering::Less => {
                    push_dependency(changes, PermissionChangeDisposition::Removed, left);
                    old_index += 1;
                }
                Ordering::Equal => {
                    if left != right {
                        push_dependency(changes, PermissionChangeDisposition::Changed, right);
                    }
                    old_index += 1;
                    candidate_index += 1;
                }
                Ordering::Greater => {
                    push_dependency(changes, PermissionChangeDisposition::Added, right);
                    candidate_index += 1;
                }
            },
            (Some(left), None) => {
                push_dependency(changes, PermissionChangeDisposition::Removed, left);
                old_index += 1;
            }
            (None, Some(right)) => {
                push_dependency(changes, PermissionChangeDisposition::Added, right);
                candidate_index += 1;
            }
            (None, None) => break,
        }
    }
}

fn push_dependency(
    changes: &mut Vec<PermissionChange>,
    disposition: PermissionChangeDisposition,
    dependency: &RuntimeDependencyDeclaration,
) {
    push_change(
        changes,
        PermissionChangeNamespace::RuntimeDependency,
        disposition,
        dependency.dependency_id().to_owned(),
    );
}

fn push_change(
    changes: &mut Vec<PermissionChange>,
    namespace: PermissionChangeNamespace,
    disposition: PermissionChangeDisposition,
    identifier: String,
) {
    changes.push(PermissionChange {
        namespace,
        disposition,
        identifier: identifier.into_boxed_str(),
    });
}

#[cfg(test)]
mod tests {
    use mengxia_plugin_package::inspect_manifest;

    use super::*;

    const GOLDEN_FILE: &[u8] =
        include_bytes!("../../mengxia-testkit/tests/fixtures/task_010/manifest-v1.golden.json");

    fn golden() -> &'static [u8] {
        GOLDEN_FILE
    }

    fn replace(source: &[u8], from: &str, to: &str) -> InspectedPluginPackage {
        let text = std::str::from_utf8(source).expect("utf8").replace(from, to);
        inspect_manifest(text.as_bytes()).expect("canonical replacement")
    }

    #[test]
    fn same_package_is_unchanged_and_identity_mismatch_denies() {
        let package = inspect_manifest(golden()).expect("golden");
        let same = diff_permissions(&package, &package);
        assert_eq!(
            same.classification(),
            PermissionDiffClassification::Unchanged
        );
        assert_eq!(same.reason(), IncomparableReason::None);
        assert!(same.changes().is_empty());

        let other = replace(golden(), "example.transcoder", "example.other");
        let mismatch = diff_permissions(&package, &other);
        assert_eq!(
            mismatch.classification(),
            PermissionDiffClassification::IncomparableDeny
        );
        assert_eq!(
            mismatch.reason(),
            IncomparableReason::PackageIdentityMismatch
        );
        assert!(mismatch.changes().is_empty());
    }

    #[test]
    fn dependency_change_and_mixed_removal_are_expansion() {
        let old = inspect_manifest(golden()).expect("golden");
        let changed = replace(golden(), "\"byte_length\":1234", "\"byte_length\":1235");
        let diff = diff_permissions(&old, &changed);
        assert_eq!(
            diff.classification(),
            PermissionDiffClassification::Expansion
        );
        assert_eq!(diff.changes().len(), 1);
        assert_eq!(
            diff.changes()[0].disposition(),
            PermissionChangeDisposition::Changed
        );

        let without_permission = replace(
            changed.canonical_bytes(),
            "{\"kind\":\"broker.asset.read@1\",\"scope\":\"run-inputs\"}",
            "",
        );
        let mixed = diff_permissions(&old, &without_permission);
        assert_eq!(
            mixed.classification(),
            PermissionDiffClassification::Expansion
        );
        assert_eq!(mixed.changes().len(), 2);
    }

    #[test]
    fn removal_only_is_contraction_and_version_only_is_unchanged() {
        let old = inspect_manifest(golden()).expect("golden");
        let without_permission = replace(
            golden(),
            "{\"kind\":\"broker.asset.read@1\",\"scope\":\"run-inputs\"}",
            "",
        );
        let contraction = diff_permissions(&old, &without_permission);
        assert_eq!(
            contraction.classification(),
            PermissionDiffClassification::Contraction
        );
        assert_eq!(contraction.reason(), IncomparableReason::None);
        assert_eq!(contraction.changes().len(), 1);
        assert_eq!(
            contraction.changes()[0].disposition(),
            PermissionChangeDisposition::Removed
        );

        let version_only = replace(golden(), "1.0.0", "1.0.1");
        assert_eq!(
            diff_permissions(&old, &version_only).classification(),
            PermissionDiffClassification::Unchanged
        );
    }
}
