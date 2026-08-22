//! Safe macOS filesystem-authority boundary for MengXia.
//!
//! The private ACL FFI backend and checked-in C shim are implemented by TASK-004.

#![deny(unsafe_op_in_unsafe_fn)]

/// Maximum ACL entries accepted by the V1 macOS adapter.
pub const ACL_ENTRY_LIMIT: u32 = 128;

/// Maximum serialized ACL bytes accepted by the V1 macOS adapter.
pub const ACL_EXTERNAL_REPRESENTATION_LIMIT: usize = 16_384;

/// Safe, owned summary produced by the private macOS ACL adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AclSummary {
    entry_count: u32,
    allow_count: u32,
    deny_count: u32,
    acl_flags: u32,
    entry_flags_or: u32,
}

impl AclSummary {
    /// Constructs a summary after the private adapter has validated all bounds.
    #[allow(dead_code)]
    pub(crate) const fn validated(
        entry_count: u32,
        allow_count: u32,
        deny_count: u32,
        acl_flags: u32,
        entry_flags_or: u32,
    ) -> Self {
        Self {
            entry_count,
            allow_count,
            deny_count,
            acl_flags,
            entry_flags_or,
        }
    }

    #[must_use]
    pub const fn entry_count(self) -> u32 {
        self.entry_count
    }

    #[must_use]
    pub const fn allow_count(self) -> u32 {
        self.allow_count
    }

    #[must_use]
    pub const fn deny_count(self) -> u32 {
        self.deny_count
    }

    #[must_use]
    pub const fn acl_flags(self) -> u32 {
        self.acl_flags
    }

    #[must_use]
    pub const fn entry_flags_or(self) -> u32 {
        self.entry_flags_or
    }
}

#[cfg(test)]
mod tests {
    use super::AclSummary;

    #[test]
    fn validated_summary_preserves_distinct_object_and_entry_flags() {
        let summary = AclSummary::validated(2, 1, 1, 0x10, 0x20);
        assert_eq!(summary.entry_count(), 2);
        assert_eq!(summary.allow_count(), 1);
        assert_eq!(summary.deny_count(), 1);
        assert_eq!(summary.acl_flags(), 0x10);
        assert_eq!(summary.entry_flags_or(), 0x20);
    }
}
