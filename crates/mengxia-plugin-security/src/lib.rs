//! Plugin policy boundary for MengXia.

#![forbid(unsafe_code)]

pub mod broker_foundation;

mod permission_diff;
mod reviewed_admission;

pub use permission_diff::{
    IncomparableReason, PermissionChange, PermissionChangeDisposition, PermissionChangeNamespace,
    PermissionDiff, PermissionDiffClassification, diff_permissions,
};
pub use reviewed_admission::{
    ReviewApproval, ReviewAssessment, ReviewClock, ReviewDenial, ReviewEligibilityCandidate,
    ReviewFact, ReviewFactKind, ReviewIdentity, ReviewIdentityField, ReviewRequirement,
    ReviewRevocationSnapshot, ReviewRevocationStatus, ReviewValidity, evaluate_review_eligibility,
};
