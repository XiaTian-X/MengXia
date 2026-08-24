//! Safe macOS filesystem-authority boundary for MengXia.

#![deny(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

mod macos_ffi;

use std::ffi::OsString;
use std::fmt;
use std::fs::{File, TryLockError};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Component, Path, PathBuf};

use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, fstat, fstatfs, fsync, linkat, mkdirat, open, openat,
    unlinkat,
};
use rustix::io::{read, write};
use rustix::process::geteuid;

/// Maximum ACL entries accepted by the V1 macOS adapter.
pub const ACL_ENTRY_LIMIT: u32 = 128;

/// Maximum serialized ACL bytes accepted by the V1 macOS adapter.
pub const ACL_EXTERNAL_REPRESENTATION_LIMIT: usize = 16_384;
/// Exact accepted TASK-004 bootstrap-intent record length.
pub const BOOTSTRAP_INTENT_RECORD_LENGTH: usize = 256;

const MNT_LOCAL: u32 = 0x0000_1000;
const MNT_IGNORE_OWNERSHIP: u32 = 0x0020_0000;
const ACL_DEFER_INHERIT: u32 = 1 << 0;
const ACL_NO_INHERIT: u32 = 1 << 1;

/// Redacted failure from the platform authority boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthorityError {
    /// The path or its security metadata could not establish safe authority.
    UnsafeConfiguration,
    /// The operating system failed while reading required security evidence.
    Io,
    /// Another live process owns the Library lock.
    Contended,
    /// Canonical and staging names claim conflicting persistent identities.
    ConflictingData,
}

impl fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeConfiguration => "filesystem authority is unsupported or unsafe",
            Self::Io => "filesystem authority inspection failed",
            Self::Contended => "library is already open by another process",
            Self::ConflictingData => "filesystem data identities conflict",
        })
    }
}

impl std::error::Error for AuthorityError {}

/// Safe, owned summary produced by the private macOS ACL adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AclSummary {
    entry_count: u32,
    allow_count: u32,
    deny_count: u32,
    acl_flags: u32,
    entry_flags_or: u32,
    inheritable_count: u32,
    external_size: u32,
}

impl AclSummary {
    pub(crate) const fn validated(
        entry_count: u32,
        allow_count: u32,
        deny_count: u32,
        acl_flags: u32,
        entry_flags_or: u32,
        inheritable_count: u32,
        external_size: u32,
    ) -> Self {
        Self {
            entry_count,
            allow_count,
            deny_count,
            acl_flags,
            entry_flags_or,
            inheritable_count,
            external_size,
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

    #[must_use]
    pub const fn inheritable_count(self) -> u32 {
        self.inheritable_count
    }

    #[must_use]
    pub const fn external_size(self) -> u32 {
        self.external_size
    }

    const fn is_empty(self) -> bool {
        self.entry_count == 0 && self.acl_flags == 0 && self.entry_flags_or == 0
    }

    const fn permits_prefix(self) -> bool {
        self.allow_count == 0
            && self.deny_count == self.entry_count
            && self.acl_flags & ACL_DEFER_INHERIT == 0
            && self.acl_flags & !ACL_NO_INHERIT == 0
    }
}

/// Safe evidence captured for one already-open macOS filesystem object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacOsObjectSecurity {
    device: u64,
    inode: u64,
    owner_uid: u32,
    mode: u32,
    acl: AclSummary,
}

impl MacOsObjectSecurity {
    #[must_use]
    pub const fn owner_uid(self) -> u32 {
        self.owner_uid
    }

    #[must_use]
    pub const fn mode(self) -> u32 {
        self.mode
    }

    #[must_use]
    pub const fn acl(self) -> AclSummary {
        self.acl
    }

    const fn same_object(self, other: Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}

struct RetainedComponent {
    name: Option<OsString>,
    fd: OwnedFd,
    security: MacOsObjectSecurity,
    role: ComponentRole,
}

#[derive(Clone, Copy)]
enum ComponentRole {
    Ancestor,
    FinalParent,
    LibraryRoot,
}

/// Opaque authority for one existing, owner-only Library root.
///
/// Construction walks from `/` with descriptor-relative no-follow opens and
/// retains every directory handle. The value deliberately has no `Clone`,
/// `Display`, serialization or unchecked constructor.
pub struct ValidatedAbsolutePath {
    components: Vec<RetainedComponent>,
    canonical_sqlite_path: PathBuf,
    staging_sqlite_path: PathBuf,
    owner_uid: u32,
    root_device: u64,
    root_inode: u64,
}

impl ValidatedAbsolutePath {
    /// Authorizes an existing Library root without mutating it.
    pub fn authorize_existing(path: &Path) -> Result<Self, AuthorityError> {
        Self::authorize(path, false).map(|(authority, _)| authority)
    }

    /// Validates the complete parent chain for a prospective Library root
    /// without opening or creating the final root entry.
    ///
    /// Fresh bootstrap uses this read-only preflight before sampling its clock
    /// and identity sources. The later locked acquisition repeats the proof
    /// before it is allowed to create or mutate the Library root.
    pub fn authorize_bootstrap_parent(path: &Path) -> Result<(), AuthorityError> {
        validate_lexical_absolute_path(path)?;
        let names: Vec<OsString> = path
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_os_string()),
                _ => None,
            })
            .collect();
        if names.is_empty() {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        let owner_uid = geteuid().as_raw();
        let root_fd = open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let root_role = if names.len() == 1 {
            ComponentRole::FinalParent
        } else {
            ComponentRole::Ancestor
        };
        let root_security = inspect_directory(root_fd.as_fd())?;
        validate_component_policy(root_security, root_role, owner_uid)?;
        let mut retained = vec![RetainedComponent {
            name: None,
            fd: root_fd,
            security: root_security,
            role: root_role,
        }];

        for (index, name) in names.iter().enumerate().take(names.len() - 1) {
            let role = if index + 2 == names.len() {
                ComponentRole::FinalParent
            } else {
                ComponentRole::Ancestor
            };
            let parent = retained.last().ok_or(AuthorityError::UnsafeConfiguration)?;
            let fd = openat(
                parent.fd.as_fd(),
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| AuthorityError::UnsafeConfiguration)?;
            let security = inspect_directory(fd.as_fd())?;
            validate_component_policy(security, role, owner_uid)?;
            retained.push(RetainedComponent {
                name: Some(name.clone()),
                fd,
                security,
                role,
            });
        }

        revalidate_components(&retained, owner_uid)
    }

    fn authorize(path: &Path, create_if_absent: bool) -> Result<(Self, bool), AuthorityError> {
        validate_lexical_absolute_path(path)?;
        let names: Vec<OsString> = path
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_os_string()),
                _ => None,
            })
            .collect();
        if names.is_empty() {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        let owner_uid = geteuid().as_raw();
        let root_fd = open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let root_role = if names.len() == 1 {
            ComponentRole::FinalParent
        } else {
            ComponentRole::Ancestor
        };
        let root_security = inspect_directory(root_fd.as_fd())?;
        validate_component_policy(root_security, root_role, owner_uid)?;
        let mut retained = vec![RetainedComponent {
            name: None,
            fd: root_fd,
            security: root_security,
            role: root_role,
        }];

        for (index, name) in names.iter().enumerate().take(names.len() - 1) {
            let role = if index + 2 == names.len() {
                ComponentRole::FinalParent
            } else {
                ComponentRole::Ancestor
            };
            let parent = retained.last().ok_or(AuthorityError::UnsafeConfiguration)?;
            let fd = openat(
                parent.fd.as_fd(),
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| AuthorityError::UnsafeConfiguration)?;
            let security = inspect_directory(fd.as_fd())?;
            validate_component_policy(security, role, owner_uid)?;
            retained.push(RetainedComponent {
                name: Some(name.clone()),
                fd,
                security,
                role,
            });
        }

        let root_name = names.last().ok_or(AuthorityError::UnsafeConfiguration)?;
        let parent = retained.last().ok_or(AuthorityError::UnsafeConfiguration)?;
        let (library_root_fd, created, sync_root_entry) = match openat(
            parent.fd.as_fd(),
            root_name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => (fd, false, false),
            Err(rustix::io::Errno::NOENT) if create_if_absent => {
                let created = match mkdirat(parent.fd.as_fd(), root_name, Mode::RWXU) {
                    Ok(()) => true,
                    Err(rustix::io::Errno::EXIST) => false,
                    Err(_) => return Err(AuthorityError::Io),
                };
                let fd = openat(
                    parent.fd.as_fd(),
                    root_name,
                    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|error| match error {
                    rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => {
                        AuthorityError::UnsafeConfiguration
                    }
                    _ => AuthorityError::Io,
                })?;
                (fd, created, true)
            }
            Err(_) => return Err(AuthorityError::UnsafeConfiguration),
        };
        let library_root_security = inspect_directory(library_root_fd.as_fd())?;
        validate_component_policy(library_root_security, ComponentRole::LibraryRoot, owner_uid)?;
        if sync_root_entry {
            fsync(library_root_fd.as_fd()).map_err(|_| AuthorityError::Io)?;
            fsync(parent.fd.as_fd()).map_err(|_| AuthorityError::Io)?;
        }
        retained.push(RetainedComponent {
            name: Some(root_name.clone()),
            fd: library_root_fd,
            security: library_root_security,
            role: ComponentRole::LibraryRoot,
        });

        let authority = Self {
            components: retained,
            canonical_sqlite_path: path.join("library.sqlite3"),
            staging_sqlite_path: path.join(".library.sqlite3.bootstrap"),
            owner_uid,
            root_device: library_root_security.device,
            root_inode: library_root_security.inode,
        };
        authority.revalidate_chain()?;
        Ok((authority, created))
    }

    /// Reopens every retained name from its retained predecessor and proves
    /// that each edge still resolves to the same device/inode under policy.
    pub fn revalidate_chain(&self) -> Result<(), AuthorityError> {
        revalidate_components(&self.components, self.owner_uid)
    }

    fn library_root_fd(&self) -> BorrowedFd<'_> {
        self.components
            .last()
            .expect("validated authority always retains its Library root")
            .fd
            .as_fd()
    }

    /// Validates one fixed SQLite child through the retained root descriptor.
    pub fn validate_sqlite_child(
        &self,
        child: SqliteChild,
    ) -> Result<MacOsObjectSecurity, AuthorityError> {
        let name = match child {
            SqliteChild::Canonical => "library.sqlite3",
            SqliteChild::BootstrapStaging => ".library.sqlite3.bootstrap",
        };
        let fd = openat(
            self.library_root_fd(),
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let stat = fstat(fd.as_fd()).map_err(|_| AuthorityError::Io)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let security = inspect_security(
            fd.as_fd(),
            stat.st_dev as u64,
            stat.st_ino as u64,
            stat.st_uid,
            stat.st_mode,
        )?;
        if security.owner_uid != self.owner_uid
            || security.mode != 0o600
            || !security.acl.is_empty()
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(security)
    }
    /// Validates every SQLite WAL/SHM sidecar currently present for one fixed
    /// database name. Absence is accepted; an unsafe present sidecar is not.
    pub fn validate_sqlite_sidecars(&self, child: SqliteChild) -> Result<(), AuthorityError> {
        let (wal, shm) = match child {
            SqliteChild::Canonical => ("library.sqlite3-wal", "library.sqlite3-shm"),
            SqliteChild::BootstrapStaging => (
                ".library.sqlite3.bootstrap-wal",
                ".library.sqlite3.bootstrap-shm",
            ),
        };
        let entries = enumerate_root(self)?;
        for name in [wal, shm] {
            if entries
                .iter()
                .any(|entry| entry.as_slice() == name.as_bytes())
            {
                let fd = openat(
                    self.library_root_fd(),
                    name,
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                    Mode::empty(),
                )
                .map_err(|_| AuthorityError::UnsafeConfiguration)?;
                inspect_internal_file(fd.as_fd(), self.owner_uid)?;
            }
        }
        Ok(())
    }

    fn open_sqlite_wal(
        &self,
        child: SqliteChild,
    ) -> Result<Option<ValidatedSqliteWal>, AuthorityError> {
        self.revalidate_chain()?;
        let name = match child {
            SqliteChild::Canonical => "library.sqlite3-wal",
            SqliteChild::BootstrapStaging => ".library.sqlite3.bootstrap-wal",
        };
        let entries = enumerate_root(self)?;
        if !entries
            .iter()
            .any(|entry| entry.as_slice() == name.as_bytes())
        {
            return Ok(None);
        }
        let fd = openat(
            self.library_root_fd(),
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        inspect_internal_file(fd.as_fd(), self.owner_uid)?;
        self.revalidate_chain()?;
        Ok(Some(ValidatedSqliteWal {
            file: File::from(fd),
        }))
    }

    /// Mints one of the two unforgeable fixed SQLite child-path tokens.
    #[must_use]
    pub fn sqlite_child(&self, child: SqliteChild) -> FixedSqliteChildPath<'_> {
        FixedSqliteChildPath {
            path: match child {
                SqliteChild::Canonical => &self.canonical_sqlite_path,
                SqliteChild::BootstrapStaging => &self.staging_sqlite_path,
            },
        }
    }
}

fn revalidate_components(
    components: &[RetainedComponent],
    owner_uid: u32,
) -> Result<(), AuthorityError> {
    let fresh_root = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let mut fresh_parent = fresh_root;
    let root_security = inspect_directory(fresh_parent.as_fd())?;
    let retained_root = components
        .first()
        .ok_or(AuthorityError::UnsafeConfiguration)?;
    validate_component_policy(root_security, retained_root.role, owner_uid)?;
    if !root_security.same_object(retained_root.security) {
        return Err(AuthorityError::UnsafeConfiguration);
    }

    for retained in components.iter().skip(1) {
        let name = retained
            .name
            .as_ref()
            .ok_or(AuthorityError::UnsafeConfiguration)?;
        let fresh = openat(
            fresh_parent.as_fd(),
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let security = inspect_directory(fresh.as_fd())?;
        validate_component_policy(security, retained.role, owner_uid)?;
        if !security.same_object(retained.security) {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        fresh_parent = fresh;
    }
    Ok(())
}

/// Exclusive bootstrap authority for one owner-only Library root.
///
/// This value owns the same locked file description for its full lifetime and
/// deliberately has no `Clone`, serialization or raw-lock accessor.
pub struct OpenedLibraryAuthority {
    path: ValidatedAbsolutePath,
    _lock_file: File,
}

impl Drop for OpenedLibraryAuthority {
    fn drop(&mut self) {
        let _ = self._lock_file.unlock();
    }
}

/// Descriptor-validated post-lock filesystem state understood by the current
/// TASK-004 bootstrap slice. Intent bytes are not semantically trusted here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BootstrapFilesystemState {
    LockOnly,
    IntentOnly([u8; BOOTSTRAP_INTENT_RECORD_LENGTH]),
    IntentWithStaging([u8; BOOTSTRAP_INTENT_RECORD_LENGTH]),
    IntentWithPublishedStaging([u8; BOOTSTRAP_INTENT_RECORD_LENGTH]),
    IntentWithCanonical([u8; BOOTSTRAP_INTENT_RECORD_LENGTH]),
    CanonicalOnly,
}

impl BootstrapFilesystemState {
    #[must_use]
    pub const fn is_lock_only(&self) -> bool {
        matches!(self, Self::LockOnly)
    }

    #[must_use]
    pub const fn intent_record(&self) -> Option<&[u8; BOOTSTRAP_INTENT_RECORD_LENGTH]> {
        match self {
            Self::IntentOnly(record)
            | Self::IntentWithStaging(record)
            | Self::IntentWithPublishedStaging(record)
            | Self::IntentWithCanonical(record) => Some(record),
            Self::LockOnly | Self::CanonicalOnly => None,
        }
    }
}

impl OpenedLibraryAuthority {
    /// Compares one already-validated configuration root without exposing or
    /// minting a fixed SQLite child-path token to the caller.
    #[must_use]
    pub fn authorizes_library_root(&self, library_root: &Path) -> bool {
        self.path.canonical_sqlite_path.parent() == Some(library_root)
    }

    /// Opens only the fixed bootstrap-staging WAL while this value retains the
    /// exclusive Library lock. The reader exposes bytes but no path or raw fd.
    pub fn open_bootstrap_staging_wal(&self) -> Result<Option<ValidatedSqliteWal>, AuthorityError> {
        self.path.open_sqlite_wal(SqliteChild::BootstrapStaging)
    }

    /// Opens or creates an absent/empty Library root and acquires its durable
    /// fixed lock. No intent, staging file or SQLite database is created.
    pub fn acquire_bootstrap(path: &Path) -> Result<Self, AuthorityError> {
        let (authority, state) = Self::acquire_bootstrap_state(path)?;
        if !state.is_lock_only() {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(authority)
    }

    /// Acquires the fixed lock, re-enumerates under that lock, and accepts only
    /// lock-only or lock-plus-one-exact-size-intent states.
    pub fn acquire_bootstrap_state(
        path: &Path,
    ) -> Result<(Self, BootstrapFilesystemState), AuthorityError> {
        let (authority, _) = ValidatedAbsolutePath::authorize(path, true)?;
        let before_lock = enumerate_root(&authority)?;
        let lock_exists = before_lock
            .iter()
            .any(|entry| entry.as_slice() == b".mengxia.lock");
        if !lock_exists && !before_lock.is_empty() {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        let lock_fd = if lock_exists {
            openat(
                authority.library_root_fd(),
                ".mengxia.lock",
                OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| AuthorityError::UnsafeConfiguration)?
        } else {
            match openat(
                authority.library_root_fd(),
                ".mengxia.lock",
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(fd) => fd,
                Err(rustix::io::Errno::EXIST) => openat(
                    authority.library_root_fd(),
                    ".mengxia.lock",
                    OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| AuthorityError::UnsafeConfiguration)?,
                Err(rustix::io::Errno::LOOP) => {
                    return Err(AuthorityError::UnsafeConfiguration);
                }
                Err(_) => return Err(AuthorityError::Io),
            }
        };
        let lock_file = File::from(lock_fd);
        match lock_file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(AuthorityError::Contended);
            }
            Err(TryLockError::Error(_)) => return Err(AuthorityError::Io),
        }

        let locked_security = inspect_internal_file(lock_file.as_fd(), authority.owner_uid)?;
        fsync(lock_file.as_fd()).map_err(|_| AuthorityError::Io)?;
        fsync(authority.library_root_fd()).map_err(|_| AuthorityError::Io)?;

        authority.revalidate_chain()?;
        let reopened = openat(
            authority.library_root_fd(),
            ".mengxia.lock",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let reopened_security = inspect_internal_file(reopened.as_fd(), authority.owner_uid)?;
        if !reopened_security.same_object(locked_security) {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        let post_lock_entries = enumerate_root(&authority)?;
        let state = match post_lock_entries.as_slice() {
            [lock] if lock == b".mengxia.lock" => BootstrapFilesystemState::LockOnly,
            [intent, lock]
                if intent == b".mengxia.bootstrap-intent" && lock == b".mengxia.lock" =>
            {
                BootstrapFilesystemState::IntentOnly(read_bootstrap_intent(&authority)?)
            }
            entries if is_intent_with_staging_entries(entries) => {
                authority.validate_sqlite_child(SqliteChild::BootstrapStaging)?;
                authority.validate_sqlite_sidecars(SqliteChild::BootstrapStaging)?;
                BootstrapFilesystemState::IntentWithStaging(
                    read_bootstrap_intent_in_observed_state(&authority, entries)?,
                )
            }
            [staging, intent, lock, canonical]
                if staging == b".library.sqlite3.bootstrap"
                    && intent == b".mengxia.bootstrap-intent"
                    && lock == b".mengxia.lock"
                    && canonical == b"library.sqlite3" =>
            {
                let staging_security =
                    authority.validate_sqlite_child(SqliteChild::BootstrapStaging)?;
                let canonical_security = authority.validate_sqlite_child(SqliteChild::Canonical)?;
                if !staging_security.same_object(canonical_security) {
                    return Err(AuthorityError::ConflictingData);
                }
                BootstrapFilesystemState::IntentWithPublishedStaging(
                    read_bootstrap_intent_with_published_staging(&authority)?,
                )
            }
            [intent, lock, canonical]
                if intent == b".mengxia.bootstrap-intent"
                    && lock == b".mengxia.lock"
                    && canonical == b"library.sqlite3" =>
            {
                authority.validate_sqlite_child(SqliteChild::Canonical)?;
                BootstrapFilesystemState::IntentWithCanonical(read_bootstrap_intent_with_canonical(
                    &authority,
                )?)
            }
            [lock, canonical] if lock == b".mengxia.lock" && canonical == b"library.sqlite3" => {
                authority.validate_sqlite_child(SqliteChild::Canonical)?;
                BootstrapFilesystemState::CanonicalOnly
            }
            _ => return Err(AuthorityError::UnsafeConfiguration),
        };
        Ok((
            Self {
                path: authority,
                _lock_file: lock_file,
            },
            state,
        ))
    }

    /// Borrows the retained path authority while this value owns the lock.
    #[must_use]
    pub const fn path_authority(&self) -> &ValidatedAbsolutePath {
        &self.path
    }

    /// Returns the effective UID proven for the root and lock.
    #[must_use]
    pub const fn owner_uid(&self) -> u32 {
        self.path.owner_uid
    }

    /// Returns the losslessly captured device/inode identity of the held root.
    #[must_use]
    pub const fn root_identity(&self) -> (u64, u64) {
        (self.path.root_device, self.path.root_inode)
    }

    /// Creates and durably writes the fixed bootstrap intent while retaining
    /// the exclusive Library lock. Failures preserve any created prefix.
    pub fn create_durable_bootstrap_intent(
        &self,
        record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.create_durable_bootstrap_intent_with(record, &mut RealBootstrapFsOps)
    }

    /// Re-proves and re-syncs one accepted intent, then creates the fixed empty
    /// staging file durably. No SQLite connection is opened by this operation.
    pub fn refsync_intent_and_create_staging(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.refsync_intent_and_create_staging_with(expected_record, &mut RealBootstrapFsOps)
    }

    /// Requires a closed staging database with no WAL/SHM sidecars, synchronizes
    /// that exact inode, and re-proves the intent/root/name authority afterward.
    pub fn sync_closed_staging_database(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        if read_bootstrap_intent_with_staging(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let expected_entries = [
            b".library.sqlite3.bootstrap".to_vec(),
            b".mengxia.bootstrap-intent".to_vec(),
            b".mengxia.lock".to_vec(),
        ];
        if enumerate_root(&self.path)? != expected_entries {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let staging = openat(
            self.path.library_root_fd(),
            ".library.sqlite3.bootstrap",
            OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let initial = inspect_internal_file(staging.as_fd(), self.path.owner_uid)?;
        fsync(staging.as_fd()).map_err(|_| AuthorityError::Io)?;

        self.path.revalidate_chain()?;
        let reopened = openat(
            self.path.library_root_fd(),
            ".library.sqlite3.bootstrap",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let reopened_security = inspect_internal_file(reopened.as_fd(), self.path.owner_uid)?;
        if !reopened_security.same_object(initial)
            || read_bootstrap_intent_with_staging(&self.path)? != *expected_record
            || enumerate_root(&self.path)? != expected_entries
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(())
    }

    /// Removes only the fixed staging database/sidecar set authorized by one
    /// exact valid intent. The caller must first prove at the SQLite layer that
    /// no bootstrap commit exists. Namespace failures preserve their completed
    /// prefix for the next locked recovery attempt.
    pub fn cleanup_authorized_incomplete_staging(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.cleanup_authorized_incomplete_staging_with(expected_record, &mut RealCleanupFsOps)
    }

    /// Publishes one fully closed and verified staging inode through a hard link,
    /// then removes staging and intent names in the accepted fsync order.
    pub fn publish_verified_staging(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.publish_verified_staging_with(expected_record, &mut RealPublishFsOps)
    }

    /// Resumes after the canonical hard link already exists and is proven to be
    /// the same inode as staging.
    pub fn finish_published_staging(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.finish_published_staging_with(expected_record, &mut RealPublishFsOps)
    }

    /// Removes a verified intent left beside a matching canonical database and
    /// synchronizes the resulting canonical-only namespace.
    pub fn finish_canonical_intent(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.finish_canonical_intent_with(expected_record, &mut RealPublishFsOps)
    }

    /// Synchronizes the final closed canonical database and proves that only it
    /// and the retained lock remain in the Library root.
    pub fn sync_closed_canonical_database(&self) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        let expected_entries = [b".mengxia.lock".to_vec(), b"library.sqlite3".to_vec()];
        if enumerate_root(&self.path)? != expected_entries {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let canonical = openat(
            self.path.library_root_fd(),
            "library.sqlite3",
            OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let initial = inspect_internal_file(canonical.as_fd(), self.path.owner_uid)?;
        fsync(canonical.as_fd()).map_err(|_| AuthorityError::Io)?;

        self.path.revalidate_chain()?;
        let reopened_security = self.path.validate_sqlite_child(SqliteChild::Canonical)?;
        if !reopened_security.same_object(initial)
            || enumerate_root(&self.path)? != expected_entries
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(())
    }

    fn publish_verified_staging_with<Ops: PublishFsOps>(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
        operations: &mut Ops,
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        if read_bootstrap_intent_with_staging(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        self.path
            .validate_sqlite_child(SqliteChild::BootstrapStaging)?;
        self.path
            .validate_sqlite_sidecars(SqliteChild::BootstrapStaging)?;

        operations.link_staging(self.path.library_root_fd())?;
        self.finish_published_staging_with(expected_record, operations)
    }

    fn finish_published_staging_with<Ops: PublishFsOps>(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
        operations: &mut Ops,
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        if read_bootstrap_intent_with_published_staging(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let linked_staging = self
            .path
            .validate_sqlite_child(SqliteChild::BootstrapStaging)?;
        let canonical_security = self.path.validate_sqlite_child(SqliteChild::Canonical)?;
        if !linked_staging.same_object(canonical_security) {
            return Err(AuthorityError::ConflictingData);
        }

        operations.sync_root(self.path.library_root_fd())?;
        if read_bootstrap_intent_with_published_staging(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        operations.unlink_staging(self.path.library_root_fd())?;
        self.path.revalidate_chain()?;
        if read_bootstrap_intent_with_canonical(&self.path)? != *expected_record
            || !self
                .path
                .validate_sqlite_child(SqliteChild::Canonical)?
                .same_object(canonical_security)
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        operations.sync_root(self.path.library_root_fd())?;
        if read_bootstrap_intent_with_canonical(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        operations.unlink_intent(self.path.library_root_fd())?;
        self.path.revalidate_chain()?;
        let expected_entries = [b".mengxia.lock".to_vec(), b"library.sqlite3".to_vec()];
        if enumerate_root(&self.path)? != expected_entries
            || !self
                .path
                .validate_sqlite_child(SqliteChild::Canonical)?
                .same_object(canonical_security)
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        operations.sync_root(self.path.library_root_fd())?;
        self.path.revalidate_chain()?;
        if enumerate_root(&self.path)? != expected_entries
            || !self
                .path
                .validate_sqlite_child(SqliteChild::Canonical)?
                .same_object(canonical_security)
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(())
    }

    fn finish_canonical_intent_with<Ops: PublishFsOps>(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
        operations: &mut Ops,
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        if read_bootstrap_intent_with_canonical(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let canonical_security = self.path.validate_sqlite_child(SqliteChild::Canonical)?;
        operations.unlink_intent(self.path.library_root_fd())?;
        let expected_entries = [b".mengxia.lock".to_vec(), b"library.sqlite3".to_vec()];
        self.path.revalidate_chain()?;
        if enumerate_root(&self.path)? != expected_entries
            || !self
                .path
                .validate_sqlite_child(SqliteChild::Canonical)?
                .same_object(canonical_security)
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        operations.sync_root(self.path.library_root_fd())?;
        self.path.revalidate_chain()?;
        if enumerate_root(&self.path)? != expected_entries
            || !self
                .path
                .validate_sqlite_child(SqliteChild::Canonical)?
                .same_object(canonical_security)
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(())
    }

    fn refsync_intent_and_create_staging_with<Ops: BootstrapFsOps>(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
        operations: &mut Ops,
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        if read_bootstrap_intent(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let intent_fd = openat(
            self.path.library_root_fd(),
            ".mengxia.bootstrap-intent",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let intent_file = File::from(intent_fd);
        inspect_internal_file_with_size(
            intent_file.as_fd(),
            self.path.owner_uid,
            Some(BOOTSTRAP_INTENT_RECORD_LENGTH as u64),
        )?;
        operations.sync_intent(&intent_file)?;
        operations.sync_root(self.path.library_root_fd())?;
        if read_bootstrap_intent(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        let staging_file = operations.create_staging(self.path.library_root_fd())?;
        let staging_security =
            inspect_internal_file_with_size(staging_file.as_fd(), self.path.owner_uid, Some(0))?;
        operations.sync_root(self.path.library_root_fd())?;

        self.path.revalidate_chain()?;
        if read_bootstrap_intent_with_staging(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let reopened = openat(
            self.path.library_root_fd(),
            ".library.sqlite3.bootstrap",
            OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let reopened_security =
            inspect_internal_file_with_size(reopened.as_fd(), self.path.owner_uid, Some(0))?;
        if !reopened_security.same_object(staging_security)
            || enumerate_root(&self.path)?
                != [
                    b".library.sqlite3.bootstrap".to_vec(),
                    b".mengxia.bootstrap-intent".to_vec(),
                    b".mengxia.lock".to_vec(),
                ]
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(())
    }

    fn create_durable_bootstrap_intent_with<Ops: BootstrapFsOps>(
        &self,
        record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
        operations: &mut Ops,
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        if enumerate_root(&self.path)? != [b".mengxia.lock".to_vec()] {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        let intent_file = operations.create_intent(self.path.library_root_fd())?;
        let initial_security =
            inspect_internal_file_with_size(intent_file.as_fd(), self.path.owner_uid, Some(0))?;
        let mut written = 0;
        while written < record.len() {
            let count = operations.write_intent(&intent_file, &record[written..])?;
            if count == 0 || count > record.len() - written {
                return Err(AuthorityError::Io);
            }
            written += count;
        }
        let written_security = inspect_internal_file_with_size(
            intent_file.as_fd(),
            self.path.owner_uid,
            Some(BOOTSTRAP_INTENT_RECORD_LENGTH as u64),
        )?;
        if !written_security.same_object(initial_security) {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        operations.sync_intent(&intent_file)?;
        operations.sync_root(self.path.library_root_fd())?;
        self.path.revalidate_chain()?;
        let reopened = openat(
            self.path.library_root_fd(),
            ".mengxia.bootstrap-intent",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let reopened_security = inspect_internal_file_with_size(
            reopened.as_fd(),
            self.path.owner_uid,
            Some(BOOTSTRAP_INTENT_RECORD_LENGTH as u64),
        )?;
        if !reopened_security.same_object(written_security)
            || enumerate_root(&self.path)?
                != [
                    b".mengxia.bootstrap-intent".to_vec(),
                    b".mengxia.lock".to_vec(),
                ]
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(())
    }

    fn cleanup_authorized_incomplete_staging_with<Ops: CleanupFsOps>(
        &self,
        expected_record: &[u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
        operations: &mut Ops,
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        let entries = enumerate_root(&self.path)?;
        if !is_intent_with_staging_entries(&entries)
            || read_bootstrap_intent_in_observed_state(&self.path, &entries)? != *expected_record
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        self.path
            .validate_sqlite_child(SqliteChild::BootstrapStaging)?;
        self.path
            .validate_sqlite_sidecars(SqliteChild::BootstrapStaging)?;

        if entries
            .iter()
            .any(|entry| entry == b".library.sqlite3.bootstrap-shm")
        {
            operations.unlink_staging_shm(self.path.library_root_fd())?;
        }
        if entries
            .iter()
            .any(|entry| entry == b".library.sqlite3.bootstrap-wal")
        {
            operations.unlink_staging_wal(self.path.library_root_fd())?;
        }
        operations.unlink_staging(self.path.library_root_fd())?;

        self.path.revalidate_chain()?;
        if read_bootstrap_intent(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        operations.sync_root(self.path.library_root_fd())?;
        if read_bootstrap_intent(&self.path)? != *expected_record {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        operations.unlink_intent(self.path.library_root_fd())?;

        self.path.revalidate_chain()?;
        if enumerate_root(&self.path)? != [b".mengxia.lock".to_vec()] {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        operations.sync_root(self.path.library_root_fd())?;
        self.path.revalidate_chain()?;
        if enumerate_root(&self.path)? != [b".mengxia.lock".to_vec()] {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        Ok(())
    }
}

trait BootstrapFsOps {
    fn create_intent(&mut self, root: BorrowedFd<'_>) -> Result<File, AuthorityError>;
    fn write_intent(&mut self, file: &File, bytes: &[u8]) -> Result<usize, AuthorityError>;
    fn sync_intent(&mut self, file: &File) -> Result<(), AuthorityError>;
    fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
    fn create_staging(&mut self, root: BorrowedFd<'_>) -> Result<File, AuthorityError>;
}

struct RealBootstrapFsOps;

impl BootstrapFsOps for RealBootstrapFsOps {
    fn create_intent(&mut self, root: BorrowedFd<'_>) -> Result<File, AuthorityError> {
        openat(
            root,
            ".mengxia.bootstrap-intent",
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map(File::from)
        .map_err(|error| match error {
            rustix::io::Errno::EXIST | rustix::io::Errno::LOOP => {
                AuthorityError::UnsafeConfiguration
            }
            _ => AuthorityError::Io,
        })
    }

    fn write_intent(&mut self, file: &File, bytes: &[u8]) -> Result<usize, AuthorityError> {
        write(file, bytes).map_err(|_| AuthorityError::Io)
    }

    fn sync_intent(&mut self, file: &File) -> Result<(), AuthorityError> {
        fsync(file).map_err(|_| AuthorityError::Io)
    }

    fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        fsync(root).map_err(|_| AuthorityError::Io)
    }

    fn create_staging(&mut self, root: BorrowedFd<'_>) -> Result<File, AuthorityError> {
        openat(
            root,
            ".library.sqlite3.bootstrap",
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map(File::from)
        .map_err(|error| match error {
            rustix::io::Errno::EXIST | rustix::io::Errno::LOOP => {
                AuthorityError::UnsafeConfiguration
            }
            _ => AuthorityError::Io,
        })
    }
}

trait PublishFsOps {
    fn link_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
    fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
    fn unlink_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
    fn unlink_intent(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
}

struct RealPublishFsOps;

impl PublishFsOps for RealPublishFsOps {
    fn link_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        linkat(
            root,
            ".library.sqlite3.bootstrap",
            root,
            "library.sqlite3",
            AtFlags::empty(),
        )
        .map_err(|error| match error {
            rustix::io::Errno::EXIST | rustix::io::Errno::LOOP => {
                AuthorityError::UnsafeConfiguration
            }
            _ => AuthorityError::Io,
        })
    }

    fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        fsync(root).map_err(|_| AuthorityError::Io)
    }

    fn unlink_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        unlinkat(root, ".library.sqlite3.bootstrap", AtFlags::empty()).map_err(
            |error| match error {
                rustix::io::Errno::NOENT => AuthorityError::UnsafeConfiguration,
                _ => AuthorityError::Io,
            },
        )
    }

    fn unlink_intent(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        unlinkat(root, ".mengxia.bootstrap-intent", AtFlags::empty()).map_err(|error| match error {
            rustix::io::Errno::NOENT => AuthorityError::UnsafeConfiguration,
            _ => AuthorityError::Io,
        })
    }
}

trait CleanupFsOps {
    fn unlink_staging_shm(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
    fn unlink_staging_wal(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
    fn unlink_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
    fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
    fn unlink_intent(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError>;
}

struct RealCleanupFsOps;

impl RealCleanupFsOps {
    fn unlink_fixed(root: BorrowedFd<'_>, name: &str) -> Result<(), AuthorityError> {
        unlinkat(root, name, AtFlags::empty()).map_err(|error| match error {
            rustix::io::Errno::NOENT => AuthorityError::UnsafeConfiguration,
            _ => AuthorityError::Io,
        })
    }
}

impl CleanupFsOps for RealCleanupFsOps {
    fn unlink_staging_shm(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        Self::unlink_fixed(root, ".library.sqlite3.bootstrap-shm")
    }

    fn unlink_staging_wal(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        Self::unlink_fixed(root, ".library.sqlite3.bootstrap-wal")
    }

    fn unlink_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        Self::unlink_fixed(root, ".library.sqlite3.bootstrap")
    }

    fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        fsync(root).map_err(|_| AuthorityError::Io)
    }

    fn unlink_intent(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
        Self::unlink_fixed(root, ".mengxia.bootstrap-intent")
    }
}

/// Fixed SQLite child selection accepted by TASK-004.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteChild {
    Canonical,
    BootstrapStaging,
}

/// Opaque reader for one descriptor-relative, owner-only SQLite WAL sidecar.
pub struct ValidatedSqliteWal {
    file: File,
}

impl ValidatedSqliteWal {
    /// Reads the next bytes from the fixed sidecar without exposing its path or
    /// underlying descriptor.
    pub fn read_chunk(&mut self, buffer: &mut [u8]) -> Result<usize, AuthorityError> {
        use std::io::Read;

        self.file.read(buffer).map_err(|_| AuthorityError::Io)
    }
}

/// Lifetime-bound, non-forgeable token for a fixed SQLite child path.
pub struct FixedSqliteChildPath<'authority> {
    path: &'authority Path,
}

impl AsRef<Path> for FixedSqliteChildPath<'_> {
    fn as_ref(&self) -> &Path {
        self.path
    }
}

fn validate_lexical_absolute_path(path: &Path) -> Result<(), AuthorityError> {
    let text = path.to_str().ok_or(AuthorityError::UnsafeConfiguration)?;
    if !path.is_absolute()
        || path == Path::new("/")
        || text.as_bytes().contains(&0)
        || text
            .split('/')
            .skip(1)
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let mut saw_root = false;
    let mut normal_count = 0usize;
    for component in path.components() {
        match component {
            Component::RootDir if !saw_root && normal_count == 0 => saw_root = true,
            Component::Normal(name) if saw_root && !name.is_empty() => normal_count += 1,
            _ => return Err(AuthorityError::UnsafeConfiguration),
        }
    }
    if !saw_root || normal_count == 0 {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(())
}

fn inspect_directory(fd: BorrowedFd<'_>) -> Result<MacOsObjectSecurity, AuthorityError> {
    let stat = fstat(fd).map_err(|_| AuthorityError::Io)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    inspect_security(
        fd,
        u64::try_from(stat.st_dev).map_err(|_| AuthorityError::UnsafeConfiguration)?,
        stat.st_ino as u64,
        stat.st_uid,
        stat.st_mode,
    )
}

fn inspect_internal_file(
    fd: BorrowedFd<'_>,
    owner_uid: u32,
) -> Result<MacOsObjectSecurity, AuthorityError> {
    inspect_internal_file_with_size(fd, owner_uid, None)
}

fn inspect_internal_file_with_size(
    fd: BorrowedFd<'_>,
    owner_uid: u32,
    expected_size: Option<u64>,
) -> Result<MacOsObjectSecurity, AuthorityError> {
    let stat = fstat(fd).map_err(|_| AuthorityError::Io)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let size = u64::try_from(stat.st_size).map_err(|_| AuthorityError::UnsafeConfiguration)?;
    if expected_size.is_some_and(|expected| size != expected) {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let security = inspect_security(
        fd,
        u64::try_from(stat.st_dev).map_err(|_| AuthorityError::UnsafeConfiguration)?,
        stat.st_ino as u64,
        stat.st_uid,
        stat.st_mode,
    )?;
    validate_internal_file_security(security, owner_uid)?;
    Ok(security)
}

fn validate_internal_file_security(
    security: MacOsObjectSecurity,
    owner_uid: u32,
) -> Result<(), AuthorityError> {
    if security.owner_uid != owner_uid || security.mode != 0o600 || !security.acl.is_empty() {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(())
}

fn read_bootstrap_intent(
    authority: &ValidatedAbsolutePath,
) -> Result<[u8; BOOTSTRAP_INTENT_RECORD_LENGTH], AuthorityError> {
    read_bootstrap_intent_in_state(authority, &[b".mengxia.bootstrap-intent", b".mengxia.lock"])
}

fn read_bootstrap_intent_with_staging(
    authority: &ValidatedAbsolutePath,
) -> Result<[u8; BOOTSTRAP_INTENT_RECORD_LENGTH], AuthorityError> {
    read_bootstrap_intent_in_state(
        authority,
        &[
            b".library.sqlite3.bootstrap",
            b".mengxia.bootstrap-intent",
            b".mengxia.lock",
        ],
    )
}

fn read_bootstrap_intent_with_published_staging(
    authority: &ValidatedAbsolutePath,
) -> Result<[u8; BOOTSTRAP_INTENT_RECORD_LENGTH], AuthorityError> {
    read_bootstrap_intent_in_state(
        authority,
        &[
            b".library.sqlite3.bootstrap",
            b".mengxia.bootstrap-intent",
            b".mengxia.lock",
            b"library.sqlite3",
        ],
    )
}

fn read_bootstrap_intent_with_canonical(
    authority: &ValidatedAbsolutePath,
) -> Result<[u8; BOOTSTRAP_INTENT_RECORD_LENGTH], AuthorityError> {
    read_bootstrap_intent_in_state(
        authority,
        &[
            b".mengxia.bootstrap-intent",
            b".mengxia.lock",
            b"library.sqlite3",
        ],
    )
}

fn read_bootstrap_intent_in_state(
    authority: &ValidatedAbsolutePath,
    expected_entries: &[&[u8]],
) -> Result<[u8; BOOTSTRAP_INTENT_RECORD_LENGTH], AuthorityError> {
    let expected_entries: Vec<Vec<u8>> = expected_entries
        .iter()
        .map(|entry| entry.to_vec())
        .collect();
    read_bootstrap_intent_in_observed_state(authority, &expected_entries)
}

fn read_bootstrap_intent_in_observed_state(
    authority: &ValidatedAbsolutePath,
    expected_entries: &[Vec<u8>],
) -> Result<[u8; BOOTSTRAP_INTENT_RECORD_LENGTH], AuthorityError> {
    let intent_fd = openat(
        authority.library_root_fd(),
        ".mengxia.bootstrap-intent",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let intent_file = File::from(intent_fd);
    let initial_security = inspect_internal_file_with_size(
        intent_file.as_fd(),
        authority.owner_uid,
        Some(BOOTSTRAP_INTENT_RECORD_LENGTH as u64),
    )?;
    let mut record = [0_u8; BOOTSTRAP_INTENT_RECORD_LENGTH];
    let mut read_length = 0;
    while read_length < record.len() {
        let count =
            read(&intent_file, &mut record[read_length..]).map_err(|_| AuthorityError::Io)?;
        if count == 0 || count > record.len() - read_length {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        read_length += count;
    }
    let mut trailing = [0_u8; 1];
    if read(&intent_file, &mut trailing).map_err(|_| AuthorityError::Io)? != 0 {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let read_security = inspect_internal_file_with_size(
        intent_file.as_fd(),
        authority.owner_uid,
        Some(BOOTSTRAP_INTENT_RECORD_LENGTH as u64),
    )?;
    if !read_security.same_object(initial_security) {
        return Err(AuthorityError::UnsafeConfiguration);
    }

    authority.revalidate_chain()?;
    let reopened = openat(
        authority.library_root_fd(),
        ".mengxia.bootstrap-intent",
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let reopened_security = inspect_internal_file_with_size(
        reopened.as_fd(),
        authority.owner_uid,
        Some(BOOTSTRAP_INTENT_RECORD_LENGTH as u64),
    )?;
    if !reopened_security.same_object(read_security)
        || enumerate_root(authority)? != expected_entries
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(record)
}

fn is_intent_with_staging_entries(entries: &[Vec<u8>]) -> bool {
    entries.len() >= 3
        && entries.len() <= 5
        && entries
            .iter()
            .any(|entry| entry == b".library.sqlite3.bootstrap")
        && entries
            .iter()
            .any(|entry| entry == b".mengxia.bootstrap-intent")
        && entries.iter().any(|entry| entry == b".mengxia.lock")
        && entries.iter().all(|entry| {
            matches!(
                entry.as_slice(),
                b".library.sqlite3.bootstrap"
                    | b".library.sqlite3.bootstrap-shm"
                    | b".library.sqlite3.bootstrap-wal"
                    | b".mengxia.bootstrap-intent"
                    | b".mengxia.lock"
            )
        })
}

fn enumerate_root(authority: &ValidatedAbsolutePath) -> Result<Vec<Vec<u8>>, AuthorityError> {
    let directory = Dir::read_from(authority.library_root_fd()).map_err(|_| AuthorityError::Io)?;
    let mut names = Vec::new();
    for entry in directory {
        let entry = entry.map_err(|_| AuthorityError::Io)?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            names.push(name.to_vec());
        }
    }
    names.sort_unstable();
    Ok(names)
}

fn inspect_security(
    fd: BorrowedFd<'_>,
    device: u64,
    inode: u64,
    owner_uid: u32,
    raw_mode: u16,
) -> Result<MacOsObjectSecurity, AuthorityError> {
    let filesystem = fstatfs(fd).map_err(|_| AuthorityError::Io)?;
    validate_filesystem_evidence(
        filesystem.f_flags,
        filesystem_name(&filesystem.f_fstypename) == b"apfs",
    )?;
    let acl = macos_ffi::inspect(fd)?;
    Ok(MacOsObjectSecurity {
        device,
        inode,
        owner_uid,
        mode: u32::from(Mode::from_raw_mode(raw_mode).as_raw_mode()),
        acl,
    })
}

fn validate_filesystem_evidence(
    filesystem_flags: u32,
    is_apfs: bool,
) -> Result<(), AuthorityError> {
    if filesystem_flags & MNT_LOCAL == 0 || filesystem_flags & MNT_IGNORE_OWNERSHIP != 0 || !is_apfs
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(())
}

fn filesystem_name(raw: &[i8]) -> &[u8] {
    let length = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    if length == 4
        && raw[0] as u8 == b'a'
        && raw[1] as u8 == b'p'
        && raw[2] as u8 == b'f'
        && raw[3] as u8 == b's'
    {
        b"apfs"
    } else {
        b""
    }
}

fn validate_component_policy(
    security: MacOsObjectSecurity,
    role: ComponentRole,
    effective_uid: u32,
) -> Result<(), AuthorityError> {
    match role {
        ComponentRole::Ancestor => {
            if (security.owner_uid != 0 && security.owner_uid != effective_uid)
                || security.mode & 0o022 != 0
                || !security.acl.permits_prefix()
            {
                return Err(AuthorityError::UnsafeConfiguration);
            }
        }
        ComponentRole::FinalParent => {
            if security.owner_uid != effective_uid
                || security.mode & 0o700 != 0o700
                || security.mode & 0o022 != 0
                || !security.acl.permits_prefix()
                || security.acl.inheritable_count != 0
            {
                return Err(AuthorityError::UnsafeConfiguration);
            }
        }
        ComponentRole::LibraryRoot => {
            if security.owner_uid != effective_uid
                || security.mode != 0o700
                || !security.acl.is_empty()
            {
                return Err(AuthorityError::UnsafeConfiguration);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::fd::BorrowedFd;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        AclSummary, AuthorityError, BOOTSTRAP_INTENT_RECORD_LENGTH, BootstrapFilesystemState,
        BootstrapFsOps, CleanupFsOps, ComponentRole, MNT_IGNORE_OWNERSHIP, MNT_LOCAL,
        MacOsObjectSecurity, OpenedLibraryAuthority, PublishFsOps, RealBootstrapFsOps,
        RealCleanupFsOps, RealPublishFsOps, SqliteChild, ValidatedAbsolutePath,
        validate_component_policy, validate_filesystem_evidence, validate_internal_file_security,
    };

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        base: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|path| path.parent())
                .expect("crate is inside workspace")
                .to_path_buf();
            let common = repository.join("target/task-004-path-tests");
            fs::create_dir_all(&common).expect("create fixture parent");
            fs::set_permissions(&common, fs::Permissions::from_mode(0o700))
                .expect("secure fixture parent");
            let unique = format!(
                "{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            );
            let base = common.join(unique);
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&base)
                .expect("create secure fixture base");
            Self { base }
        }

        fn library(&self, name: &str) -> PathBuf {
            let library = self.base.join(name);
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&library)
                .expect("create secure Library root");
            library
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn safe_chain_and_fixed_children_validate_and_revalidate() {
        let fixture = Fixture::new();
        let library = fixture.library("Library");
        let authority =
            ValidatedAbsolutePath::authorize_existing(&library).expect("safe authority");
        authority.revalidate_chain().expect("stable edge chain");
        assert_eq!(
            authority.sqlite_child(SqliteChild::Canonical).as_ref(),
            library.join("library.sqlite3")
        );
        assert_eq!(
            authority
                .sqlite_child(SqliteChild::BootstrapStaging)
                .as_ref(),
            library.join(".library.sqlite3.bootstrap")
        );
    }

    #[test]
    fn absent_and_existing_empty_roots_acquire_one_durable_lock() {
        let fixture = Fixture::new();
        let absent = fixture.base.join("AbsentLibrary");
        let opened = OpenedLibraryAuthority::acquire_bootstrap(&absent)
            .expect("create absent Library and lock it");
        assert_eq!(opened.owner_uid(), rustix::process::geteuid().as_raw());
        assert_eq!(
            fs::metadata(&absent)
                .expect("created Library metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(absent.join(".mengxia.lock"))
                .expect("created lock metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        drop(opened);

        let reopened = OpenedLibraryAuthority::acquire_bootstrap(&absent)
            .expect("reuse the durable unlocked lock");
        reopened
            .path_authority()
            .revalidate_chain()
            .expect("reopened root remains stable");
        drop(reopened);

        let empty = fixture.library("ExistingEmptyLibrary");
        OpenedLibraryAuthority::acquire_bootstrap(&empty)
            .expect("existing safe empty Library creates its one lock");
    }

    #[test]
    fn content_without_lock_fails_without_creating_a_replacement_lock() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let fixture = Fixture::new();
        for (case, name) in [
            ("UnknownContent", "unknown-entry"),
            ("CanonicalWithoutLock", "library.sqlite3"),
            ("IntentWithoutLock", ".mengxia.bootstrap-intent"),
        ] {
            let library = fixture.library(case);
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(library.join(name))
                .expect("create pre-existing content");
            assert!(matches!(
                OpenedLibraryAuthority::acquire_bootstrap(&library),
                Err(AuthorityError::UnsafeConfiguration)
            ));
            assert!(
                !library.join(".mengxia.lock").exists(),
                "missing lock must never be recreated beside {name}"
            );
        }
    }

    #[test]
    fn unsafe_existing_lock_type_or_mode_fails_closed() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let fixture = Fixture::new();
        let wrong_mode = fixture.library("WrongModeLock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(wrong_mode.join(".mengxia.lock"))
            .expect("create unsafe lock mode");
        assert!(matches!(
            OpenedLibraryAuthority::acquire_bootstrap(&wrong_mode),
            Err(AuthorityError::UnsafeConfiguration)
        ));

        let symlink_lock = fixture.library("SymlinkLock");
        let target = fixture.base.join("LockTarget");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&target)
            .expect("create symlink target");
        symlink(&target, symlink_lock.join(".mengxia.lock")).expect("create unsafe lock symlink");
        assert!(matches!(
            OpenedLibraryAuthority::acquire_bootstrap(&symlink_lock),
            Err(AuthorityError::UnsafeConfiguration)
        ));

        let acl_lock = fixture.library("AclLock");
        let acl_lock_path = acl_lock.join(".mengxia.lock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&acl_lock_path)
            .expect("create lock with safe base mode");
        chmod_acl(&acl_lock_path, &["+a", "everyone deny read"]);
        assert!(matches!(
            OpenedLibraryAuthority::acquire_bootstrap(&acl_lock),
            Err(AuthorityError::UnsafeConfiguration)
        ));
    }

    #[test]
    fn live_lock_contends_across_processes() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let fixture = Fixture::new();
        let library = fixture.base.join("ContendedLibrary");
        let holder = OpenedLibraryAuthority::acquire_bootstrap(&library)
            .expect("parent process acquires lock");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(library.join("library.sqlite3"))
            .expect("create content while the live lock is held");
        let status = Command::new(std::env::current_exe().expect("current test executable"))
            .args([
                "--exact",
                "tests::lock_contention_child_process_entrypoint",
                "--nocapture",
            ])
            .env("MENGXIA_TASK004_LOCK_CHILD", &library)
            .status()
            .expect("start lock contender subprocess");
        assert!(status.success(), "lock contender subprocess failed");

        drop(holder);
        assert!(matches!(
            OpenedLibraryAuthority::acquire_bootstrap(&library),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        assert!(library.join(".mengxia.lock").is_file());
        assert!(library.join("library.sqlite3").is_file());
    }

    #[test]
    fn lock_contention_child_process_entrypoint() {
        let Some(library) = std::env::var_os("MENGXIA_TASK004_LOCK_CHILD") else {
            return;
        };
        assert!(matches!(
            OpenedLibraryAuthority::acquire_bootstrap(std::path::Path::new(&library)),
            Err(AuthorityError::Contended)
        ));
    }

    #[test]
    fn durable_intent_short_writes_follow_exact_create_write_sync_order() {
        let fixture = Fixture::new();
        let library = fixture.base.join("DurableIntentLibrary");
        let authority = OpenedLibraryAuthority::acquire_bootstrap(&library)
            .expect("acquire lock-only authority");
        let record = intent_record();
        let mut operations = TracingBootstrapFsOps::new(37, None);
        authority
            .create_durable_bootstrap_intent_with(&record, &mut operations)
            .expect("durably create intent through forced short writes");

        assert_eq!(operations.events.first(), Some(&FsEvent::Create));
        assert_eq!(
            operations.events[operations.events.len() - 2..],
            [FsEvent::SyncIntent, FsEvent::SyncRoot]
        );
        assert_eq!(
            operations
                .events
                .iter()
                .filter_map(|event| match event {
                    FsEvent::Write(count) => Some(*count),
                    _ => None,
                })
                .sum::<usize>(),
            BOOTSTRAP_INTENT_RECORD_LENGTH
        );
        assert_eq!(
            fs::read(library.join(".mengxia.bootstrap-intent"))
                .expect("read durable intent fixture"),
            record
        );
        drop(authority);
        let (reopened, state) = OpenedLibraryAuthority::acquire_bootstrap_state(&library)
            .expect("reopen lock plus exact intent");
        assert_eq!(state.intent_record(), Some(&record));
        drop(reopened);
    }

    #[test]
    fn every_intent_io_failure_preserves_the_exact_returned_prefix() {
        for (case, maximum_write, failure) in [
            ("CreateFailure", 256, FsFailure::Create),
            ("FirstWriteFailure", 64, FsFailure::WriteCall(1)),
            ("LaterWriteFailure", 64, FsFailure::WriteCall(2)),
            ("IntentSyncFailure", 256, FsFailure::SyncIntent),
            ("RootSyncFailure", 256, FsFailure::SyncRootCall(1)),
        ] {
            let fixture = Fixture::new();
            let library = fixture.base.join(case);
            let authority = OpenedLibraryAuthority::acquire_bootstrap(&library)
                .expect("acquire fault fixture authority");
            let record = intent_record();
            let mut operations = TracingBootstrapFsOps::new(maximum_write, Some(failure));
            assert_eq!(
                authority.create_durable_bootstrap_intent_with(&record, &mut operations),
                Err(AuthorityError::Io),
                "case {case}"
            );

            let intent_path = library.join(".mengxia.bootstrap-intent");
            if failure == FsFailure::Create {
                assert!(!intent_path.exists());
            } else {
                let observed = fs::read(&intent_path).expect("created intent prefix is preserved");
                let returned_prefix: usize = operations
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        FsEvent::Write(count) => Some(*count),
                        _ => None,
                    })
                    .sum();
                assert_eq!(observed, record[..returned_prefix], "case {case}");
            }
            assert!(library.join(".mengxia.lock").is_file());
            assert!(!library.join(".library.sqlite3.bootstrap").exists());
            assert!(!library.join("library.sqlite3").exists());
        }
    }

    #[test]
    fn valid_intent_is_resynced_before_exclusive_empty_staging_creation() {
        let fixture = Fixture::new();
        let library = fixture.base.join("StagingOrderLibrary");
        let authority = OpenedLibraryAuthority::acquire_bootstrap(&library)
            .expect("acquire staging-order authority");
        let record = intent_record();
        authority
            .create_durable_bootstrap_intent(&record)
            .expect("create valid durable intent");

        let mut operations = TracingBootstrapFsOps::new(256, None);
        authority
            .refsync_intent_and_create_staging_with(&record, &mut operations)
            .expect("re-sync intent and durably create staging");
        assert_eq!(
            operations.events,
            [
                FsEvent::SyncIntent,
                FsEvent::SyncRoot,
                FsEvent::CreateStaging,
                FsEvent::SyncRoot,
            ]
        );
        let staging = fs::metadata(library.join(".library.sqlite3.bootstrap"))
            .expect("created staging metadata");
        assert_eq!(staging.len(), 0);
        assert_eq!(staging.permissions().mode() & 0o777, 0o600);
        assert_eq!(
            fs::read(library.join(".mengxia.bootstrap-intent"))
                .expect("intent remains exact after staging creation"),
            record
        );
        assert!(!library.join("library.sqlite3").exists());
    }

    #[test]
    fn every_staging_transition_io_failure_preserves_a_closed_recovery_state() {
        for (case, failure, expected_events, staging_exists) in [
            (
                "StagingIntentSyncFailure",
                FsFailure::SyncIntent,
                Vec::new(),
                false,
            ),
            (
                "StagingFirstRootSyncFailure",
                FsFailure::SyncRootCall(1),
                vec![FsEvent::SyncIntent],
                false,
            ),
            (
                "StagingCreateFailure",
                FsFailure::CreateStaging,
                vec![FsEvent::SyncIntent, FsEvent::SyncRoot],
                false,
            ),
            (
                "StagingSecondRootSyncFailure",
                FsFailure::SyncRootCall(2),
                vec![
                    FsEvent::SyncIntent,
                    FsEvent::SyncRoot,
                    FsEvent::CreateStaging,
                ],
                true,
            ),
        ] {
            let fixture = Fixture::new();
            let library = fixture.base.join(case);
            let authority = OpenedLibraryAuthority::acquire_bootstrap(&library)
                .expect("acquire staging fault authority");
            let record = intent_record();
            authority
                .create_durable_bootstrap_intent(&record)
                .expect("create valid durable intent");
            let mut operations = TracingBootstrapFsOps::new(256, Some(failure));

            assert_eq!(
                authority.refsync_intent_and_create_staging_with(&record, &mut operations),
                Err(AuthorityError::Io),
                "case {case}"
            );
            assert_eq!(operations.events, expected_events, "case {case}");
            assert_eq!(
                library.join(".library.sqlite3.bootstrap").exists(),
                staging_exists,
                "case {case}"
            );
            if staging_exists {
                let metadata = fs::metadata(library.join(".library.sqlite3.bootstrap"))
                    .expect("preserved empty staging metadata");
                assert_eq!(metadata.len(), 0, "case {case}");
                assert_eq!(metadata.permissions().mode() & 0o777, 0o600, "case {case}");
            }
            assert_eq!(
                fs::read(library.join(".mengxia.bootstrap-intent"))
                    .expect("valid intent remains exact"),
                record,
                "case {case}"
            );
            assert!(library.join(".mengxia.lock").is_file(), "case {case}");
            assert!(!library.join("library.sqlite3").exists(), "case {case}");
        }
    }

    #[test]
    fn preexisting_staging_is_never_overwritten_or_recreated() {
        use std::fs::OpenOptions;
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;

        let fixture = Fixture::new();
        let library = fixture.base.join("PreexistingStagingLibrary");
        let authority = OpenedLibraryAuthority::acquire_bootstrap(&library)
            .expect("acquire preexisting-staging authority");
        let record = intent_record();
        authority
            .create_durable_bootstrap_intent(&record)
            .expect("create valid durable intent");
        let staging_path = library.join(".library.sqlite3.bootstrap");
        let mut staging = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staging_path)
            .expect("create preexisting staging fixture");
        staging
            .write_all(b"user-preserved")
            .expect("populate preexisting staging fixture");
        drop(staging);

        assert_eq!(
            authority.refsync_intent_and_create_staging(&record),
            Err(AuthorityError::UnsafeConfiguration)
        );
        assert_eq!(
            fs::read(&staging_path).expect("preexisting staging is preserved"),
            b"user-preserved"
        );
        assert!(!library.join("library.sqlite3").exists());
    }

    #[test]
    fn intent_reopen_rejects_length_mode_and_acl_without_mutation() {
        use std::fs::OpenOptions;

        for (case, length) in [("TruncatedIntent", 255), ("OversizedIntent", 257)] {
            let fixture = Fixture::new();
            let library = fixture.base.join(case);
            let authority = OpenedLibraryAuthority::acquire_bootstrap(&library)
                .expect("acquire intent length fixture");
            authority
                .create_durable_bootstrap_intent(&intent_record())
                .expect("create valid intent before length mutation");
            drop(authority);
            OpenOptions::new()
                .write(true)
                .open(library.join(".mengxia.bootstrap-intent"))
                .expect("open intent length fixture")
                .set_len(length)
                .expect("change intent fixture length");
            assert!(matches!(
                OpenedLibraryAuthority::acquire_bootstrap_state(&library),
                Err(AuthorityError::UnsafeConfiguration)
            ));
            assert_eq!(
                fs::metadata(library.join(".mengxia.bootstrap-intent"))
                    .expect("preserved length fixture")
                    .len(),
                length
            );
        }

        let fixture = Fixture::new();
        let library = fixture.base.join("UnsafeIntentMetadata");
        let authority = OpenedLibraryAuthority::acquire_bootstrap(&library)
            .expect("acquire intent metadata fixture");
        authority
            .create_durable_bootstrap_intent(&intent_record())
            .expect("create valid intent before metadata mutation");
        drop(authority);
        let intent_path = library.join(".mengxia.bootstrap-intent");
        fs::set_permissions(&intent_path, fs::Permissions::from_mode(0o644))
            .expect("make intent mode unsafe");
        assert!(matches!(
            OpenedLibraryAuthority::acquire_bootstrap_state(&library),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        assert_eq!(
            fs::metadata(&intent_path)
                .expect("preserved unsafe mode fixture")
                .permissions()
                .mode()
                & 0o777,
            0o644
        );

        fs::set_permissions(&intent_path, fs::Permissions::from_mode(0o600))
            .expect("restore base mode for ACL fixture");
        chmod_acl(&intent_path, &["+a", "everyone deny read"]);
        assert!(matches!(
            OpenedLibraryAuthority::acquire_bootstrap_state(&library),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        assert!(intent_path.is_file());
    }

    #[test]
    fn symlink_and_wrong_root_mode_fail_closed() {
        let fixture = Fixture::new();
        let library = fixture.library("RealLibrary");
        let link = fixture.base.join("LinkedLibrary");
        symlink(&library, &link).expect("create symlink fixture");
        assert!(matches!(
            ValidatedAbsolutePath::authorize_existing(&link),
            Err(AuthorityError::UnsafeConfiguration)
        ));

        fs::set_permissions(&library, fs::Permissions::from_mode(0o755))
            .expect("change fixture mode");
        assert!(matches!(
            ValidatedAbsolutePath::authorize_existing(&library),
            Err(AuthorityError::UnsafeConfiguration)
        ));
    }

    #[test]
    fn intermediate_symlink_non_directory_and_noncanonical_text_fail_before_root() {
        use std::fs::OpenOptions;

        let fixture = Fixture::new();
        let real_parent = fixture.base.join("RealParent");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&real_parent)
            .expect("create real intermediate parent");
        let library = real_parent.join("Library");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&library)
            .expect("create nested Library root");
        let alias = fixture.base.join("AliasParent");
        symlink(&real_parent, &alias).expect("create intermediate symlink");
        assert!(matches!(
            ValidatedAbsolutePath::authorize_existing(&alias.join("Library")),
            Err(AuthorityError::UnsafeConfiguration)
        ));

        let regular = fixture.base.join("NotDirectory");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&regular)
            .expect("create non-directory intermediate");
        assert!(matches!(
            ValidatedAbsolutePath::authorize_existing(&regular.join("Library")),
            Err(AuthorityError::UnsafeConfiguration)
        ));

        let noncanonical = PathBuf::from(format!("{}//Library", real_parent.display()));
        assert!(matches!(
            ValidatedAbsolutePath::authorize_existing(&noncanonical),
            Err(AuthorityError::UnsafeConfiguration)
        ));
    }

    #[test]
    fn every_mutable_absolute_prefix_depth_rejects_symlink_or_non_directory() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        for replacement_kind in ["symlink", "regular-file"] {
            for depth in 0..3 {
                let fixture = Fixture::new();
                let components = ["LevelOne", "LevelTwo", "LevelThree"];
                let mut current = fixture.base.clone();
                let mut prefixes = Vec::new();
                for component in components {
                    current.push(component);
                    fs::DirBuilder::new()
                        .mode(0o700)
                        .create(&current)
                        .expect("create nested prefix component");
                    prefixes.push(current.clone());
                }
                let library = current.join("Library");
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&library)
                    .expect("create nested Library root");

                let selected = &prefixes[depth];
                let displaced = fixture.base.join(format!("Displaced-{depth}"));
                fs::rename(selected, &displaced).expect("displace selected prefix inode");
                match replacement_kind {
                    "symlink" => {
                        symlink(&displaced, selected).expect("replace prefix with symlink")
                    }
                    "regular-file" => {
                        OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .mode(0o600)
                            .open(selected)
                            .expect("replace prefix with regular file");
                    }
                    _ => unreachable!(),
                }

                assert!(
                    matches!(
                        ValidatedAbsolutePath::authorize_existing(&library),
                        Err(AuthorityError::UnsafeConfiguration)
                    ),
                    "{replacement_kind} at mutable prefix depth {depth}"
                );
            }
        }
    }

    #[test]
    fn fixed_sqlite_child_requires_regular_owner_only_empty_acl_file() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let fixture = Fixture::new();
        let library = fixture.library("Library");
        let database = library.join("library.sqlite3");
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&database)
            .expect("create secure database fixture");
        let authority =
            ValidatedAbsolutePath::authorize_existing(&library).expect("safe authority");
        authority
            .validate_sqlite_child(SqliteChild::Canonical)
            .expect("secure database child");

        fs::set_permissions(&database, fs::Permissions::from_mode(0o644))
            .expect("make database fixture unsafe");
        assert_eq!(
            authority.validate_sqlite_child(SqliteChild::Canonical),
            Err(AuthorityError::UnsafeConfiguration)
        );
    }

    #[test]
    fn sqlite_sidecars_must_be_owner_only_regular_files() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let fixture = Fixture::new();
        let library = fixture.library("SidecarLibrary");
        for name in [
            ".library.sqlite3.bootstrap",
            ".library.sqlite3.bootstrap-wal",
        ] {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(library.join(name))
                .expect("create secure SQLite fixture");
        }
        let authority =
            ValidatedAbsolutePath::authorize_existing(&library).expect("safe authority");
        authority
            .validate_sqlite_sidecars(SqliteChild::BootstrapStaging)
            .expect("secure WAL sidecar");

        let wal = library.join(".library.sqlite3.bootstrap-wal");
        fs::set_permissions(&wal, fs::Permissions::from_mode(0o644)).expect("make WAL mode unsafe");
        assert_eq!(
            authority.validate_sqlite_sidecars(SqliteChild::BootstrapStaging),
            Err(AuthorityError::UnsafeConfiguration)
        );

        fs::remove_file(&wal).expect("remove unsafe WAL fixture");
        symlink(
            library.join(".library.sqlite3.bootstrap"),
            library.join(".library.sqlite3.bootstrap-shm"),
        )
        .expect("create SHM symlink fixture");
        assert_eq!(
            authority.validate_sqlite_sidecars(SqliteChild::BootstrapStaging),
            Err(AuthorityError::UnsafeConfiguration)
        );
    }

    #[test]
    fn verified_staging_is_hard_linked_and_cleaned_in_order() {
        let fixture = Fixture::new();
        let (library, authority, record) = prepared_publish(&fixture, "PublishLibrary");
        let staging_metadata =
            fs::metadata(library.join(".library.sqlite3.bootstrap")).expect("staging metadata");

        authority
            .publish_verified_staging(&record)
            .expect("ordered descriptor-relative publish");
        authority
            .sync_closed_canonical_database()
            .expect("closed canonical sync");

        let canonical_metadata =
            fs::metadata(library.join("library.sqlite3")).expect("canonical metadata");
        assert_eq!(staging_metadata.dev(), canonical_metadata.dev());
        assert_eq!(staging_metadata.ino(), canonical_metadata.ino());
        assert_eq!(root_names(&library), [".mengxia.lock", "library.sqlite3"]);
    }

    #[test]
    fn every_publish_failure_preserves_the_exact_completed_namespace_prefix() {
        for (case, failure, expected_events, expected_names) in [
            (
                "LinkFailure",
                PublishFailure::Link,
                vec![],
                vec![
                    ".library.sqlite3.bootstrap",
                    ".mengxia.bootstrap-intent",
                    ".mengxia.lock",
                ],
            ),
            (
                "FirstRootSyncFailure",
                PublishFailure::SyncRootCall(1),
                vec![PublishEvent::Link],
                vec![
                    ".library.sqlite3.bootstrap",
                    ".mengxia.bootstrap-intent",
                    ".mengxia.lock",
                    "library.sqlite3",
                ],
            ),
            (
                "StagingUnlinkFailure",
                PublishFailure::UnlinkStaging,
                vec![PublishEvent::Link, PublishEvent::SyncRoot],
                vec![
                    ".library.sqlite3.bootstrap",
                    ".mengxia.bootstrap-intent",
                    ".mengxia.lock",
                    "library.sqlite3",
                ],
            ),
            (
                "SecondRootSyncFailure",
                PublishFailure::SyncRootCall(2),
                vec![
                    PublishEvent::Link,
                    PublishEvent::SyncRoot,
                    PublishEvent::UnlinkStaging,
                ],
                vec![
                    ".mengxia.bootstrap-intent",
                    ".mengxia.lock",
                    "library.sqlite3",
                ],
            ),
            (
                "IntentUnlinkFailure",
                PublishFailure::UnlinkIntent,
                vec![
                    PublishEvent::Link,
                    PublishEvent::SyncRoot,
                    PublishEvent::UnlinkStaging,
                    PublishEvent::SyncRoot,
                ],
                vec![
                    ".mengxia.bootstrap-intent",
                    ".mengxia.lock",
                    "library.sqlite3",
                ],
            ),
            (
                "FinalRootSyncFailure",
                PublishFailure::SyncRootCall(3),
                vec![
                    PublishEvent::Link,
                    PublishEvent::SyncRoot,
                    PublishEvent::UnlinkStaging,
                    PublishEvent::SyncRoot,
                    PublishEvent::UnlinkIntent,
                ],
                vec![".mengxia.lock", "library.sqlite3"],
            ),
        ] {
            let fixture = Fixture::new();
            let (library, authority, record) = prepared_publish(&fixture, case);
            let mut operations = TracingPublishFsOps::new(failure);

            assert_eq!(
                authority.publish_verified_staging_with(&record, &mut operations),
                Err(AuthorityError::Io),
                "{case}"
            );
            assert_eq!(operations.events, expected_events, "{case}");
            assert_eq!(root_names(&library), expected_names, "{case}");
        }
    }

    #[test]
    fn exact_staging_sidecar_set_reopens_and_cleans_to_lock_only() {
        let fixture = Fixture::new();
        let (library, authority, record) = prepared_cleanup(&fixture, "SidecarCleanup");
        drop(authority);

        let (reopened, state) = OpenedLibraryAuthority::acquire_bootstrap_state(&library)
            .expect("recognize exact staging sidecar set");
        assert_eq!(state, BootstrapFilesystemState::IntentWithStaging(record));
        reopened
            .cleanup_authorized_incomplete_staging(&record)
            .expect("clean exact intent-authorized staging set");
        assert_eq!(root_names(&library), [".mengxia.lock"]);
    }

    #[test]
    fn every_cleanup_failure_preserves_the_exact_completed_namespace_prefix() {
        for (case, failure, expected_events, expected_names) in [
            (
                "CleanupShmFailure",
                CleanupFailure::UnlinkShm,
                vec![],
                vec![
                    ".library.sqlite3.bootstrap",
                    ".library.sqlite3.bootstrap-shm",
                    ".library.sqlite3.bootstrap-wal",
                    ".mengxia.bootstrap-intent",
                    ".mengxia.lock",
                ],
            ),
            (
                "CleanupWalFailure",
                CleanupFailure::UnlinkWal,
                vec![CleanupEvent::UnlinkShm],
                vec![
                    ".library.sqlite3.bootstrap",
                    ".library.sqlite3.bootstrap-wal",
                    ".mengxia.bootstrap-intent",
                    ".mengxia.lock",
                ],
            ),
            (
                "CleanupStagingFailure",
                CleanupFailure::UnlinkStaging,
                vec![CleanupEvent::UnlinkShm, CleanupEvent::UnlinkWal],
                vec![
                    ".library.sqlite3.bootstrap",
                    ".mengxia.bootstrap-intent",
                    ".mengxia.lock",
                ],
            ),
            (
                "CleanupFirstSyncFailure",
                CleanupFailure::SyncRootCall(1),
                vec![
                    CleanupEvent::UnlinkShm,
                    CleanupEvent::UnlinkWal,
                    CleanupEvent::UnlinkStaging,
                ],
                vec![".mengxia.bootstrap-intent", ".mengxia.lock"],
            ),
            (
                "CleanupIntentFailure",
                CleanupFailure::UnlinkIntent,
                vec![
                    CleanupEvent::UnlinkShm,
                    CleanupEvent::UnlinkWal,
                    CleanupEvent::UnlinkStaging,
                    CleanupEvent::SyncRoot,
                ],
                vec![".mengxia.bootstrap-intent", ".mengxia.lock"],
            ),
            (
                "CleanupFinalSyncFailure",
                CleanupFailure::SyncRootCall(2),
                vec![
                    CleanupEvent::UnlinkShm,
                    CleanupEvent::UnlinkWal,
                    CleanupEvent::UnlinkStaging,
                    CleanupEvent::SyncRoot,
                    CleanupEvent::UnlinkIntent,
                ],
                vec![".mengxia.lock"],
            ),
        ] {
            let fixture = Fixture::new();
            let (library, authority, record) = prepared_cleanup(&fixture, case);
            let mut operations = TracingCleanupFsOps::new(failure);

            assert_eq!(
                authority.cleanup_authorized_incomplete_staging_with(&record, &mut operations),
                Err(AuthorityError::Io),
                "{case}"
            );
            assert_eq!(operations.events, expected_events, "{case}");
            assert_eq!(root_names(&library), expected_names, "{case}");
        }
    }

    #[test]
    fn revalidation_detects_name_to_inode_replacement() {
        let fixture = Fixture::new();
        let library = fixture.library("Library");
        let displaced = fixture.base.join("DisplacedLibrary");
        let authority =
            ValidatedAbsolutePath::authorize_existing(&library).expect("safe authority");
        fs::rename(&library, &displaced).expect("displace authorized inode");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&library)
            .expect("create replacement inode");
        assert_eq!(
            authority.revalidate_chain(),
            Err(AuthorityError::UnsafeConfiguration)
        );
    }

    #[test]
    fn real_acl_policy_accepts_deny_only_and_rejects_allow_or_inheritance() {
        let fixture = Fixture::new();
        let library = fixture.library("Library");

        chmod_acl(&fixture.base, &["+a", "everyone deny delete"]);
        ValidatedAbsolutePath::authorize_existing(&library)
            .expect("non-inheritable deny-only final-parent ACL is safe");

        chmod_acl(&fixture.base, &["-N"]);
        chmod_acl(&fixture.base, &["+a", "everyone allow delete"]);
        assert!(matches!(
            ValidatedAbsolutePath::authorize_existing(&library),
            Err(AuthorityError::UnsafeConfiguration)
        ));

        chmod_acl(&fixture.base, &["-N"]);
        chmod_acl(&fixture.base, &["+a", "everyone deny delete,file_inherit"]);
        assert!(matches!(
            ValidatedAbsolutePath::authorize_existing(&library),
            Err(AuthorityError::UnsafeConfiguration)
        ));
    }

    #[test]
    fn synthetic_filesystem_and_component_security_matrix_fails_closed() {
        let empty_acl = AclSummary::validated(0, 0, 0, 0, 0, 0, 0);
        for (case, flags, is_apfs) in [
            ("non-local", 0, true),
            ("ignore-ownership", MNT_LOCAL | MNT_IGNORE_OWNERSHIP, true),
            ("non-apfs", MNT_LOCAL, false),
        ] {
            assert_eq!(
                validate_filesystem_evidence(flags, is_apfs),
                Err(AuthorityError::UnsafeConfiguration),
                "case {case}"
            );
        }

        let deny_only_acl = AclSummary::validated(1, 0, 1, 0, 0, 0, 64);
        let allow_acl = AclSummary::validated(1, 1, 0, 0, 0, 0, 64);
        let defer_acl = AclSummary::validated(0, 0, 0, 1, 0, 0, 64);
        let inheritable_deny_acl = AclSummary::validated(1, 0, 1, 0, 2, 1, 64);
        let effective_uid = 501;
        let evidence = |owner_uid, mode, acl| MacOsObjectSecurity {
            device: 1,
            inode: 2,
            owner_uid,
            mode,
            acl,
        };

        assert_eq!(
            validate_component_policy(
                evidence(effective_uid, 0o700, deny_only_acl),
                ComponentRole::FinalParent,
                effective_uid,
            ),
            Ok(())
        );
        for (case, security, role) in [
            (
                "ancestor-owner",
                evidence(effective_uid + 1, 0o755, empty_acl),
                ComponentRole::Ancestor,
            ),
            (
                "ancestor-mode",
                evidence(effective_uid, 0o775, empty_acl),
                ComponentRole::Ancestor,
            ),
            (
                "ancestor-allow-acl",
                evidence(effective_uid, 0o755, allow_acl),
                ComponentRole::Ancestor,
            ),
            (
                "ancestor-defer-inherit-acl",
                evidence(effective_uid, 0o755, defer_acl),
                ComponentRole::Ancestor,
            ),
            (
                "final-parent-inheritable-acl",
                evidence(effective_uid, 0o700, inheritable_deny_acl),
                ComponentRole::FinalParent,
            ),
            (
                "library-root-owner",
                evidence(effective_uid + 1, 0o700, empty_acl),
                ComponentRole::LibraryRoot,
            ),
            (
                "library-root-mode",
                evidence(effective_uid, 0o750, empty_acl),
                ComponentRole::LibraryRoot,
            ),
            (
                "library-root-acl",
                evidence(effective_uid, 0o700, deny_only_acl),
                ComponentRole::LibraryRoot,
            ),
            (
                "library-root-object-flag",
                evidence(effective_uid, 0o700, defer_acl),
                ComponentRole::LibraryRoot,
            ),
        ] {
            assert_eq!(
                validate_component_policy(security, role, effective_uid),
                Err(AuthorityError::UnsafeConfiguration),
                "case {case}"
            );
        }

        assert_eq!(
            validate_internal_file_security(
                evidence(effective_uid, 0o600, empty_acl),
                effective_uid,
            ),
            Ok(())
        );
        for (case, security) in [
            (
                "internal-owner",
                evidence(effective_uid + 1, 0o600, empty_acl),
            ),
            ("internal-mode", evidence(effective_uid, 0o640, empty_acl)),
            (
                "internal-entry-acl",
                evidence(effective_uid, 0o600, deny_only_acl),
            ),
            (
                "internal-object-flag",
                evidence(effective_uid, 0o600, defer_acl),
            ),
        ] {
            assert_eq!(
                validate_internal_file_security(security, effective_uid),
                Err(AuthorityError::UnsafeConfiguration),
                "case {case}"
            );
        }
    }

    fn chmod_acl(path: &std::path::Path, arguments: &[&str]) {
        let status = Command::new("/bin/chmod")
            .args(arguments)
            .arg(path)
            .status()
            .expect("start the platform ACL fixture command");
        assert!(status.success(), "platform ACL fixture command failed");
    }

    fn intent_record() -> [u8; BOOTSTRAP_INTENT_RECORD_LENGTH] {
        core::array::from_fn(|index| index as u8)
    }

    fn prepared_publish(
        fixture: &Fixture,
        case: &str,
    ) -> (
        PathBuf,
        OpenedLibraryAuthority,
        [u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) {
        let library = fixture.library(case);
        let authority =
            OpenedLibraryAuthority::acquire_bootstrap(&library).expect("publish authority");
        let record = intent_record();
        authority
            .create_durable_bootstrap_intent(&record)
            .expect("durable publish intent");
        authority
            .refsync_intent_and_create_staging(&record)
            .expect("durable empty staging");
        fs::write(
            library.join(".library.sqlite3.bootstrap"),
            b"verified staging bytes",
        )
        .expect("write staging fixture");
        authority
            .sync_closed_staging_database(&record)
            .expect("sync staging fixture");
        (library, authority, record)
    }

    fn prepared_cleanup(
        fixture: &Fixture,
        case: &str,
    ) -> (
        PathBuf,
        OpenedLibraryAuthority,
        [u8; BOOTSTRAP_INTENT_RECORD_LENGTH],
    ) {
        let library = fixture.library(case);
        let authority =
            OpenedLibraryAuthority::acquire_bootstrap(&library).expect("cleanup authority");
        let record = intent_record();
        authority
            .create_durable_bootstrap_intent(&record)
            .expect("durable cleanup intent");
        authority
            .refsync_intent_and_create_staging(&record)
            .expect("durable cleanup staging");
        for name in [
            ".library.sqlite3.bootstrap-shm",
            ".library.sqlite3.bootstrap-wal",
        ] {
            let path = library.join(name);
            fs::write(&path, b"recognized sidecar fixture").expect("write cleanup sidecar");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .expect("secure cleanup sidecar");
        }
        (library, authority, record)
    }

    fn root_names(library: &std::path::Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(library)
            .expect("enumerate fixture root")
            .map(|entry| {
                entry
                    .expect("fixture entry")
                    .file_name()
                    .into_string()
                    .expect("ASCII fixture name")
            })
            .collect();
        names.sort_unstable();
        names
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum PublishFailure {
        Link,
        SyncRootCall(usize),
        UnlinkStaging,
        UnlinkIntent,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum PublishEvent {
        Link,
        SyncRoot,
        UnlinkStaging,
        UnlinkIntent,
    }

    struct TracingPublishFsOps {
        failure: PublishFailure,
        root_sync_calls: usize,
        events: Vec<PublishEvent>,
        real: RealPublishFsOps,
    }

    impl TracingPublishFsOps {
        fn new(failure: PublishFailure) -> Self {
            Self {
                failure,
                root_sync_calls: 0,
                events: Vec::new(),
                real: RealPublishFsOps,
            }
        }
    }

    impl PublishFsOps for TracingPublishFsOps {
        fn link_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            if self.failure == PublishFailure::Link {
                return Err(AuthorityError::Io);
            }
            self.real.link_staging(root)?;
            self.events.push(PublishEvent::Link);
            Ok(())
        }

        fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            self.root_sync_calls += 1;
            if self.failure == PublishFailure::SyncRootCall(self.root_sync_calls) {
                return Err(AuthorityError::Io);
            }
            self.real.sync_root(root)?;
            self.events.push(PublishEvent::SyncRoot);
            Ok(())
        }

        fn unlink_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            if self.failure == PublishFailure::UnlinkStaging {
                return Err(AuthorityError::Io);
            }
            self.real.unlink_staging(root)?;
            self.events.push(PublishEvent::UnlinkStaging);
            Ok(())
        }

        fn unlink_intent(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            if self.failure == PublishFailure::UnlinkIntent {
                return Err(AuthorityError::Io);
            }
            self.real.unlink_intent(root)?;
            self.events.push(PublishEvent::UnlinkIntent);
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CleanupFailure {
        UnlinkShm,
        UnlinkWal,
        UnlinkStaging,
        SyncRootCall(usize),
        UnlinkIntent,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum CleanupEvent {
        UnlinkShm,
        UnlinkWal,
        UnlinkStaging,
        SyncRoot,
        UnlinkIntent,
    }

    struct TracingCleanupFsOps {
        failure: CleanupFailure,
        root_sync_calls: usize,
        events: Vec<CleanupEvent>,
        real: RealCleanupFsOps,
    }

    impl TracingCleanupFsOps {
        fn new(failure: CleanupFailure) -> Self {
            Self {
                failure,
                root_sync_calls: 0,
                events: Vec::new(),
                real: RealCleanupFsOps,
            }
        }
    }

    impl CleanupFsOps for TracingCleanupFsOps {
        fn unlink_staging_shm(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            if self.failure == CleanupFailure::UnlinkShm {
                return Err(AuthorityError::Io);
            }
            self.real.unlink_staging_shm(root)?;
            self.events.push(CleanupEvent::UnlinkShm);
            Ok(())
        }

        fn unlink_staging_wal(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            if self.failure == CleanupFailure::UnlinkWal {
                return Err(AuthorityError::Io);
            }
            self.real.unlink_staging_wal(root)?;
            self.events.push(CleanupEvent::UnlinkWal);
            Ok(())
        }

        fn unlink_staging(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            if self.failure == CleanupFailure::UnlinkStaging {
                return Err(AuthorityError::Io);
            }
            self.real.unlink_staging(root)?;
            self.events.push(CleanupEvent::UnlinkStaging);
            Ok(())
        }

        fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            self.root_sync_calls += 1;
            if self.failure == CleanupFailure::SyncRootCall(self.root_sync_calls) {
                return Err(AuthorityError::Io);
            }
            self.real.sync_root(root)?;
            self.events.push(CleanupEvent::SyncRoot);
            Ok(())
        }

        fn unlink_intent(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            if self.failure == CleanupFailure::UnlinkIntent {
                return Err(AuthorityError::Io);
            }
            self.real.unlink_intent(root)?;
            self.events.push(CleanupEvent::UnlinkIntent);
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FsFailure {
        Create,
        WriteCall(usize),
        SyncIntent,
        SyncRootCall(usize),
        CreateStaging,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FsEvent {
        Create,
        Write(usize),
        SyncIntent,
        SyncRoot,
        CreateStaging,
    }

    struct TracingBootstrapFsOps {
        maximum_write: usize,
        failure: Option<FsFailure>,
        write_calls: usize,
        root_sync_calls: usize,
        events: Vec<FsEvent>,
        real: RealBootstrapFsOps,
    }

    impl TracingBootstrapFsOps {
        fn new(maximum_write: usize, failure: Option<FsFailure>) -> Self {
            Self {
                maximum_write,
                failure,
                write_calls: 0,
                root_sync_calls: 0,
                events: Vec::new(),
                real: RealBootstrapFsOps,
            }
        }
    }

    impl BootstrapFsOps for TracingBootstrapFsOps {
        fn create_intent(&mut self, root: BorrowedFd<'_>) -> Result<File, AuthorityError> {
            if self.failure == Some(FsFailure::Create) {
                return Err(AuthorityError::Io);
            }
            let file = self.real.create_intent(root)?;
            self.events.push(FsEvent::Create);
            Ok(file)
        }

        fn write_intent(&mut self, file: &File, bytes: &[u8]) -> Result<usize, AuthorityError> {
            self.write_calls += 1;
            if self.failure == Some(FsFailure::WriteCall(self.write_calls)) {
                return Err(AuthorityError::Io);
            }
            let limit = bytes.len().min(self.maximum_write);
            let written = self.real.write_intent(file, &bytes[..limit])?;
            self.events.push(FsEvent::Write(written));
            Ok(written)
        }

        fn sync_intent(&mut self, file: &File) -> Result<(), AuthorityError> {
            if self.failure == Some(FsFailure::SyncIntent) {
                return Err(AuthorityError::Io);
            }
            self.real.sync_intent(file)?;
            self.events.push(FsEvent::SyncIntent);
            Ok(())
        }

        fn sync_root(&mut self, root: BorrowedFd<'_>) -> Result<(), AuthorityError> {
            self.root_sync_calls += 1;
            if self.failure == Some(FsFailure::SyncRootCall(self.root_sync_calls)) {
                return Err(AuthorityError::Io);
            }
            self.real.sync_root(root)?;
            self.events.push(FsEvent::SyncRoot);
            Ok(())
        }

        fn create_staging(&mut self, root: BorrowedFd<'_>) -> Result<File, AuthorityError> {
            if self.failure == Some(FsFailure::CreateStaging) {
                return Err(AuthorityError::Io);
            }
            let file = self.real.create_staging(root)?;
            self.events.push(FsEvent::CreateStaging);
            Ok(file)
        }
    }
}
