//! Pure review-eligibility comparison, not signature verification or execution authority.
//!
//! All facts are caller-supplied. Even `Checked` is only the caller's assertion;
//! a future authenticated admission boundary must establish their provenance.

use mengxia_types::Sha256Digest;

/// Explicit absence/verification state; neither zero hashes nor booleans imply trust.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewFact<T> {
    Missing,
    Unverified,
    Invalid,
    Checked(T),
}

/// Separate identities: manifest equality alone does not identify executable bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewIdentity {
    pub manifest_digest: Sha256Digest,
    pub artifact_digest: Sha256Digest,
    pub dependency_closure_digest: Sha256Digest,
    pub permissions_digest: Sha256Digest,
    pub profile_digest: Sha256Digest,
    pub platform_tuple_digest: Sha256Digest,
}

/// Unix milliseconds, with an exclusive end. Invalid intervals are rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewValidity {
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewRequirement {
    pub identity: ReviewIdentity,
    pub policy_version: u64,
    pub evidence_digest: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewApproval {
    pub identity: ReviewIdentity,
    pub policy_version: u64,
    pub evidence_digest: Sha256Digest,
    pub record_digest: Sha256Digest,
    pub validity: ReviewValidity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewRevocationStatus {
    Unknown,
    Clear,
    Revoked,
}

/// A result for one exact approval, not a publisher-wide or unbound clear flag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewRevocationSnapshot {
    pub approval_record_digest: Sha256Digest,
    pub status: ReviewRevocationStatus,
    pub epoch: u64,
    pub validity: ReviewValidity,
}

/// Observations from a future trusted clock/rollback store; this module performs no IO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewClock {
    pub now_ms: u64,
    pub last_observed_ms: u64,
    pub minimum_snapshot_epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewAssessment {
    pub requirement: ReviewFact<ReviewRequirement>,
    pub approval: ReviewFact<ReviewApproval>,
    pub revocation: ReviewFact<ReviewRevocationSnapshot>,
    pub clock: ReviewFact<ReviewClock>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewFactKind {
    Requirement,
    Approval,
    Revocation,
    Clock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewIdentityField {
    Manifest,
    Artifact,
    DependencyClosure,
    Permissions,
    Profile,
    PlatformTuple,
}

/// Closed, deterministic reasons; no path, secret or arbitrary diagnostic payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewDenial {
    MissingFact(ReviewFactKind),
    UnverifiedFact(ReviewFactKind),
    InvalidFact(ReviewFactKind),
    InvalidApprovalValidity,
    InvalidSnapshotValidity,
    UnknownRevocation,
    Revoked,
    ApprovalNotYetValid,
    ApprovalExpired,
    SnapshotNotYetValid,
    SnapshotExpired,
    ClockRollback,
    SnapshotRollback,
    IdentityMismatch(ReviewIdentityField),
    PolicyMismatch,
    EvidenceMismatch,
    RevocationBindingMismatch,
}

/// Comparison result only. No public constructor, serialization, grant or launch conversion.
///
/// This value proves neither authenticated review nor runtime qualification. It must
/// not be cached as permission: later sensitive sinks need fresh authenticated facts.
///
/// ```compile_fail
/// use mengxia_plugin_security::ReviewEligibilityCandidate;
/// let forged = ReviewEligibilityCandidate {};
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewEligibilityCandidate {
    identity: ReviewIdentity,
    approval_record_digest: Sha256Digest,
    evidence_digest: Sha256Digest,
    policy_version: u64,
    evaluated_at_ms: u64,
    valid_until_ms: u64,
    snapshot_epoch: u64,
}

impl ReviewEligibilityCandidate {
    #[must_use]
    pub const fn identity(&self) -> ReviewIdentity {
        self.identity
    }

    #[must_use]
    pub const fn approval_record_digest(&self) -> Sha256Digest {
        self.approval_record_digest
    }

    #[must_use]
    pub const fn evidence_digest(&self) -> Sha256Digest {
        self.evidence_digest
    }

    #[must_use]
    pub const fn policy_version(&self) -> u64 {
        self.policy_version
    }

    #[must_use]
    pub const fn evaluated_at_ms(&self) -> u64 {
        self.evaluated_at_ms
    }

    #[must_use]
    pub const fn valid_until_ms(&self) -> u64 {
        self.valid_until_ms
    }

    #[must_use]
    pub const fn snapshot_epoch(&self) -> u64 {
        self.snapshot_epoch
    }
}

fn checked<T: Copy>(fact: ReviewFact<T>, kind: ReviewFactKind) -> Result<T, ReviewDenial> {
    match fact {
        ReviewFact::Missing => Err(ReviewDenial::MissingFact(kind)),
        ReviewFact::Unverified => Err(ReviewDenial::UnverifiedFact(kind)),
        ReviewFact::Invalid => Err(ReviewDenial::InvalidFact(kind)),
        ReviewFact::Checked(value) => Ok(value),
    }
}

/// Evaluate the frozen ordering in ADR-0021's foundation gate in O(1) space/work.
/// No allocation, hashing, clock access, signature verification or authority is involved.
///
/// # Errors
/// Returns the first applicable `ReviewDenial` in the documented gate order.
pub fn evaluate_review_eligibility(
    assessment: &ReviewAssessment,
) -> Result<ReviewEligibilityCandidate, ReviewDenial> {
    let requirement = checked(assessment.requirement, ReviewFactKind::Requirement)?;
    let approval = checked(assessment.approval, ReviewFactKind::Approval)?;
    let snapshot = checked(assessment.revocation, ReviewFactKind::Revocation)?;
    let clock = checked(assessment.clock, ReviewFactKind::Clock)?;

    if approval.validity.start_ms >= approval.validity.end_ms {
        return Err(ReviewDenial::InvalidApprovalValidity);
    }
    if snapshot.validity.start_ms >= snapshot.validity.end_ms {
        return Err(ReviewDenial::InvalidSnapshotValidity);
    }
    match snapshot.status {
        ReviewRevocationStatus::Unknown => return Err(ReviewDenial::UnknownRevocation),
        ReviewRevocationStatus::Revoked => return Err(ReviewDenial::Revoked),
        ReviewRevocationStatus::Clear => {}
    }
    if clock.now_ms < approval.validity.start_ms {
        return Err(ReviewDenial::ApprovalNotYetValid);
    }
    if clock.now_ms >= approval.validity.end_ms {
        return Err(ReviewDenial::ApprovalExpired);
    }
    if clock.now_ms < snapshot.validity.start_ms {
        return Err(ReviewDenial::SnapshotNotYetValid);
    }
    if clock.now_ms >= snapshot.validity.end_ms {
        return Err(ReviewDenial::SnapshotExpired);
    }
    if clock.now_ms < clock.last_observed_ms {
        return Err(ReviewDenial::ClockRollback);
    }
    if snapshot.epoch < clock.minimum_snapshot_epoch {
        return Err(ReviewDenial::SnapshotRollback);
    }

    let expected = requirement.identity;
    let actual = approval.identity;
    for (field, expected, actual) in [
        (
            ReviewIdentityField::Manifest,
            expected.manifest_digest,
            actual.manifest_digest,
        ),
        (
            ReviewIdentityField::Artifact,
            expected.artifact_digest,
            actual.artifact_digest,
        ),
        (
            ReviewIdentityField::DependencyClosure,
            expected.dependency_closure_digest,
            actual.dependency_closure_digest,
        ),
        (
            ReviewIdentityField::Permissions,
            expected.permissions_digest,
            actual.permissions_digest,
        ),
        (
            ReviewIdentityField::Profile,
            expected.profile_digest,
            actual.profile_digest,
        ),
        (
            ReviewIdentityField::PlatformTuple,
            expected.platform_tuple_digest,
            actual.platform_tuple_digest,
        ),
    ] {
        if expected != actual {
            return Err(ReviewDenial::IdentityMismatch(field));
        }
    }
    if requirement.policy_version != approval.policy_version {
        return Err(ReviewDenial::PolicyMismatch);
    }
    if requirement.evidence_digest != approval.evidence_digest {
        return Err(ReviewDenial::EvidenceMismatch);
    }
    if snapshot.approval_record_digest != approval.record_digest {
        return Err(ReviewDenial::RevocationBindingMismatch);
    }

    Ok(ReviewEligibilityCandidate {
        identity: actual,
        approval_record_digest: approval.record_digest,
        evidence_digest: approval.evidence_digest,
        policy_version: approval.policy_version,
        evaluated_at_ms: clock.now_ms,
        valid_until_ms: approval.validity.end_ms.min(snapshot.validity.end_ms),
        snapshot_epoch: snapshot.epoch,
    })
}
