//! Safe macOS filesystem-authority boundary for MengXia.

#![deny(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

mod macos_ffi;

use std::ffi::OsString;
use std::fmt;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Component, Path, PathBuf};

use rustix::fs::{FileType, Mode, OFlags, fstat, fstatfs, open, openat};
use rustix::process::geteuid;

/// Maximum ACL entries accepted by the V1 macOS adapter.
pub const ACL_ENTRY_LIMIT: u32 = 128;

/// Maximum serialized ACL bytes accepted by the V1 macOS adapter.
pub const ACL_EXTERNAL_REPRESENTATION_LIMIT: usize = 16_384;

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
}

impl fmt::Display for AuthorityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsafeConfiguration => "filesystem authority is unsupported or unsafe",
            Self::Io => "filesystem authority inspection failed",
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
}

impl ValidatedAbsolutePath {
    /// Authorizes an existing Library root without mutating it.
    pub fn authorize_existing(path: &Path) -> Result<Self, AuthorityError> {
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

        for (index, name) in names.iter().enumerate() {
            let role = if index + 1 == names.len() {
                ComponentRole::LibraryRoot
            } else if index + 2 == names.len() {
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

        let authority = Self {
            components: retained,
            canonical_sqlite_path: path.join("library.sqlite3"),
            staging_sqlite_path: path.join(".library.sqlite3.bootstrap"),
            owner_uid,
        };
        authority.revalidate_chain()?;
        Ok(authority)
    }

    /// Reopens every retained name from its retained predecessor and proves
    /// that each edge still resolves to the same device/inode under policy.
    pub fn revalidate_chain(&self) -> Result<(), AuthorityError> {
        let fresh_root = open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let mut fresh_parent = fresh_root;
        let root_security = inspect_directory(fresh_parent.as_fd())?;
        let retained_root = self
            .components
            .first()
            .ok_or(AuthorityError::UnsafeConfiguration)?;
        validate_component_policy(root_security, retained_root.role, self.owner_uid)?;
        if !root_security.same_object(retained_root.security) {
            return Err(AuthorityError::UnsafeConfiguration);
        }

        for retained in self.components.iter().skip(1) {
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
            validate_component_policy(security, retained.role, self.owner_uid)?;
            if !security.same_object(retained.security) {
                return Err(AuthorityError::UnsafeConfiguration);
            }
            fresh_parent = fresh;
        }
        Ok(())
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

/// Fixed SQLite child selection accepted by TASK-004.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SqliteChild {
    Canonical,
    BootstrapStaging,
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
        stat.st_dev as u64,
        stat.st_ino as u64,
        stat.st_uid,
        stat.st_mode,
    )
}

fn inspect_security(
    fd: BorrowedFd<'_>,
    device: u64,
    inode: u64,
    owner_uid: u32,
    raw_mode: u16,
) -> Result<MacOsObjectSecurity, AuthorityError> {
    let filesystem = fstatfs(fd).map_err(|_| AuthorityError::Io)?;
    if filesystem.f_flags & MNT_LOCAL == 0
        || filesystem.f_flags & MNT_IGNORE_OWNERSHIP != 0
        || filesystem_name(&filesystem.f_fstypename) != b"apfs"
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let acl = macos_ffi::inspect(fd)?;
    Ok(MacOsObjectSecurity {
        device,
        inode,
        owner_uid,
        mode: u32::from(Mode::from_raw_mode(raw_mode).as_raw_mode()),
        acl,
    })
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
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{AuthorityError, SqliteChild, ValidatedAbsolutePath};

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

    fn chmod_acl(path: &std::path::Path, arguments: &[&str]) {
        let status = Command::new("/bin/chmod")
            .args(arguments)
            .arg(path)
            .status()
            .expect("start the platform ACL fixture command");
        assert!(status.success(), "platform ACL fixture command failed");
    }
}
