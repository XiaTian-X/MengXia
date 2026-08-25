use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Component, Path, PathBuf};

use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, chmodat, fstat, fsync, linkat, mkdirat, open, openat,
    statat, unlinkat,
};
use rustix::io::{read, write};
use rustix::process::geteuid;
use sha2::{Digest, Sha256};

use super::{
    AuthorityError, ComponentRole, MacOsObjectSecurity, inspect_directory, inspect_security,
    validate_component_policy,
};

const SOCKET_NAME: &[u8] = b"client.sock";
const MARKER_NAME: &[u8] = b".mengxia.runtime-owner-v1";
const STAGING_MARKER_NAME: &[u8] = b".mengxia.runtime-owner-v1.staging";
const MARKER_LENGTH: usize = 128;
const SUN_PATH_BYTES_WITH_NUL: usize = 104;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MutationPoint {
    CreateRuntime,
    SyncCreatedRuntime,
    SyncHostingDirectory,
    CreateMarker,
    WriteMarker,
    SyncMarker,
    LinkMarker,
    SyncLinkedMarker,
    UnlinkStagingMarker,
    SyncPublishedMarker,
    RecoverUnlinkStagingMarker,
    RecoverSyncMarker,
    UnlinkStaleSocket,
    SyncStaleSocketRemoval,
    BindSocket,
    ModeSocket,
    ConfigureListener,
    SyncPublishedSocket,
    CleanupJustBoundSocket,
    SyncJustBoundCleanup,
    CleanupPublishedSocket,
    SyncPublishedCleanup,
}

trait MutationFault {
    fn before(&mut self, point: MutationPoint) -> Result<(), AuthorityError>;
}

struct NoMutationFault;

impl MutationFault for NoMutationFault {
    fn before(&mut self, _point: MutationPoint) -> Result<(), AuthorityError> {
        Ok(())
    }
}

struct RetainedRuntimePath {
    hosting_fd: OwnedFd,
    runtime_fd: OwnedFd,
    endpoint_path: PathBuf,
    owner_uid: u32,
}

#[derive(Clone, Copy)]
struct SocketIdentity {
    device: u64,
    inode: u64,
}

/// Published owner-only runtime endpoint. It retains descriptor authority until
/// consuming cleanup, and never exposes the socket path.
pub struct PublishedRuntimeEndpoint {
    authority: RetainedRuntimePath,
    listener: Option<UnixListener>,
    socket_identity: SocketIdentity,
}

impl PublishedRuntimeEndpoint {
    /// Clones only the listening socket handle for adoption by the async runtime.
    pub fn try_clone_listener(&self) -> Result<UnixListener, AuthorityError> {
        self.listener
            .as_ref()
            .ok_or(AuthorityError::UnsafeConfiguration)?
            .try_clone()
            .map_err(|_| AuthorityError::Io)
    }

    /// Closes admission, revalidates and unlinks exactly the published socket,
    /// then synchronizes the runtime directory.
    pub fn cleanup(self) -> Result<(), AuthorityError> {
        self.cleanup_with(&mut NoMutationFault)
    }

    fn cleanup_with<F: MutationFault>(mut self, fault: &mut F) -> Result<(), AuthorityError> {
        self.listener.take();
        revalidate_runtime(&self.authority)?;
        let observed = inspect_socket_edge(&self.authority)?;
        if observed.device != self.socket_identity.device
            || observed.inode != self.socket_identity.inode
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        fault.before(MutationPoint::CleanupPublishedSocket)?;
        unlinkat(
            self.authority.runtime_fd.as_fd(),
            "client.sock",
            AtFlags::empty(),
        )
        .map_err(|_| AuthorityError::Io)?;
        fault.before(MutationPoint::SyncPublishedCleanup)?;
        fsync(self.authority.runtime_fd.as_fd()).map_err(|_| AuthorityError::Io)
    }
}

/// Read-only protected endpoint authority used by the Client before ID creation.
pub struct ClientEndpointAuthority {
    authority: RetainedRuntimePath,
    socket_identity: SocketIdentity,
}

impl ClientEndpointAuthority {
    /// Revalidates the protected edge and connects once without retry.
    pub fn connect(&self) -> Result<UnixStream, AuthorityError> {
        revalidate_runtime(&self.authority)?;
        let before = inspect_socket_edge(&self.authority)?;
        if before.device != self.socket_identity.device
            || before.inode != self.socket_identity.inode
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let stream =
            UnixStream::connect(&self.authority.endpoint_path).map_err(|_| AuthorityError::Io)?;
        let after = inspect_socket_edge(&self.authority)?;
        if after.device != before.device || after.inode != before.inode {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        revalidate_runtime(&self.authority)?;
        Ok(stream)
    }
}

/// Captures the effective user identity through the platform boundary.
#[must_use]
pub fn effective_user_id() -> u32 {
    geteuid().as_raw()
}

/// Performs only lexical and macOS socket-address validation; no namespace is accessed.
pub fn validate_runtime_endpoint_path(endpoint: &Path) -> Result<(), AuthorityError> {
    validate_endpoint_path(endpoint)?;
    let runtime = endpoint
        .parent()
        .ok_or(AuthorityError::UnsafeConfiguration)?;
    let hosting = runtime
        .parent()
        .ok_or(AuthorityError::UnsafeConfiguration)?;
    if std::fs::canonicalize(hosting).map_err(|_| AuthorityError::UnsafeConfiguration)? != hosting {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    match std::fs::canonicalize(runtime) {
        Ok(canonical) if canonical != runtime => Err(AuthorityError::UnsafeConfiguration),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AuthorityError::UnsafeConfiguration),
    }
}

/// Creates/validates the marked runtime namespace and publishes one listener.
pub fn bind_runtime_endpoint(
    endpoint: &Path,
    library_id: [u8; 16],
    owner_uid: u32,
) -> Result<PublishedRuntimeEndpoint, AuthorityError> {
    bind_runtime_endpoint_with(endpoint, library_id, owner_uid, &mut NoMutationFault)
}

fn bind_runtime_endpoint_with<F: MutationFault>(
    endpoint: &Path,
    library_id: [u8; 16],
    owner_uid: u32,
    fault: &mut F,
) -> Result<PublishedRuntimeEndpoint, AuthorityError> {
    if owner_uid != effective_user_id() || !is_uuid_v7(library_id) {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let authority = open_runtime_path(endpoint, owner_uid, true, fault)?;
    publish_or_validate_marker(&authority, library_id, fault)?;
    remove_proven_stale_socket(&authority, fault)?;

    fault.before(MutationPoint::BindSocket)?;
    let listener = UnixListener::bind(&authority.endpoint_path).map_err(|_| AuthorityError::Io)?;
    let publish_result = (|| {
        fault.before(MutationPoint::ModeSocket)?;
        chmodat(
            authority.runtime_fd.as_fd(),
            "client.sock",
            Mode::RUSR | Mode::WUSR,
            AtFlags::empty(),
        )
        .map_err(|_| AuthorityError::Io)?;
        let identity = inspect_socket_edge(&authority)?;
        fault.before(MutationPoint::ConfigureListener)?;
        listener
            .set_nonblocking(true)
            .map_err(|_| AuthorityError::Io)?;
        fault.before(MutationPoint::SyncPublishedSocket)?;
        fsync(authority.runtime_fd.as_fd()).map_err(|_| AuthorityError::Io)?;
        Ok(identity)
    })();

    match publish_result {
        Ok(socket_identity) => Ok(PublishedRuntimeEndpoint {
            authority,
            listener: Some(listener),
            socket_identity,
        }),
        Err(primary) => {
            drop(listener);
            let _ = remove_just_bound_socket(&authority, fault);
            Err(primary)
        }
    }
}

/// Validates an existing marked endpoint without connecting or creating an ID.
pub fn validate_client_endpoint(
    endpoint: &Path,
    client_uid: u32,
) -> Result<ClientEndpointAuthority, AuthorityError> {
    if client_uid != effective_user_id() {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let authority = open_runtime_path(endpoint, client_uid, false, &mut NoMutationFault)?;
    let marker = read_marker(&authority, MARKER_NAME, 1)?;
    validate_marker(&marker, None, client_uid)?;
    let socket_identity = inspect_socket_edge(&authority)?;
    Ok(ClientEndpointAuthority {
        authority,
        socket_identity,
    })
}

fn open_runtime_path(
    endpoint: &Path,
    owner_uid: u32,
    create_runtime: bool,
    fault: &mut impl MutationFault,
) -> Result<RetainedRuntimePath, AuthorityError> {
    validate_endpoint_path(endpoint)?;
    let runtime_path = endpoint
        .parent()
        .ok_or(AuthorityError::UnsafeConfiguration)?;
    let hosting_path = runtime_path
        .parent()
        .ok_or(AuthorityError::UnsafeConfiguration)?;
    let hosting_names = absolute_components(hosting_path)?;
    if hosting_names.is_empty() {
        return Err(AuthorityError::UnsafeConfiguration);
    }

    let mut current = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    for (index, name) in hosting_names.iter().enumerate() {
        let role = if index + 1 == hosting_names.len() {
            ComponentRole::LibraryRoot
        } else {
            ComponentRole::Ancestor
        };
        let next = openat(
            current.as_fd(),
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let security = inspect_directory(next.as_fd())?;
        validate_component_policy(security, role, owner_uid)?;
        current = next;
    }
    let hosting_fd = current;

    let runtime_name = runtime_path
        .file_name()
        .ok_or(AuthorityError::UnsafeConfiguration)?;
    let (runtime_fd, created) = match openat(
        hosting_fd.as_fd(),
        runtime_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => (fd, false),
        Err(rustix::io::Errno::NOENT) if create_runtime => {
            fault.before(MutationPoint::CreateRuntime)?;
            mkdirat(hosting_fd.as_fd(), runtime_name, Mode::RWXU)
                .map_err(|_| AuthorityError::Io)?;
            let fd = openat(
                hosting_fd.as_fd(),
                runtime_name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| AuthorityError::Io)?;
            (fd, true)
        }
        Err(_) => return Err(AuthorityError::UnsafeConfiguration),
    };
    validate_runtime_directory(runtime_fd.as_fd(), owner_uid)?;
    if created {
        fault.before(MutationPoint::SyncCreatedRuntime)?;
        fsync(runtime_fd.as_fd()).map_err(|_| AuthorityError::Io)?;
        fault.before(MutationPoint::SyncHostingDirectory)?;
        fsync(hosting_fd.as_fd()).map_err(|_| AuthorityError::Io)?;
    }
    let authority = RetainedRuntimePath {
        hosting_fd,
        runtime_fd,
        endpoint_path: endpoint.to_path_buf(),
        owner_uid,
    };
    revalidate_runtime(&authority)?;
    Ok(authority)
}

fn validate_endpoint_path(endpoint: &Path) -> Result<(), AuthorityError> {
    if !endpoint.is_absolute()
        || endpoint
            .file_name()
            .is_none_or(|name| name.as_bytes() != SOCKET_NAME)
        || endpoint.as_os_str().as_bytes().contains(&0)
        || endpoint.as_os_str().as_bytes().len() + 1 > SUN_PATH_BYTES_WITH_NUL
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let components = absolute_components(endpoint)?;
    if components.len() < 3 {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(())
}

fn absolute_components(path: &Path) -> Result<Vec<OsString>, AuthorityError> {
    let mut saw_root = false;
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir if !saw_root && names.is_empty() => saw_root = true,
            Component::Normal(name) if saw_root && !name.is_empty() => {
                names.push(name.to_os_string());
            }
            _ => return Err(AuthorityError::UnsafeConfiguration),
        }
    }
    if !saw_root {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(names)
}

fn validate_runtime_directory(
    fd: std::os::fd::BorrowedFd<'_>,
    owner_uid: u32,
) -> Result<MacOsObjectSecurity, AuthorityError> {
    let security = inspect_directory(fd)?;
    if security.owner_uid() != owner_uid || security.mode() != 0o700 || !security.acl().is_empty() {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(security)
}

fn revalidate_runtime(authority: &RetainedRuntimePath) -> Result<(), AuthorityError> {
    validate_runtime_directory(authority.hosting_fd.as_fd(), authority.owner_uid)?;
    let runtime_name = authority
        .endpoint_path
        .parent()
        .and_then(Path::file_name)
        .ok_or(AuthorityError::UnsafeConfiguration)?;
    let reopened = openat(
        authority.hosting_fd.as_fd(),
        runtime_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let retained = validate_runtime_directory(authority.runtime_fd.as_fd(), authority.owner_uid)?;
    let observed = validate_runtime_directory(reopened.as_fd(), authority.owner_uid)?;
    if !retained.same_object(observed) {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(())
}

fn publish_or_validate_marker(
    authority: &RetainedRuntimePath,
    library_id: [u8; 16],
    fault: &mut impl MutationFault,
) -> Result<(), AuthorityError> {
    let entries = enumerate(authority.runtime_fd.as_fd())?;
    match entries.as_slice() {
        [] => publish_marker(authority, library_id, fault),
        [name] if name == STAGING_MARKER_NAME => {
            let staging = read_marker(authority, STAGING_MARKER_NAME, 1)?;
            validate_marker(&staging, Some(library_id), authority.owner_uid)?;
            publish_valid_staging(authority, fault)
        }
        [name] if name == MARKER_NAME => {
            let marker = read_marker(authority, MARKER_NAME, 1)?;
            validate_marker(&marker, Some(library_id), authority.owner_uid)
        }
        [first, second] if first == MARKER_NAME && second == STAGING_MARKER_NAME => {
            let canonical = marker_security(authority, MARKER_NAME, 2)?;
            let staging = marker_security(authority, STAGING_MARKER_NAME, 2)?;
            if !canonical.same_object(staging) {
                return Err(AuthorityError::UnsafeConfiguration);
            }
            let marker = read_marker(authority, MARKER_NAME, 2)?;
            validate_marker(&marker, Some(library_id), authority.owner_uid)?;
            fault.before(MutationPoint::RecoverUnlinkStagingMarker)?;
            unlinkat(
                authority.runtime_fd.as_fd(),
                ".mengxia.runtime-owner-v1.staging",
                AtFlags::empty(),
            )
            .map_err(|_| AuthorityError::Io)?;
            fault.before(MutationPoint::RecoverSyncMarker)?;
            fsync(authority.runtime_fd.as_fd()).map_err(|_| AuthorityError::Io)
        }
        [first, second] if first == MARKER_NAME && second == SOCKET_NAME => {
            let marker = read_marker(authority, MARKER_NAME, 1)?;
            validate_marker(&marker, Some(library_id), authority.owner_uid)
        }
        _ => Err(AuthorityError::UnsafeConfiguration),
    }
}

fn publish_marker(
    authority: &RetainedRuntimePath,
    library_id: [u8; 16],
    fault: &mut impl MutationFault,
) -> Result<(), AuthorityError> {
    let bytes = encode_marker(library_id, authority.owner_uid);
    fault.before(MutationPoint::CreateMarker)?;
    let fd = openat(
        authority.runtime_fd.as_fd(),
        ".mengxia.runtime-owner-v1.staging",
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|_| AuthorityError::Io)?;
    let file = File::from(fd);
    let mut offset = 0;
    while offset < bytes.len() {
        fault.before(MutationPoint::WriteMarker)?;
        let count = write(&file, &bytes[offset..]).map_err(|_| AuthorityError::Io)?;
        if count == 0 || count > bytes.len() - offset {
            return Err(AuthorityError::Io);
        }
        offset += count;
    }
    fault.before(MutationPoint::SyncMarker)?;
    fsync(file.as_fd()).map_err(|_| AuthorityError::Io)?;
    let security = marker_security(authority, STAGING_MARKER_NAME, 1)?;
    if fstat(file.as_fd()).map_err(|_| AuthorityError::Io)?.st_ino as u64 != security.inode {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let reread = read_marker(authority, STAGING_MARKER_NAME, 1)?;
    validate_marker(&reread, Some(library_id), authority.owner_uid)?;
    drop(file);
    publish_valid_staging(authority, fault)
}

fn publish_valid_staging(
    authority: &RetainedRuntimePath,
    fault: &mut impl MutationFault,
) -> Result<(), AuthorityError> {
    fault.before(MutationPoint::LinkMarker)?;
    linkat(
        authority.runtime_fd.as_fd(),
        ".mengxia.runtime-owner-v1.staging",
        authority.runtime_fd.as_fd(),
        ".mengxia.runtime-owner-v1",
        AtFlags::empty(),
    )
    .map_err(|_| AuthorityError::Io)?;
    fault.before(MutationPoint::SyncLinkedMarker)?;
    fsync(authority.runtime_fd.as_fd()).map_err(|_| AuthorityError::Io)?;
    let canonical = marker_security(authority, MARKER_NAME, 2)?;
    let staging = marker_security(authority, STAGING_MARKER_NAME, 2)?;
    if !canonical.same_object(staging) {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    fault.before(MutationPoint::UnlinkStagingMarker)?;
    unlinkat(
        authority.runtime_fd.as_fd(),
        ".mengxia.runtime-owner-v1.staging",
        AtFlags::empty(),
    )
    .map_err(|_| AuthorityError::Io)?;
    fault.before(MutationPoint::SyncPublishedMarker)?;
    fsync(authority.runtime_fd.as_fd()).map_err(|_| AuthorityError::Io)
}

fn encode_marker(library_id: [u8; 16], owner_uid: u32) -> [u8; MARKER_LENGTH] {
    let mut bytes = [0_u8; MARKER_LENGTH];
    bytes[..16].copy_from_slice(b"MENGXIA-RUNTIME\0");
    bytes[16..18].copy_from_slice(&1_u16.to_be_bytes());
    bytes[18..20].copy_from_slice(&(MARKER_LENGTH as u16).to_be_bytes());
    bytes[20..36].copy_from_slice(&library_id);
    bytes[36..40].copy_from_slice(&owner_uid.to_be_bytes());
    let checksum = Sha256::digest(&bytes[..96]);
    bytes[96..].copy_from_slice(&checksum);
    bytes
}

fn validate_marker(
    bytes: &[u8; MARKER_LENGTH],
    expected_library_id: Option<[u8; 16]>,
    owner_uid: u32,
) -> Result<(), AuthorityError> {
    let mut library_id = [0_u8; 16];
    library_id.copy_from_slice(&bytes[20..36]);
    let mut encoded_uid = [0_u8; 4];
    encoded_uid.copy_from_slice(&bytes[36..40]);
    if &bytes[..16] != b"MENGXIA-RUNTIME\0"
        || bytes[16..18] != 1_u16.to_be_bytes()
        || bytes[18..20] != (MARKER_LENGTH as u16).to_be_bytes()
        || expected_library_id.is_some_and(|expected| expected != library_id)
        || !is_uuid_v7(library_id)
        || u32::from_be_bytes(encoded_uid) != owner_uid
        || bytes[40..96].iter().any(|byte| *byte != 0)
        || &bytes[96..] != Sha256::digest(&bytes[..96]).as_slice()
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(())
}

fn read_marker(
    authority: &RetainedRuntimePath,
    name: &[u8],
    expected_links: u64,
) -> Result<[u8; MARKER_LENGTH], AuthorityError> {
    let name = std::ffi::OsStr::from_bytes(name);
    let fd = openat(
        authority.runtime_fd.as_fd(),
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let file = File::from(fd);
    validate_marker_file(file.as_fd(), authority.owner_uid, expected_links)?;
    let mut bytes = [0_u8; MARKER_LENGTH];
    let mut offset = 0;
    while offset < bytes.len() {
        let count = read(&file, &mut bytes[offset..]).map_err(|_| AuthorityError::Io)?;
        if count == 0 || count > bytes.len() - offset {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        offset += count;
    }
    let mut trailing = [0_u8; 1];
    if read(&file, &mut trailing).map_err(|_| AuthorityError::Io)? != 0 {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    validate_marker_file(file.as_fd(), authority.owner_uid, expected_links)?;
    Ok(bytes)
}

fn marker_security(
    authority: &RetainedRuntimePath,
    name: &[u8],
    expected_links: u64,
) -> Result<MacOsObjectSecurity, AuthorityError> {
    let fd = openat(
        authority.runtime_fd.as_fd(),
        std::ffi::OsStr::from_bytes(name),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    validate_marker_file(fd.as_fd(), authority.owner_uid, expected_links)
}

fn validate_marker_file(
    fd: std::os::fd::BorrowedFd<'_>,
    owner_uid: u32,
    expected_links: u64,
) -> Result<MacOsObjectSecurity, AuthorityError> {
    let stat = fstat(fd).map_err(|_| AuthorityError::Io)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_size != MARKER_LENGTH as i64
        || u64::from(stat.st_nlink) != expected_links
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let security = inspect_security(
        fd,
        stat.st_dev as u64,
        stat.st_ino as u64,
        stat.st_uid,
        stat.st_mode,
    )?;
    if security.owner_uid() != owner_uid || security.mode() != 0o600 || !security.acl().is_empty() {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(security)
}

fn remove_proven_stale_socket(
    authority: &RetainedRuntimePath,
    fault: &mut impl MutationFault,
) -> Result<(), AuthorityError> {
    let entries = enumerate(authority.runtime_fd.as_fd())?;
    match entries.as_slice() {
        [name] if name == MARKER_NAME => Ok(()),
        [first, second] if first == MARKER_NAME && second == SOCKET_NAME => {
            let before = inspect_socket_edge(authority)?;
            match UnixStream::connect(&authority.endpoint_path) {
                Ok(_) => Err(AuthorityError::UnsafeConfiguration),
                Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
                    let after = inspect_socket_edge(authority)?;
                    if before.device != after.device || before.inode != after.inode {
                        return Err(AuthorityError::UnsafeConfiguration);
                    }
                    fault.before(MutationPoint::UnlinkStaleSocket)?;
                    unlinkat(
                        authority.runtime_fd.as_fd(),
                        "client.sock",
                        AtFlags::empty(),
                    )
                    .map_err(|_| AuthorityError::Io)?;
                    fault.before(MutationPoint::SyncStaleSocketRemoval)?;
                    fsync(authority.runtime_fd.as_fd()).map_err(|_| AuthorityError::Io)
                }
                Err(_) => Err(AuthorityError::UnsafeConfiguration),
            }
        }
        _ => Err(AuthorityError::UnsafeConfiguration),
    }
}

fn remove_just_bound_socket(
    authority: &RetainedRuntimePath,
    fault: &mut impl MutationFault,
) -> Result<(), AuthorityError> {
    inspect_socket_edge(authority)?;
    fault.before(MutationPoint::CleanupJustBoundSocket)?;
    unlinkat(
        authority.runtime_fd.as_fd(),
        "client.sock",
        AtFlags::empty(),
    )
    .map_err(|_| AuthorityError::Io)?;
    fault.before(MutationPoint::SyncJustBoundCleanup)?;
    fsync(authority.runtime_fd.as_fd()).map_err(|_| AuthorityError::Io)
}

fn inspect_socket_edge(authority: &RetainedRuntimePath) -> Result<SocketIdentity, AuthorityError> {
    let stat = statat(
        authority.runtime_fd.as_fd(),
        "client.sock",
        AtFlags::SYMLINK_NOFOLLOW,
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Socket
        || stat.st_uid != authority.owner_uid
        || Mode::from_raw_mode(stat.st_mode).as_raw_mode() != 0o600
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    super::macos_ffi::require_empty_path(&authority.endpoint_path)?;
    Ok(SocketIdentity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
    })
}

fn enumerate(fd: std::os::fd::BorrowedFd<'_>) -> Result<Vec<Vec<u8>>, AuthorityError> {
    let directory = Dir::read_from(fd).map_err(|_| AuthorityError::Io)?;
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

fn is_uuid_v7(bytes: [u8; 16]) -> bool {
    bytes != [0; 16] && bytes[6] >> 4 == 7 && bytes[8] >> 6 == 2
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct FailAt(Vec<MutationPoint>);

    impl MutationFault for FailAt {
        fn before(&mut self, point: MutationPoint) -> Result<(), AuthorityError> {
            if self.0.contains(&point) {
                Err(AuthorityError::Io)
            } else {
                Ok(())
            }
        }
    }

    fn fixture() -> (PathBuf, PathBuf) {
        let home = fs::canonicalize(PathBuf::from(std::env::var_os("HOME").unwrap())).unwrap();
        let base = home.join(format!(
            ".mengxia-task003-endpoint-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
        let endpoint = base.join("mengxia-runtime-v1/client.sock");
        (base, endpoint)
    }

    fn library_id() -> [u8; 16] {
        let mut bytes = [0x5a; 16];
        bytes[6] = 0x7a;
        bytes[8] = 0x9a;
        bytes
    }

    #[test]
    fn marker_codec_is_exact_and_rejects_tampering() {
        let bytes = encode_marker(library_id(), effective_user_id());
        assert_eq!(bytes.len(), MARKER_LENGTH);
        validate_marker(&bytes, Some(library_id()), effective_user_id()).unwrap();
        for offset in [0, 16, 18, 20, 36, 40] {
            let mut tampered = bytes;
            tampered[offset] ^= 1;
            let checksum = Sha256::digest(&tampered[..96]);
            tampered[96..].copy_from_slice(&checksum);
            assert_eq!(
                validate_marker(&tampered, Some(library_id()), effective_user_id()),
                Err(AuthorityError::UnsafeConfiguration),
                "field at offset {offset} must be independently validated"
            );
        }
        let mut tampered = bytes;
        tampered[127] ^= 1;
        assert_eq!(
            validate_marker(&tampered, Some(library_id()), effective_user_id()),
            Err(AuthorityError::UnsafeConfiguration)
        );
    }

    #[test]
    fn endpoint_path_and_runtime_security_matrix_fails_before_publication() {
        let long = format!("/private/tmp/{}/client.sock", "x".repeat(100));
        for path in [
            PathBuf::from("relative/client.sock"),
            PathBuf::from("/private/tmp/runtime/wrong.sock"),
            PathBuf::from("/private/tmp/../tmp/runtime/client.sock"),
            PathBuf::from(long),
            PathBuf::from(OsString::from_vec(
                b"/private/tmp/runtime/client.sock\0suffix".to_vec(),
            )),
        ] {
            assert_eq!(
                validate_runtime_endpoint_path(&path),
                Err(AuthorityError::UnsafeConfiguration)
            );
        }

        let (base, endpoint) = fixture();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        assert!(!endpoint.parent().unwrap().exists());
        fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();

        fs::DirBuilder::new()
            .mode(0o755)
            .create(endpoint.parent().unwrap())
            .unwrap();
        assert!(matches!(
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        assert!(!endpoint.exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn endpoint_publish_client_validation_and_cleanup_are_identity_bound() {
        let (base, endpoint) = fixture();
        let published =
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()).unwrap();
        validate_client_endpoint(&endpoint, effective_user_id()).unwrap();
        published.cleanup().unwrap();
        assert!(!endpoint.exists());
        assert!(
            endpoint
                .parent()
                .unwrap()
                .join(".mengxia.runtime-owner-v1")
                .exists()
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn unknown_content_is_preserved_and_fails_closed() {
        let (base, endpoint) = fixture();
        fs::DirBuilder::new()
            .mode(0o700)
            .create(endpoint.parent().unwrap())
            .unwrap();
        let unknown = endpoint.parent().unwrap().join("unknown");
        fs::write(&unknown, b"owned by user").unwrap();
        assert!(matches!(
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        assert_eq!(fs::read(&unknown).unwrap(), b"owned by user");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn unproven_staging_and_conflicting_marker_states_are_preserved() {
        for bytes in [
            vec![],
            vec![0x5a],
            vec![0x5a; 127],
            vec![0x5a; 128],
            vec![0x5a; 129],
        ] {
            let (base, endpoint) = fixture();
            fs::DirBuilder::new()
                .mode(0o700)
                .create(endpoint.parent().unwrap())
                .unwrap();
            let staging = endpoint
                .parent()
                .unwrap()
                .join(std::ffi::OsStr::from_bytes(STAGING_MARKER_NAME));
            fs::write(&staging, &bytes).unwrap();
            fs::set_permissions(&staging, fs::Permissions::from_mode(0o600)).unwrap();
            let inode = fs::metadata(&staging).unwrap().ino();
            assert!(matches!(
                bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()),
                Err(AuthorityError::UnsafeConfiguration)
            ));
            assert_eq!(fs::read(&staging).unwrap(), bytes);
            assert_eq!(fs::metadata(&staging).unwrap().ino(), inode);
            assert!(!endpoint.exists());
            assert!(
                !endpoint
                    .parent()
                    .unwrap()
                    .join(std::ffi::OsStr::from_bytes(MARKER_NAME))
                    .exists()
            );
            fs::remove_dir_all(base).unwrap();
        }

        let (base, endpoint) = fixture();
        let published =
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()).unwrap();
        published.cleanup().unwrap();
        let runtime = endpoint.parent().unwrap();
        let canonical = runtime.join(std::ffi::OsStr::from_bytes(MARKER_NAME));
        let staging = runtime.join(std::ffi::OsStr::from_bytes(STAGING_MARKER_NAME));
        fs::copy(&canonical, &staging).unwrap();
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o600)).unwrap();
        let canonical_inode = fs::metadata(&canonical).unwrap().ino();
        let staging_inode = fs::metadata(&staging).unwrap().ino();
        assert_ne!(canonical_inode, staging_inode);
        assert!(matches!(
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        assert_eq!(fs::metadata(&canonical).unwrap().ino(), canonical_inode);
        assert_eq!(fs::metadata(&staging).unwrap().ino(), staging_inode);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn live_socket_collision_is_never_unlinked() {
        let (base, endpoint) = fixture();
        let published =
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()).unwrap();
        let inode = fs::symlink_metadata(&endpoint).unwrap().ino();
        assert!(matches!(
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        assert_eq!(fs::symlink_metadata(&endpoint).unwrap().ino(), inode);
        published.cleanup().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn socket_extended_acl_is_rejected_even_with_mode_0600() {
        let (base, endpoint) = fixture();
        let published =
            bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()).unwrap();
        let status = Command::new("/bin/chmod")
            .args(["+a", "everyone allow read"])
            .arg(&endpoint)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(matches!(
            validate_client_endpoint(&endpoint, effective_user_id()),
            Err(AuthorityError::UnsafeConfiguration)
        ));
        let status = Command::new("/bin/chmod")
            .arg("-N")
            .arg(&endpoint)
            .status()
            .unwrap();
        assert!(status.success());
        published.cleanup().unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn every_startup_mutation_failure_preserves_a_recognized_prefix() {
        for point in [
            MutationPoint::CreateRuntime,
            MutationPoint::SyncCreatedRuntime,
            MutationPoint::SyncHostingDirectory,
            MutationPoint::CreateMarker,
            MutationPoint::WriteMarker,
            MutationPoint::SyncMarker,
            MutationPoint::LinkMarker,
            MutationPoint::SyncLinkedMarker,
            MutationPoint::UnlinkStagingMarker,
            MutationPoint::SyncPublishedMarker,
            MutationPoint::BindSocket,
            MutationPoint::ModeSocket,
            MutationPoint::ConfigureListener,
            MutationPoint::SyncPublishedSocket,
        ] {
            let (base, endpoint) = fixture();
            let result = bind_runtime_endpoint_with(
                &endpoint,
                library_id(),
                effective_user_id(),
                &mut FailAt(vec![point]),
            );
            assert!(
                matches!(result, Err(AuthorityError::Io)),
                "unexpected result at {point:?}"
            );
            let runtime = endpoint.parent().unwrap();
            if runtime.exists() {
                let entries = fs::read_dir(runtime).unwrap().count();
                assert!(entries <= 2, "unbounded prefix at {point:?}");
            }
            fs::remove_dir_all(base).unwrap();
        }
    }

    #[test]
    fn recovery_stale_and_cleanup_failures_preserve_exact_completed_edges() {
        for point in [
            MutationPoint::RecoverUnlinkStagingMarker,
            MutationPoint::RecoverSyncMarker,
        ] {
            let (base, endpoint) = fixture();
            let first = bind_runtime_endpoint_with(
                &endpoint,
                library_id(),
                effective_user_id(),
                &mut FailAt(vec![MutationPoint::SyncLinkedMarker]),
            );
            assert!(matches!(first, Err(AuthorityError::Io)));
            let result = bind_runtime_endpoint_with(
                &endpoint,
                library_id(),
                effective_user_id(),
                &mut FailAt(vec![point]),
            );
            assert!(matches!(result, Err(AuthorityError::Io)));
            fs::remove_dir_all(base).unwrap();
        }

        for point in [
            MutationPoint::UnlinkStaleSocket,
            MutationPoint::SyncStaleSocketRemoval,
        ] {
            let (base, endpoint) = fixture();
            let published =
                bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()).unwrap();
            drop(published);
            let result = bind_runtime_endpoint_with(
                &endpoint,
                library_id(),
                effective_user_id(),
                &mut FailAt(vec![point]),
            );
            assert!(matches!(result, Err(AuthorityError::Io)));
            fs::remove_dir_all(base).unwrap();
        }

        for point in [
            MutationPoint::CleanupPublishedSocket,
            MutationPoint::SyncPublishedCleanup,
        ] {
            let (base, endpoint) = fixture();
            let published =
                bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()).unwrap();
            assert!(matches!(
                published.cleanup_with(&mut FailAt(vec![point])),
                Err(AuthorityError::Io)
            ));
            fs::remove_dir_all(base).unwrap();
        }

        for point in [
            MutationPoint::CleanupJustBoundSocket,
            MutationPoint::SyncJustBoundCleanup,
        ] {
            let (base, endpoint) = fixture();
            let result = bind_runtime_endpoint_with(
                &endpoint,
                library_id(),
                effective_user_id(),
                &mut FailAt(vec![MutationPoint::ModeSocket, point]),
            );
            assert!(matches!(result, Err(AuthorityError::Io)));
            fs::remove_dir_all(base).unwrap();
        }
    }

    #[test]
    fn same_os_sigkill_reopens_only_the_proven_stale_socket() {
        const ROLE: &str = "MENGXIA_TASK003_ENDPOINT_SIGKILL_ROLE";
        const PATH: &str = "MENGXIA_TASK003_ENDPOINT_SIGKILL_PATH";
        if std::env::var_os(ROLE).is_some() {
            let endpoint = PathBuf::from(std::env::var_os(PATH).unwrap());
            let _published =
                bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()).unwrap();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }

        let (base, endpoint) = fixture();
        let executable = std::env::current_exe().unwrap();
        let mut child = Command::new(executable)
            .env(ROLE, "child")
            .env(PATH, &endpoint)
            .args([
                "runtime_endpoint::tests::same_os_sigkill_reopens_only_the_proven_stale_socket",
                "--exact",
                "--nocapture",
            ])
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !endpoint.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "child endpoint publication exceeded deadline"
            );
            assert!(child.try_wait().unwrap().is_none(), "child exited early");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());

        let reopened = bind_runtime_endpoint(&endpoint, library_id(), effective_user_id()).unwrap();
        reopened.cleanup().unwrap();
        fs::remove_dir_all(base).unwrap();
    }
}
