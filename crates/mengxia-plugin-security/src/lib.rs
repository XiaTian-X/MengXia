//! Plugin policy boundary for MengXia.

#![forbid(unsafe_code)]

mod permission_diff;

pub use permission_diff::{
    IncomparableReason, PermissionChange, PermissionChangeDisposition, PermissionChangeNamespace,
    PermissionDiff, PermissionDiffClassification, diff_permissions,
};
