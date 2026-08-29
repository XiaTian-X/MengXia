//! Opaque descriptor-first authority for the TASK-005 local CAS.

use std::ffi::{OsStr, OsString};
use std::fs::{File, TryLockError};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, fcntl_fullfsync, fstat, fstatvfs, getpath,
    mkdirat, open, openat, renameat_with, unlinkat,
};
use rustix::io::{pread, pwrite};
use sha2::{Digest as _, Sha256};

use super::{
    AuthorityError, ComponentRole, FinalDirectoryCreatePoint, LibraryLockLease,
    ValidatedAbsolutePath, inspect_directory, inspect_internal_file, validate_component_policy,
};

const MAX_BLOB_ROOT_BYTES: usize = 937;
const LOCK_PREFIX: &str = ".mengxia-cas-v1-";
const STAGING_DIRECTORY: &str = ".staging-v1";
const CAS_DIRECTORY: &str = "sha256-v1";
const MAX_SOURCE_PATH_BYTES: usize = 1023;
const MAX_INTERRUPTED_SYSCALL_RETRIES: usize = 8;
const MAX_OBSERVED_STAGING_ENTRIES: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlobMutationPoint {
    BeforeRootMkdir,
    AfterRootMkdir,
    BeforeRootSync,
    AfterRootChildSync,
    BeforeRootParentSync,
    AfterRootSync,
    BeforeLockCreate,
    AfterLockCreateAndLock,
    BeforeLockFileSync,
    AfterLockFileSync,
    BeforeLockRootSync,
    AfterLockRootSync,
    BeforeLockedReenumerate,
    AfterLockedReenumerate,
    BeforeStagingMkdir,
    AfterStagingMkdir,
    BeforeStagingChildSync,
    AfterStagingChildSync,
    BeforeStagingParentSync,
    AfterStagingParentSync,
    BeforeCasMkdir,
    AfterCasMkdir,
    BeforeCasChildSync,
    AfterCasChildSync,
    BeforeCasParentSync,
    AfterCasParentSync,
}

trait BlobMutationFault {
    fn at(&mut self, point: BlobMutationPoint) -> Result<(), AuthorityError>;
}

struct NoBlobMutationFault;

impl BlobMutationFault for NoBlobMutationFault {
    fn at(&mut self, _point: BlobMutationPoint) -> Result<(), AuthorityError> {
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct DirectoryMutationPoints {
    before_mkdir: BlobMutationPoint,
    after_mkdir: BlobMutationPoint,
    before_child_sync: BlobMutationPoint,
    after_child_sync: BlobMutationPoint,
    before_parent_sync: BlobMutationPoint,
    after_parent_sync: BlobMutationPoint,
}

const STAGING_MUTATION_POINTS: DirectoryMutationPoints = DirectoryMutationPoints {
    before_mkdir: BlobMutationPoint::BeforeStagingMkdir,
    after_mkdir: BlobMutationPoint::AfterStagingMkdir,
    before_child_sync: BlobMutationPoint::BeforeStagingChildSync,
    after_child_sync: BlobMutationPoint::AfterStagingChildSync,
    before_parent_sync: BlobMutationPoint::BeforeStagingParentSync,
    after_parent_sync: BlobMutationPoint::AfterStagingParentSync,
};

const CAS_MUTATION_POINTS: DirectoryMutationPoints = DirectoryMutationPoints {
    before_mkdir: BlobMutationPoint::BeforeCasMkdir,
    after_mkdir: BlobMutationPoint::AfterCasMkdir,
    before_child_sync: BlobMutationPoint::BeforeCasChildSync,
    after_child_sync: BlobMutationPoint::AfterCasChildSync,
    before_parent_sync: BlobMutationPoint::BeforeCasParentSync,
    after_parent_sync: BlobMutationPoint::AfterCasParentSync,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlobFileMutationPoint {
    BeforeSourceWalk,
    AfterSourceOpen,
    BeforeSourceRevalidate,
    AfterSourceRevalidate,
    AfterStagingCreate,
    AfterStagingNameSync,
    BeforeStagingFileSync,
    AfterStagingFileSync,
    BeforePrepromoteRevalidate,
    AfterPrepromoteRevalidate,
    BeforeFirstShardMkdirOpen,
    AfterFirstShardMkdirOpen,
    BeforeFirstShardSyncs,
    AfterFirstShardChildSync,
    BeforeFirstShardParentSync,
    AfterFirstShardSyncs,
    BeforeSecondShardMkdirOpen,
    AfterSecondShardMkdirOpen,
    BeforeSecondShardSyncs,
    AfterSecondShardChildSync,
    BeforeSecondShardParentSync,
    AfterSecondShardSyncs,
    BeforeNoReplaceRename,
    NoReplaceError,
    AfterNoReplaceRename,
    CanonicalReopenOrCaseProof,
    BeforeDestinationSync,
    AfterDestinationSync,
    BeforePostPromoteStagingSync,
    AfterPostPromoteStagingSync,
    BeforeExistingVerify,
    AfterExistingVerify,
    BeforeDedupStagingRevalidate,
    AfterDedupStagingRevalidate,
    BeforeCleanupRevalidate,
    BeforeCleanupUnlink,
    AfterCleanupUnlink,
    AfterCleanupPostUnlinkSync,
}

trait BlobFileMutationFault {
    fn at(&mut self, point: BlobFileMutationPoint) -> Result<(), BlobFileError>;

    fn no_replace_result(&mut self) -> Option<rustix::io::Result<()>> {
        None
    }
}

struct NoBlobFileMutationFault;

impl BlobFileMutationFault for NoBlobFileMutationFault {
    fn at(&mut self, _point: BlobFileMutationPoint) -> Result<(), BlobFileError> {
        Ok(())
    }
}

/// Redacted operation failure consumed only by the local storage adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlobFileError {
    InvalidPath,
    UnsupportedType,
    Io,
    Modified,
    Configuration,
    Corruption,
    Collision,
    CleanupFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlobCapacity {
    pub available_bytes: u128,
    pub total_bytes: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BlobOrphanSummary {
    pub count: u16,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlobCommitOutcome {
    Published,
    ExistingVerified,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct FileSnapshot {
    device: u64,
    inode: u64,
    length: u64,
    modified_seconds: i64,
    modified_nanos: i64,
    changed_seconds: i64,
    changed_nanos: i64,
    link_count: u64,
}

/// Stable, opaque descriptor for one local source.
pub struct OpenedBlobSource {
    file: File,
    parent: OwnedFd,
    name: OsString,
    snapshot: FileSnapshot,
}

impl OpenedBlobSource {
    #[must_use]
    pub const fn declared_length(&self) -> u64 {
        self.snapshot.length
    }

    pub fn revalidate(&self) -> Result<(), BlobFileError> {
        if source_snapshot(self.file.as_fd())? != self.snapshot {
            return Err(BlobFileError::Modified);
        }
        let reopened = openat(
            self.parent.as_fd(),
            &self.name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|_| BlobFileError::Modified)?;
        if source_snapshot(reopened.as_fd())? != self.snapshot {
            return Err(BlobFileError::Modified);
        }
        Ok(())
    }

    pub fn read_at(&self, buffer: &mut [u8], offset: u64) -> Result<usize, BlobFileError> {
        retry_interrupted(|| pread(self.file.as_fd(), &mut *buffer, offset))
            .map_err(|_| BlobFileError::Io)
    }
}

/// Exclusively created staging inode; it exposes no filename or descriptor.
pub struct OpenedBlobStaging {
    file: File,
    name: String,
    device: u64,
    inode: u64,
}

/// Exclusively locked Blob-root file whose release does not depend on closing
/// the last duplicate descriptor.
///
/// A concurrently spawned process can inherit a `CLOEXEC` descriptor during
/// the narrow fork-to-exec window. Explicit unlock on every drop keeps that
/// inherited duplicate from extending the logical authority lifetime.
struct ExclusiveBlobLock {
    file: File,
}

impl ExclusiveBlobLock {
    fn acquire(file: File) -> Result<Self, AuthorityError> {
        match file.try_lock() {
            Ok(()) => Ok(Self { file }),
            Err(TryLockError::WouldBlock) => Err(AuthorityError::Contended),
            Err(TryLockError::Error(_)) => Err(AuthorityError::Io),
        }
    }

    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.file.as_fd()
    }
}

impl Drop for ExclusiveBlobLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

impl OpenedBlobStaging {
    pub fn write_at(&self, bytes: &[u8], offset: u64) -> Result<usize, BlobFileError> {
        retry_interrupted(|| pwrite(self.file.as_fd(), bytes, offset))
            .map_err(|_| BlobFileError::Io)
    }
}

/// Validated but unopened request for one configured Blob root.
///
/// The path bytes and comparison identity deliberately have no public accessor.
pub struct BlobRootRequest {
    path: PathBuf,
    identity: [u8; 32],
}

impl BlobRootRequest {
    /// Validates the platform path shape without walking or mutating it.
    pub fn from_absolute_path(path: &Path) -> Result<Self, AuthorityError> {
        super::validate_lexical_absolute_path(path)?;
        let bytes = path.as_os_str().as_bytes();
        if bytes.len() > MAX_BLOB_ROOT_BYTES || bytes.contains(&0) {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        let identity = Sha256::digest(bytes).into();
        Ok(Self {
            path: path.to_path_buf(),
            identity,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// Exclusive, opaque authority for one initialized Blob root.
pub struct OpenedBlobRootAuthority {
    request_identity: [u8; 32],
    library: Arc<ValidatedAbsolutePath>,
    root: ValidatedAbsolutePath,
    _blob_lock: ExclusiveBlobLock,
    _library_lock_lease: Arc<LibraryLockLease>,
    staging: OwnedFd,
    cas: OwnedFd,
    library_id: [u8; 16],
    backend_instance_digest: [u8; 32],
}

impl OpenedBlobRootAuthority {
    /// Confirms that this grant was minted from the same immutable request.
    #[must_use]
    pub fn authorizes(&self, request: &BlobRootRequest) -> bool {
        self.request_identity == request.identity
    }

    /// Returns the non-secret digest used to derive the opaque backend ID.
    #[must_use]
    pub const fn backend_instance_digest(&self) -> [u8; 32] {
        self.backend_instance_digest
    }

    /// Returns the durable Library UUID directly bound into this authority.
    #[must_use]
    pub const fn library_id_bytes(&self) -> [u8; 16] {
        self.library_id
    }

    /// Revalidates all retained roots and the exact fixed CAS directories.
    pub fn revalidate(&self) -> Result<(), AuthorityError> {
        self.library.revalidate_chain()?;
        self.root.revalidate_chain()?;
        validate_named_directory(&self.root, STAGING_DIRECTORY, self.staging.as_fd())?;
        validate_named_directory(&self.root, CAS_DIRECTORY, self.cas.as_fd())?;
        Ok(())
    }

    /// Revalidates and permanently flushes the retained CAS namespace.
    pub fn sync_for_shutdown(&self) -> Result<(), AuthorityError> {
        self.revalidate()?;
        fcntl_fullfsync(self.staging.as_fd()).map_err(|_| AuthorityError::Io)?;
        fcntl_fullfsync(self.cas.as_fd()).map_err(|_| AuthorityError::Io)?;
        fcntl_fullfsync(self.root.library_root_fd()).map_err(|_| AuthorityError::Io)?;
        self.revalidate()
    }

    pub fn capacity(&self) -> Result<BlobCapacity, BlobFileError> {
        self.revalidate().map_err(map_authority_error)?;
        let stat = fstatvfs(self.staging.as_fd()).map_err(|_| BlobFileError::Io)?;
        Ok(BlobCapacity {
            available_bytes: u128::from(stat.f_bavail)
                .checked_mul(u128::from(stat.f_frsize))
                .ok_or(BlobFileError::Configuration)?,
            total_bytes: u128::from(stat.f_blocks)
                .checked_mul(u128::from(stat.f_frsize))
                .ok_or(BlobFileError::Configuration)?,
        })
    }

    pub fn observe_staging_orphans(&self) -> Result<BlobOrphanSummary, BlobFileError> {
        self.revalidate().map_err(map_authority_error)?;
        let directory = Dir::read_from(self.staging.as_fd()).map_err(|_| BlobFileError::Io)?;
        let mut count = 0_u16;
        let mut bytes = 0_u64;
        for entry in directory {
            let entry = entry.map_err(|_| BlobFileError::Io)?;
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            if usize::from(count) == MAX_OBSERVED_STAGING_ENTRIES || !valid_staging_name(name) {
                return Err(BlobFileError::Configuration);
            }
            let fd = openat(
                self.staging.as_fd(),
                entry.file_name(),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|_| BlobFileError::Configuration)?;
            let security = validate_blob_file(fd.as_fd(), self.root.owner_uid)?;
            if security.device != self.root.root_device {
                return Err(BlobFileError::Configuration);
            }
            exact_final_component(fd.as_fd(), name).map_err(|_| BlobFileError::Configuration)?;
            let stat = fstat(fd.as_fd()).map_err(|_| BlobFileError::Io)?;
            let length = u64::try_from(stat.st_size).map_err(|_| BlobFileError::Configuration)?;
            count = count.checked_add(1).ok_or(BlobFileError::Configuration)?;
            bytes = bytes
                .checked_add(length)
                .ok_or(BlobFileError::Configuration)?;
        }
        Ok(BlobOrphanSummary { count, bytes })
    }

    pub fn open_source(&self, path: &Path) -> Result<OpenedBlobSource, BlobFileError> {
        self.open_source_with(path, &mut NoBlobFileMutationFault)
    }

    fn open_source_with(
        &self,
        path: &Path,
        fault: &mut impl BlobFileMutationFault,
    ) -> Result<OpenedBlobSource, BlobFileError> {
        self.revalidate().map_err(map_authority_error)?;
        fault.at(BlobFileMutationPoint::BeforeSourceWalk)?;
        validate_source_path(path)?;
        let names: Vec<OsString> = path
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_os_string()),
                _ => None,
            })
            .collect();
        let final_name = names.last().ok_or(BlobFileError::InvalidPath)?.clone();
        let mut parent = open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| BlobFileError::Io)?;
        for name in names.iter().take(names.len() - 1) {
            let next = openat(
                parent.as_fd(),
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| BlobFileError::InvalidPath)?;
            let security =
                inspect_directory(next.as_fd()).map_err(|_| BlobFileError::InvalidPath)?;
            exact_final_component(next.as_fd(), name.as_bytes())
                .map_err(|_| BlobFileError::InvalidPath)?;
            let reopened = openat(
                parent.as_fd(),
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| BlobFileError::InvalidPath)?;
            let reopened_security =
                inspect_directory(reopened.as_fd()).map_err(|_| BlobFileError::InvalidPath)?;
            if !reopened_security.same_object(security) {
                return Err(BlobFileError::InvalidPath);
            }
            if (security.device, security.inode)
                == (self.library.root_device, self.library.root_inode)
                || (security.device, security.inode)
                    == (self.root.root_device, self.root.root_inode)
            {
                return Err(BlobFileError::InvalidPath);
            }
            parent = next;
        }
        let fd = openat(
            parent.as_fd(),
            &final_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|error| match error {
            rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => BlobFileError::InvalidPath,
            _ => BlobFileError::Io,
        })?;
        fault.at(BlobFileMutationPoint::AfterSourceOpen)?;
        let stat = fstat(fd.as_fd()).map_err(|_| BlobFileError::Io)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(BlobFileError::UnsupportedType);
        }
        let security = super::inspect_security(
            fd.as_fd(),
            stat.st_dev as u64,
            stat.st_ino as u64,
            stat.st_uid,
            stat.st_mode,
        )
        .map_err(|_| BlobFileError::InvalidPath)?;
        if security.owner_uid != self.root.owner_uid {
            return Err(BlobFileError::InvalidPath);
        }
        exact_final_component(fd.as_fd(), final_name.as_bytes())
            .map_err(|_| BlobFileError::InvalidPath)?;
        let file = File::from(fd);
        let snapshot = source_snapshot(file.as_fd())?;
        let source = OpenedBlobSource {
            file,
            parent,
            name: final_name,
            snapshot,
        };
        fault.at(BlobFileMutationPoint::BeforeSourceRevalidate)?;
        source.revalidate()?;
        fault.at(BlobFileMutationPoint::AfterSourceRevalidate)?;
        Ok(source)
    }

    pub fn create_staging(&self, random: [u8; 16]) -> Result<OpenedBlobStaging, BlobFileError> {
        self.create_staging_with(random, &mut NoBlobFileMutationFault)
    }

    fn create_staging_with(
        &self,
        random: [u8; 16],
        fault: &mut impl BlobFileMutationFault,
    ) -> Result<OpenedBlobStaging, BlobFileError> {
        self.revalidate().map_err(map_authority_error)?;
        let name = staging_name(random);
        let fd = openat(
            self.staging.as_fd(),
            name.as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|error| match error {
            rustix::io::Errno::EXIST => BlobFileError::Collision,
            _ => BlobFileError::Io,
        })?;
        fault.at(BlobFileMutationPoint::AfterStagingCreate)?;
        let security = validate_blob_file(fd.as_fd(), self.root.owner_uid)?;
        exact_final_component(fd.as_fd(), name.as_bytes())
            .map_err(|_| BlobFileError::Configuration)?;
        fcntl_fullfsync(self.staging.as_fd()).map_err(|_| BlobFileError::Io)?;
        fault.at(BlobFileMutationPoint::AfterStagingNameSync)?;
        Ok(OpenedBlobStaging {
            file: File::from(fd),
            name,
            device: security.device,
            inode: security.inode,
        })
    }

    pub fn cleanup_staging(&self, staging: &OpenedBlobStaging) -> Result<(), BlobFileError> {
        self.cleanup_staging_with(staging, &mut NoBlobFileMutationFault)
    }

    fn cleanup_staging_with(
        &self,
        staging: &OpenedBlobStaging,
        fault: &mut impl BlobFileMutationFault,
    ) -> Result<(), BlobFileError> {
        fault
            .at(BlobFileMutationPoint::BeforeCleanupRevalidate)
            .map_err(|_| BlobFileError::CleanupFailed)?;
        self.revalidate()
            .map_err(|_| BlobFileError::CleanupFailed)?;
        self.validate_staging(staging, None)
            .map_err(|_| BlobFileError::CleanupFailed)?;
        fault
            .at(BlobFileMutationPoint::BeforeCleanupUnlink)
            .map_err(|_| BlobFileError::CleanupFailed)?;
        unlinkat(
            self.staging.as_fd(),
            staging.name.as_str(),
            AtFlags::empty(),
        )
        .map_err(|_| BlobFileError::CleanupFailed)?;
        fault
            .at(BlobFileMutationPoint::AfterCleanupUnlink)
            .map_err(|_| BlobFileError::CleanupFailed)?;
        fcntl_fullfsync(self.staging.as_fd()).map_err(|_| BlobFileError::CleanupFailed)?;
        fault
            .at(BlobFileMutationPoint::AfterCleanupPostUnlinkSync)
            .map_err(|_| BlobFileError::CleanupFailed)
    }

    pub fn commit_staging(
        &self,
        staging: &OpenedBlobStaging,
        digest: [u8; 32],
        length: u64,
        rehash_buffer_bytes: usize,
    ) -> Result<BlobCommitOutcome, BlobFileError> {
        self.commit_staging_with(
            staging,
            digest,
            length,
            rehash_buffer_bytes,
            &mut NoBlobFileMutationFault,
        )
    }

    fn commit_staging_with(
        &self,
        staging: &OpenedBlobStaging,
        digest: [u8; 32],
        length: u64,
        rehash_buffer_bytes: usize,
        fault: &mut impl BlobFileMutationFault,
    ) -> Result<BlobCommitOutcome, BlobFileError> {
        self.revalidate().map_err(map_authority_error)?;
        fault.at(BlobFileMutationPoint::BeforeStagingFileSync)?;
        fcntl_fullfsync(staging.file.as_fd()).map_err(|_| BlobFileError::Io)?;
        fault.at(BlobFileMutationPoint::AfterStagingFileSync)?;
        fault.at(BlobFileMutationPoint::BeforePrepromoteRevalidate)?;
        self.validate_staging(staging, Some(length))?;
        fault.at(BlobFileMutationPoint::AfterPrepromoteRevalidate)?;
        let hex = lowercase_hex(digest);
        let first = open_or_create_shard(
            self,
            self.cas.as_fd(),
            &hex[..2],
            ShardMutationPoints::FIRST,
            fault,
        )?;
        let second = open_or_create_shard(
            self,
            first.as_fd(),
            &hex[2..4],
            ShardMutationPoints::SECOND,
            fault,
        )?;
        let final_name = format!("{hex}.blob");
        fault.at(BlobFileMutationPoint::BeforeNoReplaceRename)?;
        let rename_result = fault.no_replace_result().unwrap_or_else(|| {
            renameat_with(
                self.staging.as_fd(),
                staging.name.as_str(),
                second.as_fd(),
                final_name.as_str(),
                RenameFlags::NOREPLACE,
            )
        });
        match rename_result {
            Ok(()) => {
                fault.at(BlobFileMutationPoint::AfterNoReplaceRename)?;
                let canonical = openat(
                    second.as_fd(),
                    final_name.as_str(),
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| BlobFileError::Corruption)?;
                let security = validate_blob_file(canonical.as_fd(), self.root.owner_uid)
                    .map_err(|_| BlobFileError::Corruption)?;
                if security.device != staging.device || security.inode != staging.inode {
                    return Err(BlobFileError::Corruption);
                }
                exact_final_component(canonical.as_fd(), final_name.as_bytes())
                    .map_err(|_| BlobFileError::Configuration)?;
                fault.at(BlobFileMutationPoint::CanonicalReopenOrCaseProof)?;
                fault.at(BlobFileMutationPoint::BeforeDestinationSync)?;
                fcntl_fullfsync(second.as_fd()).map_err(|_| BlobFileError::Io)?;
                fault.at(BlobFileMutationPoint::AfterDestinationSync)?;
                fault.at(BlobFileMutationPoint::BeforePostPromoteStagingSync)?;
                fcntl_fullfsync(self.staging.as_fd()).map_err(|_| BlobFileError::Io)?;
                fault.at(BlobFileMutationPoint::AfterPostPromoteStagingSync)?;
                Ok(BlobCommitOutcome::Published)
            }
            Err(rustix::io::Errno::EXIST) => {
                fault.at(BlobFileMutationPoint::BeforeExistingVerify)?;
                verify_existing_blob(
                    self,
                    second.as_fd(),
                    &final_name,
                    digest,
                    length,
                    rehash_buffer_bytes,
                )?;
                fault.at(BlobFileMutationPoint::AfterExistingVerify)?;
                fault.at(BlobFileMutationPoint::BeforeDedupStagingRevalidate)?;
                self.validate_staging(staging, None)?;
                fault.at(BlobFileMutationPoint::AfterDedupStagingRevalidate)?;
                self.cleanup_staging_with(staging, fault)?;
                Ok(BlobCommitOutcome::ExistingVerified)
            }
            Err(rustix::io::Errno::XDEV) => Err(BlobFileError::Configuration),
            Err(_) => {
                fault.at(BlobFileMutationPoint::NoReplaceError)?;
                Err(BlobFileError::Io)
            }
        }
    }

    fn validate_staging(
        &self,
        staging: &OpenedBlobStaging,
        expected_length: Option<u64>,
    ) -> Result<(), BlobFileError> {
        let security = validate_blob_file(staging.file.as_fd(), self.root.owner_uid)?;
        let stat = fstat(staging.file.as_fd()).map_err(|_| BlobFileError::Io)?;
        let length = u64::try_from(stat.st_size).map_err(|_| BlobFileError::Configuration)?;
        if security.device != staging.device
            || security.inode != staging.inode
            || expected_length.is_some_and(|expected| expected != length)
        {
            return Err(BlobFileError::Modified);
        }
        let reopened = openat(
            self.staging.as_fd(),
            staging.name.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| BlobFileError::Modified)?;
        let reopened = validate_blob_file(reopened.as_fd(), self.root.owner_uid)
            .map_err(|_| BlobFileError::Modified)?;
        if reopened.device != staging.device || reopened.inode != staging.inode {
            return Err(BlobFileError::Modified);
        }
        Ok(())
    }
}

pub(super) fn authorize_blob_root(
    library: Arc<ValidatedAbsolutePath>,
    library_lock_lease: Arc<LibraryLockLease>,
    request: &BlobRootRequest,
    library_id: [u8; 16],
) -> Result<OpenedBlobRootAuthority, AuthorityError> {
    authorize_blob_root_with(
        library,
        library_lock_lease,
        request,
        library_id,
        &mut NoBlobMutationFault,
    )
}

fn authorize_blob_root_with<F: BlobMutationFault>(
    library: Arc<ValidatedAbsolutePath>,
    library_lock_lease: Arc<LibraryLockLease>,
    request: &BlobRootRequest,
    library_id: [u8; 16],
    fault: &mut F,
) -> Result<OpenedBlobRootAuthority, AuthorityError> {
    library.revalidate_chain()?;
    let (root, root_created) =
        ValidatedAbsolutePath::authorize_with(request.path(), true, &mut |point| {
            fault.at(match point {
                FinalDirectoryCreatePoint::BeforeMkdir => BlobMutationPoint::BeforeRootMkdir,
                FinalDirectoryCreatePoint::AfterMkdir => BlobMutationPoint::AfterRootMkdir,
            })
        })?;
    if root.root_device == library.root_device && root.root_inode == library.root_inode {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    if root_created {
        fault.at(BlobMutationPoint::BeforeRootSync)?;
        fcntl_fullfsync(root.library_root_fd()).map_err(|_| AuthorityError::Io)?;
        fault.at(BlobMutationPoint::AfterRootChildSync)?;
        let parent = root
            .components
            .get(root.components.len().saturating_sub(2))
            .ok_or(AuthorityError::UnsafeConfiguration)?;
        fault.at(BlobMutationPoint::BeforeRootParentSync)?;
        fcntl_fullfsync(parent.fd.as_fd()).map_err(|_| AuthorityError::Io)?;
        fault.at(BlobMutationPoint::AfterRootSync)?;
    }

    let lock_name = lock_name(library_id);
    let entries = enumerate_top(&root)?;
    let lock_exists = entries
        .iter()
        .any(|entry| entry.as_slice() == lock_name.as_bytes());
    if !lock_exists && !entries.is_empty() {
        return Err(AuthorityError::UnsafeConfiguration);
    }

    let lock_fd = if lock_exists {
        openat(
            root.library_root_fd(),
            lock_name.as_str(),
            OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?
    } else {
        fault.at(BlobMutationPoint::BeforeLockCreate)?;
        match openat(
            root.library_root_fd(),
            lock_name.as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::EXIST) => openat(
                root.library_root_fd(),
                lock_name.as_str(),
                OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| AuthorityError::UnsafeConfiguration)?,
            Err(rustix::io::Errno::LOOP) => return Err(AuthorityError::UnsafeConfiguration),
            Err(_) => return Err(AuthorityError::Io),
        }
    };
    let lock = ExclusiveBlobLock::acquire(File::from(lock_fd))?;
    if !lock_exists {
        fault.at(BlobMutationPoint::AfterLockCreateAndLock)?;
    }
    inspect_internal_file(lock.as_fd(), root.owner_uid)?;
    if fstat(lock.as_fd())
        .map_err(|_| AuthorityError::Io)?
        .st_nlink
        != 1
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    exact_final_component(lock.as_fd(), lock_name.as_bytes())?;

    fault.at(BlobMutationPoint::BeforeLockFileSync)?;
    fcntl_fullfsync(lock.as_fd()).map_err(|_| AuthorityError::Io)?;
    fault.at(BlobMutationPoint::AfterLockFileSync)?;
    fault.at(BlobMutationPoint::BeforeLockRootSync)?;
    fcntl_fullfsync(root.library_root_fd()).map_err(|_| AuthorityError::Io)?;
    fault.at(BlobMutationPoint::AfterLockRootSync)?;
    root.revalidate_chain()?;

    fault.at(BlobMutationPoint::BeforeLockedReenumerate)?;
    let mut state = enumerate_top(&root)?;
    state.sort_unstable();
    fault.at(BlobMutationPoint::AfterLockedReenumerate)?;
    let lock_only = vec![lock_name.as_bytes().to_vec()];
    let lock_staging = sorted_names(&[lock_name.as_str(), STAGING_DIRECTORY]);
    let complete = sorted_names(&[lock_name.as_str(), STAGING_DIRECTORY, CAS_DIRECTORY]);
    let (staging, cas) = if state == lock_only {
        let staging =
            create_fixed_directory(&root, STAGING_DIRECTORY, STAGING_MUTATION_POINTS, fault)?;
        (
            staging,
            create_fixed_directory(&root, CAS_DIRECTORY, CAS_MUTATION_POINTS, fault)?,
        )
    } else if state == lock_staging {
        let staging = open_fixed_directory(&root, STAGING_DIRECTORY)?;
        if enumerate_directory(staging.as_fd(), 1)?.is_empty() {
            (
                staging,
                create_fixed_directory(&root, CAS_DIRECTORY, CAS_MUTATION_POINTS, fault)?,
            )
        } else {
            return Err(AuthorityError::UnsafeConfiguration);
        }
    } else if state == complete {
        (
            open_fixed_directory(&root, STAGING_DIRECTORY)?,
            open_fixed_directory(&root, CAS_DIRECTORY)?,
        )
    } else {
        return Err(AuthorityError::UnsafeConfiguration);
    };

    root.revalidate_chain()?;
    if enumerate_top(&root)? != complete {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let mut hasher = Sha256::new();
    hasher.update(b"mengxia.local-cas-instance-v1\0");
    hasher.update(library_id);
    hasher.update(root.root_device.to_be_bytes());
    hasher.update(root.root_inode.to_be_bytes());
    let backend_instance_digest = hasher.finalize().into();

    Ok(OpenedBlobRootAuthority {
        request_identity: request.identity,
        library,
        root,
        _blob_lock: lock,
        _library_lock_lease: library_lock_lease,
        staging,
        cas,
        library_id,
        backend_instance_digest,
    })
}

fn create_fixed_directory(
    root: &ValidatedAbsolutePath,
    name: &str,
    points: DirectoryMutationPoints,
    fault: &mut impl BlobMutationFault,
) -> Result<OwnedFd, AuthorityError> {
    fault.at(points.before_mkdir)?;
    mkdirat(root.library_root_fd(), name, Mode::RWXU).map_err(|error| match error {
        rustix::io::Errno::EXIST => AuthorityError::UnsafeConfiguration,
        _ => AuthorityError::Io,
    })?;
    fault.at(points.after_mkdir)?;
    let fd = open_fixed_directory(root, name)?;
    fault.at(points.before_child_sync)?;
    fcntl_fullfsync(fd.as_fd()).map_err(|_| AuthorityError::Io)?;
    fault.at(points.after_child_sync)?;
    fault.at(points.before_parent_sync)?;
    fcntl_fullfsync(root.library_root_fd()).map_err(|_| AuthorityError::Io)?;
    fault.at(points.after_parent_sync)?;
    Ok(fd)
}

fn open_fixed_directory(
    root: &ValidatedAbsolutePath,
    name: &str,
) -> Result<OwnedFd, AuthorityError> {
    let fd = openat(
        root.library_root_fd(),
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    validate_named_directory(root, name, fd.as_fd())?;
    Ok(fd)
}

fn validate_named_directory(
    root: &ValidatedAbsolutePath,
    name: &str,
    fd: std::os::fd::BorrowedFd<'_>,
) -> Result<(), AuthorityError> {
    let security = inspect_directory(fd)?;
    validate_component_policy(security, ComponentRole::LibraryRoot, root.owner_uid)?;
    if security.device != root.root_device {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    exact_final_component(fd, name.as_bytes())?;
    let reopened = openat(
        root.library_root_fd(),
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let reopened_security = inspect_directory(reopened.as_fd())?;
    if !reopened_security.same_object(security) {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(())
}

pub(super) fn exact_final_component(
    fd: std::os::fd::BorrowedFd<'_>,
    expected: &[u8],
) -> Result<(), AuthorityError> {
    let path = getpath(fd).map_err(|_| AuthorityError::Io)?;
    let actual = Path::new(OsStr::from_bytes(path.as_bytes()))
        .file_name()
        .map(OsStr::as_bytes)
        .ok_or(AuthorityError::UnsafeConfiguration)?;
    if actual != expected {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(())
}

fn enumerate_top(root: &ValidatedAbsolutePath) -> Result<Vec<Vec<u8>>, AuthorityError> {
    let mut names = enumerate_directory(root.library_root_fd(), 3)?;
    names.sort_unstable();
    Ok(names)
}

fn enumerate_directory(
    fd: std::os::fd::BorrowedFd<'_>,
    limit: usize,
) -> Result<Vec<Vec<u8>>, AuthorityError> {
    let directory = Dir::read_from(fd).map_err(|_| AuthorityError::Io)?;
    let mut names = Vec::with_capacity(limit);
    for entry in directory {
        let entry = entry.map_err(|_| AuthorityError::Io)?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            if names.len() == limit {
                return Err(AuthorityError::UnsafeConfiguration);
            }
            names.push(name.to_vec());
        }
    }
    Ok(names)
}

fn lock_name(library_id: [u8; 16]) -> String {
    use std::fmt::Write as _;

    let mut result = String::with_capacity(LOCK_PREFIX.len() + 32);
    result.push_str(LOCK_PREFIX);
    for byte in library_id {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}

fn sorted_names(names: &[&str]) -> Vec<Vec<u8>> {
    let mut names: Vec<Vec<u8>> = names.iter().map(|name| name.as_bytes().to_vec()).collect();
    names.sort_unstable();
    names
}

fn validate_source_path(path: &Path) -> Result<(), BlobFileError> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_SOURCE_PATH_BYTES
        || bytes.contains(&0)
        || path == Path::new("/")
        || !path.is_absolute()
        || path.components().any(|component| match component {
            Component::CurDir | Component::ParentDir => true,
            Component::Normal(name) => name.as_bytes().is_empty() || name.as_bytes().len() > 255,
            Component::RootDir => false,
            Component::Prefix(_) => true,
        })
    {
        return Err(BlobFileError::InvalidPath);
    }
    let rebuilt: PathBuf = path.components().collect();
    if rebuilt != path || bytes.windows(2).any(|window| window == b"//") {
        return Err(BlobFileError::InvalidPath);
    }
    Ok(())
}

fn source_snapshot(fd: std::os::fd::BorrowedFd<'_>) -> Result<FileSnapshot, BlobFileError> {
    let stat = fstat(fd).map_err(|_| BlobFileError::Io)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(BlobFileError::UnsupportedType);
    }
    Ok(FileSnapshot {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
        length: u64::try_from(stat.st_size).map_err(|_| BlobFileError::Modified)?,
        modified_seconds: stat.st_mtime,
        modified_nanos: stat.st_mtime_nsec as i64,
        changed_seconds: stat.st_ctime,
        changed_nanos: stat.st_ctime_nsec as i64,
        link_count: u64::from(stat.st_nlink),
    })
}

fn validate_blob_file(
    fd: std::os::fd::BorrowedFd<'_>,
    owner_uid: u32,
) -> Result<super::MacOsObjectSecurity, BlobFileError> {
    let security =
        inspect_internal_file(fd, owner_uid).map_err(|_| BlobFileError::Configuration)?;
    let stat = fstat(fd).map_err(|_| BlobFileError::Io)?;
    if stat.st_nlink != 1 {
        return Err(BlobFileError::Configuration);
    }
    Ok(security)
}

fn map_authority_error(error: AuthorityError) -> BlobFileError {
    match error {
        AuthorityError::Io => BlobFileError::Io,
        AuthorityError::ConflictingData => BlobFileError::Corruption,
        AuthorityError::UnsafeConfiguration | AuthorityError::Contended => {
            BlobFileError::Configuration
        }
    }
}

fn retry_interrupted<T>(
    mut operation: impl FnMut() -> rustix::io::Result<T>,
) -> rustix::io::Result<T> {
    let mut interruptions = 0;
    loop {
        match operation() {
            Err(rustix::io::Errno::INTR) if interruptions < MAX_INTERRUPTED_SYSCALL_RETRIES => {
                interruptions += 1;
            }
            result => return result,
        }
    }
}

fn staging_name(random: [u8; 16]) -> String {
    use std::fmt::Write as _;

    let mut name = String::with_capacity(45);
    name.push_str(".ingest-");
    for byte in random {
        write!(&mut name, "{byte:02x}").expect("writing to String cannot fail");
    }
    name.push_str(".part");
    name
}

fn valid_staging_name(name: &[u8]) -> bool {
    name.len() == 45
        && name.starts_with(b".ingest-")
        && name.ends_with(b".part")
        && name[8..40]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

#[derive(Clone, Copy)]
struct ShardMutationPoints {
    before_mkdir_open: BlobFileMutationPoint,
    after_mkdir_open: BlobFileMutationPoint,
    before_syncs: BlobFileMutationPoint,
    after_child_sync: BlobFileMutationPoint,
    before_parent_sync: BlobFileMutationPoint,
    after_syncs: BlobFileMutationPoint,
}

impl ShardMutationPoints {
    const FIRST: Self = Self {
        before_mkdir_open: BlobFileMutationPoint::BeforeFirstShardMkdirOpen,
        after_mkdir_open: BlobFileMutationPoint::AfterFirstShardMkdirOpen,
        before_syncs: BlobFileMutationPoint::BeforeFirstShardSyncs,
        after_child_sync: BlobFileMutationPoint::AfterFirstShardChildSync,
        before_parent_sync: BlobFileMutationPoint::BeforeFirstShardParentSync,
        after_syncs: BlobFileMutationPoint::AfterFirstShardSyncs,
    };
    const SECOND: Self = Self {
        before_mkdir_open: BlobFileMutationPoint::BeforeSecondShardMkdirOpen,
        after_mkdir_open: BlobFileMutationPoint::AfterSecondShardMkdirOpen,
        before_syncs: BlobFileMutationPoint::BeforeSecondShardSyncs,
        after_child_sync: BlobFileMutationPoint::AfterSecondShardChildSync,
        before_parent_sync: BlobFileMutationPoint::BeforeSecondShardParentSync,
        after_syncs: BlobFileMutationPoint::AfterSecondShardSyncs,
    };
}

fn lowercase_hex(bytes: [u8; 32]) -> String {
    use std::fmt::Write as _;

    let mut result = String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("writing to String cannot fail");
    }
    result
}

fn open_or_create_shard(
    authority: &OpenedBlobRootAuthority,
    parent: std::os::fd::BorrowedFd<'_>,
    name: &str,
    points: ShardMutationPoints,
    fault: &mut impl BlobFileMutationFault,
) -> Result<OwnedFd, BlobFileError> {
    fault.at(points.before_mkdir_open)?;
    let created = match mkdirat(parent, name, Mode::RWXU) {
        Ok(()) => true,
        Err(rustix::io::Errno::EXIST) => false,
        Err(_) => return Err(BlobFileError::Io),
    };
    let fd = openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| BlobFileError::Configuration)?;
    let security = inspect_directory(fd.as_fd()).map_err(|_| BlobFileError::Configuration)?;
    validate_component_policy(
        security,
        ComponentRole::LibraryRoot,
        authority.root.owner_uid,
    )
    .map_err(|_| BlobFileError::Configuration)?;
    if security.device != authority.root.root_device {
        return Err(BlobFileError::Configuration);
    }
    exact_final_component(fd.as_fd(), name.as_bytes()).map_err(|_| BlobFileError::Configuration)?;
    let reopened = openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| BlobFileError::Configuration)?;
    let reopened = inspect_directory(reopened.as_fd()).map_err(|_| BlobFileError::Configuration)?;
    if !reopened.same_object(security) {
        return Err(BlobFileError::Configuration);
    }
    fault.at(points.after_mkdir_open)?;
    if created {
        fault.at(points.before_syncs)?;
        fcntl_fullfsync(fd.as_fd()).map_err(|_| BlobFileError::Io)?;
        fault.at(points.after_child_sync)?;
        fault.at(points.before_parent_sync)?;
        fcntl_fullfsync(parent).map_err(|_| BlobFileError::Io)?;
        fault.at(points.after_syncs)?;
    }
    Ok(fd)
}

fn verify_existing_blob(
    authority: &OpenedBlobRootAuthority,
    parent: std::os::fd::BorrowedFd<'_>,
    name: &str,
    expected_digest: [u8; 32],
    expected_length: u64,
    buffer_bytes: usize,
) -> Result<(), BlobFileError> {
    let fd = openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| BlobFileError::Corruption)?;
    let security = validate_blob_file(fd.as_fd(), authority.root.owner_uid)
        .map_err(|_| BlobFileError::Corruption)?;
    if security.device != authority.root.root_device {
        return Err(BlobFileError::Corruption);
    }
    exact_final_component(fd.as_fd(), name.as_bytes()).map_err(|_| BlobFileError::Configuration)?;
    let before = source_snapshot(fd.as_fd()).map_err(|_| BlobFileError::Corruption)?;
    if before.length != expected_length {
        return Err(BlobFileError::Corruption);
    }
    let mut hasher = Sha256::new();
    let mut offset = 0_u64;
    if buffer_bytes == 0 {
        return Err(BlobFileError::Configuration);
    }
    let mut buffer = vec![0_u8; buffer_bytes];
    while offset < expected_length {
        let remaining = usize::try_from((expected_length - offset).min(buffer.len() as u64))
            .map_err(|_| BlobFileError::Corruption)?;
        let read = retry_interrupted(|| pread(fd.as_fd(), &mut buffer[..remaining], offset))
            .map_err(|_| BlobFileError::Io)?;
        if read == 0 {
            return Err(BlobFileError::Corruption);
        }
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(read as u64)
            .ok_or(BlobFileError::Corruption)?;
    }
    let mut eof = [0_u8; 1];
    if retry_interrupted(|| pread(fd.as_fd(), &mut eof, expected_length))
        .map_err(|_| BlobFileError::Io)?
        != 0
    {
        return Err(BlobFileError::Corruption);
    }
    if source_snapshot(fd.as_fd()).map_err(|_| BlobFileError::Corruption)? != before
        || <[u8; 32]>::from(hasher.finalize()) != expected_digest
    {
        return Err(BlobFileError::Corruption);
    }
    let reopened = openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| BlobFileError::Corruption)?;
    let reopened = source_snapshot(reopened.as_fd()).map_err(|_| BlobFileError::Corruption)?;
    if reopened.device != before.device || reopened.inode != before.inode {
        return Err(BlobFileError::Corruption);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use super::*;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct FailAt {
        target: BlobMutationPoint,
        visited: Vec<BlobMutationPoint>,
    }

    impl BlobMutationFault for FailAt {
        fn at(&mut self, point: BlobMutationPoint) -> Result<(), AuthorityError> {
            self.visited.push(point);
            if point == self.target {
                Err(AuthorityError::Io)
            } else {
                Ok(())
            }
        }
    }

    struct PauseAt {
        target: BlobMutationPoint,
        ready: PathBuf,
    }

    struct FailFileAt {
        target: BlobFileMutationPoint,
        visited: Vec<BlobFileMutationPoint>,
    }

    impl BlobFileMutationFault for FailFileAt {
        fn at(&mut self, point: BlobFileMutationPoint) -> Result<(), BlobFileError> {
            self.visited.push(point);
            if point == self.target {
                Err(BlobFileError::Io)
            } else {
                Ok(())
            }
        }

        fn no_replace_result(&mut self) -> Option<rustix::io::Result<()>> {
            if self.target == BlobFileMutationPoint::NoReplaceError {
                self.visited.push(BlobFileMutationPoint::NoReplaceError);
                Some(Err(rustix::io::Errno::IO))
            } else {
                None
            }
        }
    }

    struct PauseFileAt {
        target: BlobFileMutationPoint,
        ready: PathBuf,
    }

    impl BlobFileMutationFault for PauseFileAt {
        fn at(&mut self, point: BlobFileMutationPoint) -> Result<(), BlobFileError> {
            if point == self.target {
                let ready = File::create(&self.ready).expect("create file crash acknowledgement");
                ready.sync_all().expect("sync file crash acknowledgement");
                loop {
                    std::thread::park();
                }
            }
            Ok(())
        }
    }

    impl BlobMutationFault for PauseAt {
        fn at(&mut self, point: BlobMutationPoint) -> Result<(), AuthorityError> {
            if point == self.target {
                let ready = File::create(&self.ready).expect("create crash-point acknowledgement");
                ready.sync_all().expect("sync crash-point acknowledgement");
                loop {
                    std::thread::park();
                }
            }
            Ok(())
        }
    }

    struct Fixture {
        base: PathBuf,
        library: PathBuf,
        blob: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let home = fs::canonicalize(PathBuf::from(std::env::var_os("HOME").unwrap())).unwrap();
            let base = home.join(format!(
                ".mengxia-task005-platform-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let library = base.join("Library");
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&library)
                .unwrap();
            fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
            fs::set_permissions(&library, fs::Permissions::from_mode(0o700)).unwrap();
            let blob = library.join("storage");
            Self {
                base,
                library,
                blob,
            }
        }

        fn inputs(
            &self,
        ) -> (
            Arc<ValidatedAbsolutePath>,
            Arc<LibraryLockLease>,
            BlobRootRequest,
        ) {
            authority_inputs(&self.library, &self.blob)
        }

        fn names(&self) -> Vec<Vec<u8>> {
            if !self.blob.exists() {
                return Vec::new();
            }
            let mut names: Vec<Vec<u8>> = fs::read_dir(&self.blob)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().as_encoded_bytes().to_vec())
                .collect();
            names.sort_unstable();
            names
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn authority_inputs(
        library_path: &Path,
        blob_path: &Path,
    ) -> (
        Arc<ValidatedAbsolutePath>,
        Arc<LibraryLockLease>,
        BlobRootRequest,
    ) {
        let library = Arc::new(
            ValidatedAbsolutePath::authorize_existing(library_path)
                .expect("authorize fixture Library"),
        );
        let lease = Arc::new(LibraryLockLease {
            lock_file: File::open(library_path).expect("retain fixture lease descriptor"),
        });
        let request = BlobRootRequest::from_absolute_path(blob_path).unwrap();
        (library, lease, request)
    }

    fn initialized_authority(fixture: &Fixture) -> OpenedBlobRootAuthority {
        let (library, lease, request) = fixture.inputs();
        authorize_blob_root(library, lease, &request, [0x5a; 16]).unwrap()
    }

    fn staging_with_bytes(
        authority: &OpenedBlobRootAuthority,
        random: u8,
        bytes: &[u8],
    ) -> OpenedBlobStaging {
        let staging = authority.create_staging([random; 16]).unwrap();
        let mut offset = 0;
        while offset < bytes.len() {
            let written = staging.write_at(&bytes[offset..], offset as u64).unwrap();
            assert_ne!(written, 0);
            offset += written;
        }
        staging
    }

    fn digest_paths(fixture: &Fixture, digest: [u8; 32]) -> (PathBuf, PathBuf, PathBuf) {
        let hex = lowercase_hex(digest);
        let first = fixture.blob.join("sha256-v1").join(&hex[..2]);
        let second = first.join(&hex[2..4]);
        let canonical = second.join(format!("{hex}.blob"));
        (first, second, canonical)
    }

    #[test]
    fn task_005_blob_root_initialization_fault_matrix_is_ordered_and_recoverable() {
        let library_id = [0x5a; 16];
        let cases: &[(u8, BlobMutationPoint, &[&str])] = &[
            (1, BlobMutationPoint::BeforeRootMkdir, &[]),
            (2, BlobMutationPoint::AfterRootMkdir, &[]),
            (3, BlobMutationPoint::BeforeRootSync, &[]),
            (69, BlobMutationPoint::AfterRootChildSync, &[]),
            (70, BlobMutationPoint::BeforeRootParentSync, &[]),
            (4, BlobMutationPoint::AfterRootSync, &[]),
            (5, BlobMutationPoint::BeforeLockCreate, &[]),
            (
                6,
                BlobMutationPoint::AfterLockCreateAndLock,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                7,
                BlobMutationPoint::BeforeLockFileSync,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                8,
                BlobMutationPoint::AfterLockFileSync,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                9,
                BlobMutationPoint::BeforeLockRootSync,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                10,
                BlobMutationPoint::AfterLockRootSync,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                11,
                BlobMutationPoint::BeforeLockedReenumerate,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                12,
                BlobMutationPoint::AfterLockedReenumerate,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                13,
                BlobMutationPoint::BeforeStagingMkdir,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                14,
                BlobMutationPoint::AfterStagingMkdir,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                ],
            ),
            (
                15,
                BlobMutationPoint::BeforeStagingChildSync,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                ],
            ),
            (
                71,
                BlobMutationPoint::AfterStagingChildSync,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                ],
            ),
            (
                72,
                BlobMutationPoint::BeforeStagingParentSync,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                ],
            ),
            (
                16,
                BlobMutationPoint::AfterStagingParentSync,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                ],
            ),
            (
                17,
                BlobMutationPoint::BeforeCasMkdir,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                ],
            ),
            (
                18,
                BlobMutationPoint::AfterCasMkdir,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                    "sha256-v1",
                ],
            ),
            (
                19,
                BlobMutationPoint::BeforeCasChildSync,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                    "sha256-v1",
                ],
            ),
            (
                73,
                BlobMutationPoint::AfterCasChildSync,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                    "sha256-v1",
                ],
            ),
            (
                74,
                BlobMutationPoint::BeforeCasParentSync,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                    "sha256-v1",
                ],
            ),
            (
                20,
                BlobMutationPoint::AfterCasParentSync,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                    "sha256-v1",
                ],
            ),
        ];

        for &(fault_id, point, expected_names) in cases {
            let fixture = Fixture::new();
            let (library, lease, request) = fixture.inputs();
            let mut fault = FailAt {
                target: point,
                visited: Vec::new(),
            };
            assert_eq!(
                authorize_blob_root_with(
                    Arc::clone(&library),
                    Arc::clone(&lease),
                    &request,
                    library_id,
                    &mut fault,
                )
                .err(),
                Some(AuthorityError::Io),
                "FAULT-005-{fault_id:03} must return the injected error"
            );
            assert_eq!(fault.visited.last(), Some(&point));
            let mut expected: Vec<Vec<u8>> = expected_names
                .iter()
                .map(|name| name.as_bytes().to_vec())
                .collect();
            expected.sort_unstable();
            assert_eq!(fixture.names(), expected, "FAULT-005-{fault_id:03}");

            let recovered = authorize_blob_root(
                Arc::clone(&library),
                Arc::clone(&lease),
                &request,
                library_id,
            )
            .unwrap_or_else(|error| panic!("FAULT-005-{fault_id:03} recovery: {error}"));
            recovered.revalidate().unwrap();
            drop(recovered);
            assert_eq!(fixture.names().len(), 3);
        }
    }

    #[test]
    fn blob_lock_drop_unlocks_even_while_a_duplicate_descriptor_survives() {
        let fixture = Fixture::new();
        let lock_path = fixture.base.join("duplicate-lock");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&lock_path)
            .expect("create duplicate-descriptor lock fixture");
        let guard = ExclusiveBlobLock::acquire(file).expect("acquire fixture lock");
        let inherited = guard
            .file
            .try_clone()
            .expect("model a descriptor inherited across fork");

        drop(guard);

        let contender = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .expect("reopen duplicate-descriptor lock fixture");
        let contender = ExclusiveBlobLock::acquire(contender)
            .expect("logical authority release must not wait for inherited descriptor close");
        drop(contender);
        drop(inherited);
    }

    #[test]
    fn interrupted_syscall_retry_budget_is_exact_and_resets_per_operation() {
        let mut calls = 0;
        let error = retry_interrupted::<()>(|| {
            calls += 1;
            Err(rustix::io::Errno::INTR)
        });
        assert_eq!(error, Err(rustix::io::Errno::INTR), "FAULT-005-025/036");
        assert_eq!(calls, MAX_INTERRUPTED_SYSCALL_RETRIES + 1);

        let mut calls = 0;
        let value = retry_interrupted(|| {
            calls += 1;
            if calls <= MAX_INTERRUPTED_SYSCALL_RETRIES {
                Err(rustix::io::Errno::INTR)
            } else {
                Ok(7_u8)
            }
        });
        assert_eq!(value, Ok(7), "FAULT-005-026 positive progress");
        assert_eq!(calls, MAX_INTERRUPTED_SYSCALL_RETRIES + 1);

        let mut next_operation_calls = 0;
        assert_eq!(
            retry_interrupted(|| {
                next_operation_calls += 1;
                if next_operation_calls == 1 {
                    Err(rustix::io::Errno::INTR)
                } else {
                    Ok(1_u8)
                }
            }),
            Ok(1),
            "FAULT-005-026 retry budget resets for the next logical read"
        );
        assert_eq!(next_operation_calls, 2);
    }

    #[test]
    fn task_005_publish_fault_matrix_preserves_exact_no_clobber_prefixes() {
        let bytes = b"MengXia TASK-005 publish fault matrix";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let cases: &[(u8, BlobFileMutationPoint, bool, bool, bool, bool)] = &[
            (
                37,
                BlobFileMutationPoint::BeforeStagingFileSync,
                true,
                false,
                false,
                false,
            ),
            (
                38,
                BlobFileMutationPoint::AfterStagingFileSync,
                true,
                false,
                false,
                false,
            ),
            (
                39,
                BlobFileMutationPoint::BeforePrepromoteRevalidate,
                true,
                false,
                false,
                false,
            ),
            (
                40,
                BlobFileMutationPoint::AfterPrepromoteRevalidate,
                true,
                false,
                false,
                false,
            ),
            (
                41,
                BlobFileMutationPoint::BeforeFirstShardMkdirOpen,
                true,
                false,
                false,
                false,
            ),
            (
                42,
                BlobFileMutationPoint::AfterFirstShardMkdirOpen,
                true,
                true,
                false,
                false,
            ),
            (
                43,
                BlobFileMutationPoint::BeforeFirstShardSyncs,
                true,
                true,
                false,
                false,
            ),
            (
                75,
                BlobFileMutationPoint::AfterFirstShardChildSync,
                true,
                true,
                false,
                false,
            ),
            (
                76,
                BlobFileMutationPoint::BeforeFirstShardParentSync,
                true,
                true,
                false,
                false,
            ),
            (
                44,
                BlobFileMutationPoint::AfterFirstShardSyncs,
                true,
                true,
                false,
                false,
            ),
            (
                45,
                BlobFileMutationPoint::BeforeSecondShardMkdirOpen,
                true,
                true,
                false,
                false,
            ),
            (
                46,
                BlobFileMutationPoint::AfterSecondShardMkdirOpen,
                true,
                true,
                true,
                false,
            ),
            (
                47,
                BlobFileMutationPoint::BeforeSecondShardSyncs,
                true,
                true,
                true,
                false,
            ),
            (
                77,
                BlobFileMutationPoint::AfterSecondShardChildSync,
                true,
                true,
                true,
                false,
            ),
            (
                78,
                BlobFileMutationPoint::BeforeSecondShardParentSync,
                true,
                true,
                true,
                false,
            ),
            (
                48,
                BlobFileMutationPoint::AfterSecondShardSyncs,
                true,
                true,
                true,
                false,
            ),
            (
                49,
                BlobFileMutationPoint::BeforeNoReplaceRename,
                true,
                true,
                true,
                false,
            ),
            (
                50,
                BlobFileMutationPoint::NoReplaceError,
                true,
                true,
                true,
                false,
            ),
            (
                51,
                BlobFileMutationPoint::AfterNoReplaceRename,
                false,
                true,
                true,
                true,
            ),
            (
                52,
                BlobFileMutationPoint::CanonicalReopenOrCaseProof,
                false,
                true,
                true,
                true,
            ),
            (
                53,
                BlobFileMutationPoint::BeforeDestinationSync,
                false,
                true,
                true,
                true,
            ),
            (
                54,
                BlobFileMutationPoint::AfterDestinationSync,
                false,
                true,
                true,
                true,
            ),
            (
                55,
                BlobFileMutationPoint::BeforePostPromoteStagingSync,
                false,
                true,
                true,
                true,
            ),
            (
                56,
                BlobFileMutationPoint::AfterPostPromoteStagingSync,
                false,
                true,
                true,
                true,
            ),
        ];
        for &(fault_id, point, has_staging, has_first, has_second, has_canonical) in cases {
            let fixture = Fixture::new();
            let authority = initialized_authority(&fixture);
            let staging = staging_with_bytes(&authority, 1, bytes);
            let staging_path = fixture
                .blob
                .join(".staging-v1/.ingest-01010101010101010101010101010101.part");
            let (first, second, canonical) = digest_paths(&fixture, digest);
            let mut fault = FailFileAt {
                target: point,
                visited: Vec::new(),
            };
            assert_eq!(
                authority
                    .commit_staging_with(&staging, digest, bytes.len() as u64, 1024, &mut fault)
                    .err(),
                Some(BlobFileError::Io),
                "FAULT-005-{fault_id:03}"
            );
            assert_eq!(fault.visited.last(), Some(&point));
            assert_eq!(
                staging_path.exists(),
                has_staging,
                "FAULT-005-{fault_id:03}"
            );
            assert_eq!(first.exists(), has_first, "FAULT-005-{fault_id:03}");
            assert_eq!(second.exists(), has_second, "FAULT-005-{fault_id:03}");
            assert_eq!(canonical.exists(), has_canonical, "FAULT-005-{fault_id:03}");
            if has_canonical {
                assert_eq!(fs::read(canonical).unwrap(), bytes);
            }
        }
    }

    #[test]
    fn task_005_source_authority_fault_matrix_is_read_only_and_bounded() {
        let cases = [
            (21, BlobFileMutationPoint::BeforeSourceWalk),
            (22, BlobFileMutationPoint::AfterSourceOpen),
            (23, BlobFileMutationPoint::BeforeSourceRevalidate),
            (24, BlobFileMutationPoint::AfterSourceRevalidate),
        ];
        for (fault_id, point) in cases {
            let fixture = Fixture::new();
            let source = fixture.base.join("source.bin");
            let bytes = b"source authority remains immutable";
            fs::write(&source, bytes).unwrap();
            fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
            let authority = initialized_authority(&fixture);
            let before = fs::metadata(&source).unwrap();
            let mut fault = FailFileAt {
                target: point,
                visited: Vec::new(),
            };
            assert_eq!(
                authority.open_source_with(&source, &mut fault).err(),
                Some(BlobFileError::Io),
                "FAULT-005-{fault_id:03}"
            );
            assert_eq!(fault.visited.last(), Some(&point));
            let after = fs::metadata(&source).unwrap();
            assert_eq!(before.len(), after.len());
            assert_eq!(fs::read(&source).unwrap(), bytes);
            assert_eq!(
                fs::read_dir(fixture.blob.join(".staging-v1"))
                    .unwrap()
                    .count(),
                0
            );
        }
    }

    #[test]
    fn task_005_dedup_and_cleanup_fault_matrix_never_removes_foreign_canonical_data() {
        let bytes = b"MengXia TASK-005 dedup fault matrix";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let cases: &[(u8, BlobFileMutationPoint, bool, BlobFileError)] = &[
            (
                57,
                BlobFileMutationPoint::BeforeExistingVerify,
                true,
                BlobFileError::Io,
            ),
            (
                58,
                BlobFileMutationPoint::AfterExistingVerify,
                true,
                BlobFileError::Io,
            ),
            (
                59,
                BlobFileMutationPoint::BeforeDedupStagingRevalidate,
                true,
                BlobFileError::Io,
            ),
            (
                60,
                BlobFileMutationPoint::AfterDedupStagingRevalidate,
                true,
                BlobFileError::Io,
            ),
            (
                61,
                BlobFileMutationPoint::BeforeCleanupRevalidate,
                true,
                BlobFileError::CleanupFailed,
            ),
            (
                62,
                BlobFileMutationPoint::BeforeCleanupUnlink,
                true,
                BlobFileError::CleanupFailed,
            ),
            (
                63,
                BlobFileMutationPoint::AfterCleanupUnlink,
                false,
                BlobFileError::CleanupFailed,
            ),
        ];
        for &(fault_id, point, has_staging, expected_error) in cases {
            let fixture = Fixture::new();
            let authority = initialized_authority(&fixture);
            let canonical_staging = staging_with_bytes(&authority, 1, bytes);
            assert_eq!(
                authority
                    .commit_staging(&canonical_staging, digest, bytes.len() as u64, 1024)
                    .unwrap(),
                BlobCommitOutcome::Published
            );
            let duplicate = staging_with_bytes(&authority, 2, bytes);
            let duplicate_path = fixture
                .blob
                .join(".staging-v1/.ingest-02020202020202020202020202020202.part");
            let (_, _, canonical) = digest_paths(&fixture, digest);
            let mut fault = FailFileAt {
                target: point,
                visited: Vec::new(),
            };
            assert_eq!(
                authority
                    .commit_staging_with(&duplicate, digest, bytes.len() as u64, 1024, &mut fault)
                    .err(),
                Some(expected_error),
                "FAULT-005-{fault_id:03}"
            );
            assert_eq!(fault.visited.last(), Some(&point));
            assert_eq!(
                duplicate_path.exists(),
                has_staging,
                "FAULT-005-{fault_id:03}"
            );
            assert_eq!(
                fs::read(canonical).unwrap(),
                bytes,
                "FAULT-005-{fault_id:03}"
            );
        }
    }

    #[test]
    fn task_005_blob_root_sigkill_matrix_has_exact_same_os_restart_states() {
        let cases: &[(u8, BlobMutationPoint, bool, &[&str])] = &[
            (1, BlobMutationPoint::BeforeRootMkdir, false, &[]),
            (2, BlobMutationPoint::AfterRootMkdir, true, &[]),
            (3, BlobMutationPoint::AfterRootSync, true, &[]),
            (
                4,
                BlobMutationPoint::AfterLockCreateAndLock,
                true,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                5,
                BlobMutationPoint::AfterLockFileSync,
                true,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                6,
                BlobMutationPoint::AfterLockRootSync,
                true,
                &[".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"],
            ),
            (
                7,
                BlobMutationPoint::AfterStagingMkdir,
                true,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                ],
            ),
            (
                8,
                BlobMutationPoint::AfterStagingParentSync,
                true,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                ],
            ),
            (
                9,
                BlobMutationPoint::AfterCasMkdir,
                true,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                    "sha256-v1",
                ],
            ),
            (
                10,
                BlobMutationPoint::AfterCasParentSync,
                true,
                &[
                    ".mengxia-cas-v1-5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a",
                    ".staging-v1",
                    "sha256-v1",
                ],
            ),
        ];
        for &(kill_id, point, root_exists, expected_names) in cases {
            let fixture = Fixture::new();
            let ready = fixture.base.join("ready");
            let mut child = Command::new(std::env::current_exe().unwrap())
                .arg("blob_storage::tests::task_005_blob_root_crash_child_entrypoint")
                .arg("--exact")
                .arg("--nocapture")
                .env("MENGXIA_TASK005_PLATFORM_CRASH_POINT", format!("{point:?}"))
                .env("MENGXIA_TASK005_PLATFORM_LIBRARY", &fixture.library)
                .env("MENGXIA_TASK005_PLATFORM_BLOB", &fixture.blob)
                .env("MENGXIA_TASK005_PLATFORM_READY", &ready)
                .spawn()
                .expect("spawn TASK-005 crash child");
            wait_for_ready(&mut child, &ready);
            child.kill().expect("SIGKILL TASK-005 crash child");
            let _ = child.wait().expect("reap TASK-005 crash child");

            assert_eq!(
                fixture.blob.exists(),
                root_exists,
                "KILL-005-{kill_id:03} root visibility"
            );
            let mut expected: Vec<Vec<u8>> = expected_names
                .iter()
                .map(|name| name.as_bytes().to_vec())
                .collect();
            expected.sort_unstable();
            assert_eq!(fixture.names(), expected, "KILL-005-{kill_id:03}");

            let (library, lease, request) = fixture.inputs();
            let recovered = authorize_blob_root(library, lease, &request, [0x5a; 16])
                .unwrap_or_else(|error| panic!("KILL-005-{kill_id:03} recovery: {error}"));
            recovered.revalidate().unwrap();
        }
    }

    #[test]
    fn task_005_blob_root_crash_child_entrypoint() {
        let Some(point) = std::env::var_os("MENGXIA_TASK005_PLATFORM_CRASH_POINT") else {
            return;
        };
        let point = match point.to_str().expect("ASCII crash point") {
            "BeforeRootMkdir" => BlobMutationPoint::BeforeRootMkdir,
            "AfterRootMkdir" => BlobMutationPoint::AfterRootMkdir,
            "AfterRootSync" => BlobMutationPoint::AfterRootSync,
            "AfterLockCreateAndLock" => BlobMutationPoint::AfterLockCreateAndLock,
            "AfterLockFileSync" => BlobMutationPoint::AfterLockFileSync,
            "AfterLockRootSync" => BlobMutationPoint::AfterLockRootSync,
            "AfterStagingMkdir" => BlobMutationPoint::AfterStagingMkdir,
            "AfterStagingParentSync" => BlobMutationPoint::AfterStagingParentSync,
            "AfterCasMkdir" => BlobMutationPoint::AfterCasMkdir,
            "AfterCasParentSync" => BlobMutationPoint::AfterCasParentSync,
            other => panic!("unknown crash point {other}"),
        };
        let library = PathBuf::from(
            std::env::var_os("MENGXIA_TASK005_PLATFORM_LIBRARY").expect("crash Library"),
        );
        let blob = PathBuf::from(
            std::env::var_os("MENGXIA_TASK005_PLATFORM_BLOB").expect("crash Blob root"),
        );
        let ready = PathBuf::from(
            std::env::var_os("MENGXIA_TASK005_PLATFORM_READY").expect("crash ready path"),
        );
        let (library, lease, request) = authority_inputs(&library, &blob);
        let mut pause = PauseAt {
            target: point,
            ready,
        };
        let result = authorize_blob_root_with(library, lease, &request, [0x5a; 16], &mut pause);
        match result {
            Ok(_) => panic!("crash point was not reached before successful initialization"),
            Err(error) => panic!("crash point was not reached: {error}"),
        }
    }

    #[test]
    fn task_005_blob_file_sigkill_matrix_has_exact_same_os_restart_states() {
        let bytes = b"MengXia TASK-005 crash matrix bytes";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let cases: &[(u8, BlobFileMutationPoint, bool, bool, bool, bool, u64)] = &[
            (
                12,
                BlobFileMutationPoint::AfterStagingCreate,
                true,
                false,
                false,
                false,
                0,
            ),
            (
                13,
                BlobFileMutationPoint::AfterStagingNameSync,
                true,
                false,
                false,
                false,
                0,
            ),
            (
                19,
                BlobFileMutationPoint::AfterStagingFileSync,
                true,
                false,
                false,
                false,
                bytes.len() as u64,
            ),
            (
                20,
                BlobFileMutationPoint::AfterFirstShardMkdirOpen,
                true,
                true,
                false,
                false,
                bytes.len() as u64,
            ),
            (
                21,
                BlobFileMutationPoint::AfterFirstShardSyncs,
                true,
                true,
                false,
                false,
                bytes.len() as u64,
            ),
            (
                22,
                BlobFileMutationPoint::AfterSecondShardMkdirOpen,
                true,
                true,
                true,
                false,
                bytes.len() as u64,
            ),
            (
                23,
                BlobFileMutationPoint::AfterSecondShardSyncs,
                true,
                true,
                true,
                false,
                bytes.len() as u64,
            ),
            (
                24,
                BlobFileMutationPoint::AfterNoReplaceRename,
                false,
                true,
                true,
                true,
                0,
            ),
            (
                25,
                BlobFileMutationPoint::AfterDestinationSync,
                false,
                true,
                true,
                true,
                0,
            ),
            (
                26,
                BlobFileMutationPoint::AfterPostPromoteStagingSync,
                false,
                true,
                true,
                true,
                0,
            ),
            (
                27,
                BlobFileMutationPoint::AfterExistingVerify,
                true,
                true,
                true,
                true,
                bytes.len() as u64,
            ),
            (
                28,
                BlobFileMutationPoint::AfterCleanupUnlink,
                false,
                true,
                true,
                true,
                0,
            ),
            (
                29,
                BlobFileMutationPoint::AfterCleanupPostUnlinkSync,
                false,
                true,
                true,
                true,
                0,
            ),
        ];
        for &(kill_id, point, has_staging, has_first, has_second, has_canonical, orphan_bytes) in
            cases
        {
            let fixture = Fixture::new();
            let ready = fixture.base.join("file-ready");
            let mut child = Command::new(std::env::current_exe().unwrap())
                .arg("blob_storage::tests::task_005_blob_file_crash_child_entrypoint")
                .arg("--exact")
                .arg("--nocapture")
                .env("MENGXIA_TASK005_FILE_CRASH_POINT", format!("{point:?}"))
                .env("MENGXIA_TASK005_PLATFORM_LIBRARY", &fixture.library)
                .env("MENGXIA_TASK005_PLATFORM_BLOB", &fixture.blob)
                .env("MENGXIA_TASK005_PLATFORM_READY", &ready)
                .spawn()
                .expect("spawn TASK-005 file crash child");
            wait_for_ready(&mut child, &ready);
            child.kill().expect("SIGKILL TASK-005 file crash child");
            let _ = child.wait().expect("reap TASK-005 file crash child");

            let staging_path = fixture
                .blob
                .join(".staging-v1/.ingest-01010101010101010101010101010101.part");
            let (first, second, canonical) = digest_paths(&fixture, digest);
            assert_eq!(staging_path.exists(), has_staging, "KILL-005-{kill_id:03}");
            assert_eq!(first.exists(), has_first, "KILL-005-{kill_id:03}");
            assert_eq!(second.exists(), has_second, "KILL-005-{kill_id:03}");
            assert_eq!(canonical.exists(), has_canonical, "KILL-005-{kill_id:03}");
            if has_staging {
                assert_eq!(fs::metadata(&staging_path).unwrap().len(), orphan_bytes);
            }
            if has_canonical {
                assert_eq!(fs::read(&canonical).unwrap(), bytes);
            }

            let authority = initialized_authority(&fixture);
            let orphans = authority.observe_staging_orphans().unwrap();
            assert_eq!(orphans.count, u16::from(has_staging));
            assert_eq!(orphans.bytes, orphan_bytes);
            if has_canonical {
                let retry = staging_with_bytes(&authority, 3, bytes);
                assert_eq!(
                    authority
                        .commit_staging(&retry, digest, bytes.len() as u64, 1024)
                        .unwrap(),
                    BlobCommitOutcome::ExistingVerified,
                    "KILL-005-{kill_id:03} retry"
                );
            }
        }
    }

    #[test]
    fn task_005_blob_file_crash_child_entrypoint() {
        let Some(point) = std::env::var_os("MENGXIA_TASK005_FILE_CRASH_POINT") else {
            return;
        };
        let point = parse_file_crash_point(point.to_str().expect("ASCII file crash point"));
        let library = PathBuf::from(
            std::env::var_os("MENGXIA_TASK005_PLATFORM_LIBRARY").expect("crash Library"),
        );
        let blob = PathBuf::from(
            std::env::var_os("MENGXIA_TASK005_PLATFORM_BLOB").expect("crash Blob root"),
        );
        let ready = PathBuf::from(
            std::env::var_os("MENGXIA_TASK005_PLATFORM_READY").expect("crash ready path"),
        );
        let (library, lease, request) = authority_inputs(&library, &blob);
        let authority = authorize_blob_root(library, lease, &request, [0x5a; 16]).unwrap();
        let bytes = b"MengXia TASK-005 crash matrix bytes";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let mut pause = PauseFileAt {
            target: point,
            ready,
        };
        if matches!(
            point,
            BlobFileMutationPoint::AfterStagingCreate | BlobFileMutationPoint::AfterStagingNameSync
        ) {
            let result = authority.create_staging_with([1; 16], &mut pause);
            match result {
                Ok(_) => panic!("file crash point was not reached during staging creation"),
                Err(error) => panic!("file crash point was not reached: {error:?}"),
            }
        }
        if matches!(
            point,
            BlobFileMutationPoint::AfterExistingVerify
                | BlobFileMutationPoint::AfterCleanupUnlink
                | BlobFileMutationPoint::AfterCleanupPostUnlinkSync
        ) {
            let first = staging_with_bytes(&authority, 9, bytes);
            authority
                .commit_staging(&first, digest, bytes.len() as u64, 1024)
                .unwrap();
        }
        let staging = staging_with_bytes(&authority, 1, bytes);
        let result =
            authority.commit_staging_with(&staging, digest, bytes.len() as u64, 1024, &mut pause);
        match result {
            Ok(_) => panic!("file crash point was not reached during commit"),
            Err(error) => panic!("file crash point was not reached: {error:?}"),
        }
    }

    #[test]
    fn task_005_before_success_reply_sigkill_is_durable_and_retry_deduplicates() {
        let fixture = Fixture::new();
        let ready = fixture.base.join("reply-ready");
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("blob_storage::tests::task_005_before_success_reply_child_entrypoint")
            .arg("--exact")
            .arg("--nocapture")
            .env("MENGXIA_TASK005_PLATFORM_LIBRARY", &fixture.library)
            .env("MENGXIA_TASK005_PLATFORM_BLOB", &fixture.blob)
            .env("MENGXIA_TASK005_BEFORE_REPLY_READY", &ready)
            .spawn()
            .expect("spawn before-reply crash child");
        wait_for_ready(&mut child, &ready);
        child.kill().expect("SIGKILL before success reply");
        let _ = child.wait().expect("reap before-reply child");

        let bytes = b"MengXia TASK-005 before success reply";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let (_, _, canonical) = digest_paths(&fixture, digest);
        assert_eq!(fs::read(&canonical).unwrap(), bytes, "KILL-005-030");
        assert_eq!(
            fs::read_dir(fixture.blob.join(".staging-v1"))
                .unwrap()
                .count(),
            0
        );
        let authority = initialized_authority(&fixture);
        let retry = staging_with_bytes(&authority, 4, bytes);
        assert_eq!(
            authority
                .commit_staging(&retry, digest, bytes.len() as u64, 1024)
                .unwrap(),
            BlobCommitOutcome::ExistingVerified
        );
    }

    #[test]
    fn task_005_before_success_reply_child_entrypoint() {
        let Some(ready) = std::env::var_os("MENGXIA_TASK005_BEFORE_REPLY_READY") else {
            return;
        };
        let library = PathBuf::from(
            std::env::var_os("MENGXIA_TASK005_PLATFORM_LIBRARY").expect("crash Library"),
        );
        let blob = PathBuf::from(
            std::env::var_os("MENGXIA_TASK005_PLATFORM_BLOB").expect("crash Blob root"),
        );
        let (library, lease, request) = authority_inputs(&library, &blob);
        let authority = authorize_blob_root(library, lease, &request, [0x5a; 16]).unwrap();
        let bytes = b"MengXia TASK-005 before success reply";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let staging = staging_with_bytes(&authority, 1, bytes);
        assert_eq!(
            authority
                .commit_staging(&staging, digest, bytes.len() as u64, 1024)
                .unwrap(),
            BlobCommitOutcome::Published
        );
        let ready =
            File::create(PathBuf::from(ready)).expect("create before-reply acknowledgement");
        ready.sync_all().expect("sync before-reply acknowledgement");
        loop {
            std::thread::park();
        }
    }

    fn parse_file_crash_point(point: &str) -> BlobFileMutationPoint {
        match point {
            "AfterStagingCreate" => BlobFileMutationPoint::AfterStagingCreate,
            "AfterStagingNameSync" => BlobFileMutationPoint::AfterStagingNameSync,
            "AfterStagingFileSync" => BlobFileMutationPoint::AfterStagingFileSync,
            "AfterFirstShardMkdirOpen" => BlobFileMutationPoint::AfterFirstShardMkdirOpen,
            "AfterFirstShardSyncs" => BlobFileMutationPoint::AfterFirstShardSyncs,
            "AfterSecondShardMkdirOpen" => BlobFileMutationPoint::AfterSecondShardMkdirOpen,
            "AfterSecondShardSyncs" => BlobFileMutationPoint::AfterSecondShardSyncs,
            "AfterNoReplaceRename" => BlobFileMutationPoint::AfterNoReplaceRename,
            "AfterDestinationSync" => BlobFileMutationPoint::AfterDestinationSync,
            "AfterPostPromoteStagingSync" => BlobFileMutationPoint::AfterPostPromoteStagingSync,
            "AfterExistingVerify" => BlobFileMutationPoint::AfterExistingVerify,
            "AfterCleanupUnlink" => BlobFileMutationPoint::AfterCleanupUnlink,
            "AfterCleanupPostUnlinkSync" => BlobFileMutationPoint::AfterCleanupPostUnlinkSync,
            other => panic!("unknown file crash point {other}"),
        }
    }

    fn wait_for_ready(child: &mut std::process::Child, ready: &Path) {
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            if ready.is_file() {
                return;
            }
            if let Some(status) = child.try_wait().expect("poll TASK-005 crash child") {
                panic!("TASK-005 crash child exited before acknowledgement: {status}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = child.kill();
        let _ = child.wait();
        panic!("TASK-005 crash child timed out");
    }
}
