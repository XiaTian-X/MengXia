//! The sole audited unsafe boundary for the checked-in macOS ACL shim.

#![allow(unsafe_code)]

use std::ffi::CString;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use crate::{ACL_ENTRY_LIMIT, ACL_EXTERNAL_REPRESENTATION_LIMIT, AclSummary, AuthorityError};

const ABI_VERSION: u32 = 1;
const STATUS_OK: i32 = 0;
const ACL_FLAG_MASK: u32 = 0b11;
const ENTRY_FLAG_MASK: u32 = 0b1_1111;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MengxiaAclSummaryV1 {
    abi_version: u32,
    entry_count: u32,
    allow_count: u32,
    deny_count: u32,
    acl_flags: u32,
    entry_flags_or: u32,
    inheritable_count: u32,
    external_size: u32,
    os_errno: i32,
    reserved: u32,
}

unsafe extern "C" {
    fn mengxia_acl_abi_version_v1() -> u32;
    fn mengxia_acl_inspect_fd_v1(fd: i32, out: *mut MengxiaAclSummaryV1) -> i32;
    fn mengxia_acl_path_is_empty_v1(path: *const i8, os_errno: *mut i32) -> i32;
}

pub(super) fn require_empty_path(path: &Path) -> Result<(), AuthorityError> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let mut os_errno = 0_i32;
    // SAFETY: `path` is a live NUL-terminated byte string and `os_errno` is a
    // uniquely borrowed, correctly aligned C int output for the call.
    let status = unsafe { mengxia_acl_path_is_empty_v1(path.as_ptr(), &mut os_errno) };
    if status == STATUS_OK && os_errno == 0 {
        Ok(())
    } else {
        Err(match status {
            2 => AuthorityError::Io,
            _ => AuthorityError::UnsafeConfiguration,
        })
    }
}

pub(super) fn inspect(fd: BorrowedFd<'_>) -> Result<AclSummary, AuthorityError> {
    // SAFETY: the linked function has the checked-in version-one C signature,
    // takes no pointers, and the build-time ABI probe asserts its integer ABI.
    if unsafe { mengxia_acl_abi_version_v1() } != ABI_VERSION {
        return Err(AuthorityError::UnsafeConfiguration);
    }

    let mut raw = MengxiaAclSummaryV1::default();
    // SAFETY: `raw` is a valid, uniquely borrowed, exactly laid-out 40-byte
    // output object for the duration of the call; `fd` is borrowed and open.
    let status = unsafe { mengxia_acl_inspect_fd_v1(fd.as_raw_fd(), &mut raw) };
    if status != STATUS_OK {
        return Err(match status {
            1 | 3..=7 => AuthorityError::UnsafeConfiguration,
            2 => AuthorityError::Io,
            _ => AuthorityError::UnsafeConfiguration,
        });
    }

    let counts_are_valid = raw.entry_count <= ACL_ENTRY_LIMIT
        && raw
            .allow_count
            .checked_add(raw.deny_count)
            .is_some_and(|total| total == raw.entry_count)
        && raw.inheritable_count <= raw.entry_count;
    let representation_is_valid = (raw.entry_count == 0
        && raw.external_size == 0
        && raw.acl_flags == 0
        && raw.entry_flags_or == 0)
        || (raw.external_size > 0
            && usize::try_from(raw.external_size)
                .is_ok_and(|size| size <= ACL_EXTERNAL_REPRESENTATION_LIMIT));
    if raw.abi_version != ABI_VERSION
        || raw.reserved != 0
        || raw.os_errno != 0
        || !counts_are_valid
        || !representation_is_valid
        || raw.acl_flags & !ACL_FLAG_MASK != 0
        || raw.entry_flags_or & !ENTRY_FLAG_MASK != 0
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }

    Ok(AclSummary::validated(
        raw.entry_count,
        raw.allow_count,
        raw.deny_count,
        raw.acl_flags,
        raw.entry_flags_or,
        raw.inheritable_count,
        raw.external_size,
    ))
}

#[cfg(test)]
mod tests {
    use std::mem::{align_of, offset_of, size_of};

    use super::MengxiaAclSummaryV1;

    #[test]
    fn rust_layout_exactly_matches_version_one_c_abi() {
        assert_eq!(size_of::<MengxiaAclSummaryV1>(), 40);
        assert_eq!(align_of::<MengxiaAclSummaryV1>(), 4);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, abi_version), 0);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, entry_count), 4);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, allow_count), 8);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, deny_count), 12);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, acl_flags), 16);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, entry_flags_or), 20);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, inheritable_count), 24);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, external_size), 28);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, os_errno), 32);
        assert_eq!(offset_of!(MengxiaAclSummaryV1, reserved), 36);
    }
}
