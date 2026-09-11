use std::fs::File;
use std::io::{Read as _, Write as _};
use std::os::fd::AsFd as _;
use std::path::{Path, PathBuf};

use rustix::fs::{AtFlags, Mode, OFlags, fsync, linkat, openat, unlinkat};
use sha2::{Digest, Sha256};

use super::{
    AuthorityError, OpenedLibraryAuthority, ValidatedAbsolutePath, enumerate_root, fstat,
    inspect_internal_file, inspect_internal_file_with_size, is_canonical_runtime_entries,
    validate_storage_directory,
};

pub const MIGRATION_INTENT_RECORD_LENGTH: usize = 512;
const INTENT_NAME: &str = ".mengxia-migration-0002.intent";
const STAGING_NAME: &str = ".library.sqlite3.pre-0002.snapshot.staging";
const SNAPSHOT_NAME: &str = ".library.sqlite3.pre-0002.snapshot";
const CANONICAL_NAME: &str = "library.sqlite3";
const JOURNAL_NAME: &str = "library.sqlite3-journal";
const COPY_BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationSourceEvidence {
    device: u64,
    inode: u64,
    length: u64,
    mtime_seconds: i64,
    mtime_nanoseconds: i64,
    ctime_seconds: i64,
    ctime_nanoseconds: i64,
    sha256: [u8; 32],
}

/// Lifetime-bound path token for the sole accepted immutable migration
/// snapshot. Callers cannot select a basename or attach URI parameters.
pub struct FixedMigrationSnapshotPath<'authority> {
    path: PathBuf,
    _authority: std::marker::PhantomData<&'authority OpenedLibraryAuthority>,
}

impl AsRef<Path> for FixedMigrationSnapshotPath<'_> {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl MigrationSourceEvidence {
    #[must_use]
    pub const fn device(self) -> u64 {
        self.device
    }

    #[must_use]
    pub const fn inode(self) -> u64 {
        self.inode
    }

    #[must_use]
    pub const fn length(self) -> u64 {
        self.length
    }

    #[must_use]
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MigrationFilesystemState {
    IntentOnly([u8; MIGRATION_INTENT_RECORD_LENGTH]),
    IntentOnlyAndRuntimeSidecars([u8; MIGRATION_INTENT_RECORD_LENGTH]),
    IntentWithStaging([u8; MIGRATION_INTENT_RECORD_LENGTH]),
    IntentWithStagingAndRuntimeSidecars([u8; MIGRATION_INTENT_RECORD_LENGTH]),
    IntentWithPublishedSnapshot([u8; MIGRATION_INTENT_RECORD_LENGTH]),
    IntentWithPublishedSnapshotAndRuntimeSidecars([u8; MIGRATION_INTENT_RECORD_LENGTH]),
    IntentWithSnapshot([u8; MIGRATION_INTENT_RECORD_LENGTH]),
    IntentWithSnapshotAndJournal([u8; MIGRATION_INTENT_RECORD_LENGTH]),
    IntentWithSnapshotAndRuntimeSidecars([u8; MIGRATION_INTENT_RECORD_LENGTH]),
}

impl MigrationFilesystemState {
    #[must_use]
    pub const fn intent_record(self) -> [u8; MIGRATION_INTENT_RECORD_LENGTH] {
        match self {
            Self::IntentOnly(record)
            | Self::IntentOnlyAndRuntimeSidecars(record)
            | Self::IntentWithStaging(record)
            | Self::IntentWithStagingAndRuntimeSidecars(record)
            | Self::IntentWithPublishedSnapshot(record)
            | Self::IntentWithPublishedSnapshotAndRuntimeSidecars(record)
            | Self::IntentWithSnapshot(record)
            | Self::IntentWithSnapshotAndJournal(record)
            | Self::IntentWithSnapshotAndRuntimeSidecars(record) => record,
        }
    }

    #[must_use]
    pub const fn has_runtime_sidecars(self) -> bool {
        matches!(
            self,
            Self::IntentOnlyAndRuntimeSidecars(_)
                | Self::IntentWithStagingAndRuntimeSidecars(_)
                | Self::IntentWithPublishedSnapshotAndRuntimeSidecars(_)
                | Self::IntentWithSnapshotAndRuntimeSidecars(_)
        )
    }
}

impl OpenedLibraryAuthority {
    pub fn migration_filesystem_state(
        &self,
    ) -> Result<Option<MigrationFilesystemState>, AuthorityError> {
        self.path.revalidate_chain()?;
        classify_state(&self.path, &enumerate_root(&self.path)?)
    }

    pub fn migration_volume_capacity(&self) -> Result<(u128, u128), AuthorityError> {
        self.path.revalidate_chain()?;
        let stat =
            rustix::fs::fstatfs(self.path.library_root_fd()).map_err(|_| AuthorityError::Io)?;
        let block_size = u128::from(stat.f_bsize);
        let available = u128::from(stat.f_bavail)
            .checked_mul(block_size)
            .ok_or(AuthorityError::UnsafeConfiguration)?;
        let total = u128::from(stat.f_blocks)
            .checked_mul(block_size)
            .ok_or(AuthorityError::UnsafeConfiguration)?;
        Ok((available, total))
    }

    pub fn inspect_migration_source(&self) -> Result<MigrationSourceEvidence, AuthorityError> {
        inspect_source(self.path_authority())
    }

    pub fn create_durable_migration_intent(
        &self,
        expected_source: MigrationSourceEvidence,
        record: &[u8; MIGRATION_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        require_closed_runtime_namespace(&self.path)?;
        if inspect_source(&self.path)? != expected_source {
            return Err(AuthorityError::ConflictingData);
        }

        let intent = openat(
            self.path.library_root_fd(),
            INTENT_NAME,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|error| match error {
            rustix::io::Errno::EXIST | rustix::io::Errno::LOOP => {
                AuthorityError::UnsafeConfiguration
            }
            _ => AuthorityError::Io,
        })?;
        let mut intent = File::from(intent);
        inspect_internal_file_with_size(intent.as_fd(), self.owner_uid, Some(0))?;
        intent.write_all(record).map_err(|_| AuthorityError::Io)?;
        fsync(intent.as_fd()).map_err(|_| AuthorityError::Io)?;
        inspect_internal_file_with_size(
            intent.as_fd(),
            self.owner_uid,
            Some(MIGRATION_INTENT_RECORD_LENGTH as u64),
        )?;
        fsync(self.path.library_root_fd()).map_err(|_| AuthorityError::Io)?;
        self.path.revalidate_chain()?;
        let entries = enumerate_root(&self.path)?;
        let state =
            classify_state(&self.path, &entries)?.ok_or(AuthorityError::UnsafeConfiguration)?;
        if state != MigrationFilesystemState::IntentOnly(*record)
            || inspect_source_file(&self.path)? != expected_source
        {
            return Err(AuthorityError::ConflictingData);
        }
        Ok(())
    }

    pub fn create_and_publish_migration_snapshot(
        &self,
        expected_source: MigrationSourceEvidence,
        expected_record: &[u8; MIGRATION_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        let entries = enumerate_root(&self.path)?;
        if classify_state(&self.path, &entries)?
            != Some(MigrationFilesystemState::IntentOnly(*expected_record))
            || inspect_source_file(&self.path)? != expected_source
        {
            return Err(AuthorityError::ConflictingData);
        }

        let source_fd = openat(
            self.path.library_root_fd(),
            CANONICAL_NAME,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let mut source = File::from(source_fd);
        validate_source_file(&source, self.owner_uid, self.path.root_device)?;
        if !stat_matches_evidence(
            &fstat(source.as_fd()).map_err(|_| AuthorityError::Io)?,
            expected_source,
        )? {
            return Err(AuthorityError::ConflictingData);
        }

        let staging_fd = openat(
            self.path.library_root_fd(),
            STAGING_NAME,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|error| match error {
            rustix::io::Errno::EXIST | rustix::io::Errno::LOOP => {
                AuthorityError::UnsafeConfiguration
            }
            _ => AuthorityError::Io,
        })?;
        let mut staging = File::from(staging_fd);
        inspect_internal_file_with_size(staging.as_fd(), self.owner_uid, Some(0))?;
        fsync(self.path.library_root_fd()).map_err(|_| AuthorityError::Io)?;

        let mut copied = 0_u64;
        let mut digest = Sha256::new();
        let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
        loop {
            let count = source.read(&mut buffer).map_err(|_| AuthorityError::Io)?;
            if count == 0 {
                break;
            }
            copied = copied
                .checked_add(u64::try_from(count).map_err(|_| AuthorityError::Io)?)
                .ok_or(AuthorityError::UnsafeConfiguration)?;
            if copied > expected_source.length {
                return Err(AuthorityError::ConflictingData);
            }
            digest.update(&buffer[..count]);
            staging
                .write_all(&buffer[..count])
                .map_err(|_| AuthorityError::Io)?;
        }
        let copied_digest: [u8; 32] = digest.finalize().into();
        if copied != expected_source.length || copied_digest != expected_source.sha256 {
            return Err(AuthorityError::ConflictingData);
        }
        fsync(staging.as_fd()).map_err(|_| AuthorityError::Io)?;
        validate_snapshot_file(
            &staging,
            self.owner_uid,
            self.path.root_device,
            expected_source.length,
            1,
        )?;
        if inspect_source_file(&self.path)? != expected_source {
            return Err(AuthorityError::ConflictingData);
        }

        linkat(
            self.path.library_root_fd(),
            STAGING_NAME,
            self.path.library_root_fd(),
            SNAPSHOT_NAME,
            AtFlags::empty(),
        )
        .map_err(|error| match error {
            rustix::io::Errno::EXIST | rustix::io::Errno::LOOP => {
                AuthorityError::UnsafeConfiguration
            }
            _ => AuthorityError::Io,
        })?;
        fsync(self.path.library_root_fd()).map_err(|_| AuthorityError::Io)?;
        let entries = enumerate_root(&self.path)?;
        if classify_state(&self.path, &entries)?
            != Some(MigrationFilesystemState::IntentWithPublishedSnapshot(
                *expected_record,
            ))
        {
            return Err(AuthorityError::ConflictingData);
        }

        unlinkat(self.path.library_root_fd(), STAGING_NAME, AtFlags::empty())
            .map_err(|_| AuthorityError::Io)?;
        fsync(self.path.library_root_fd()).map_err(|_| AuthorityError::Io)?;
        self.validate_migration_snapshot(expected_source, expected_record)
    }

    pub fn discard_owned_migration_snapshot_staging(
        &self,
        expected_record: &[u8; MIGRATION_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        let entries = enumerate_root(&self.path)?;
        if classify_state(&self.path, &entries)?
            != Some(MigrationFilesystemState::IntentWithStaging(
                *expected_record,
            ))
        {
            return Err(AuthorityError::ConflictingData);
        }
        unlinkat(self.path.library_root_fd(), STAGING_NAME, AtFlags::empty())
            .map_err(|_| AuthorityError::Io)?;
        fsync(self.path.library_root_fd()).map_err(|_| AuthorityError::Io)?;
        if self.migration_filesystem_state()?
            == Some(MigrationFilesystemState::IntentOnly(*expected_record))
        {
            Ok(())
        } else {
            Err(AuthorityError::ConflictingData)
        }
    }

    pub fn finish_published_migration_snapshot(
        &self,
        expected_record: &[u8; MIGRATION_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        let entries = enumerate_root(&self.path)?;
        if classify_state(&self.path, &entries)?
            != Some(MigrationFilesystemState::IntentWithPublishedSnapshot(
                *expected_record,
            ))
        {
            return Err(AuthorityError::ConflictingData);
        }
        unlinkat(self.path.library_root_fd(), STAGING_NAME, AtFlags::empty())
            .map_err(|_| AuthorityError::Io)?;
        fsync(self.path.library_root_fd()).map_err(|_| AuthorityError::Io)?;
        if self.migration_filesystem_state()?
            == Some(MigrationFilesystemState::IntentWithSnapshot(
                *expected_record,
            ))
        {
            Ok(())
        } else {
            Err(AuthorityError::ConflictingData)
        }
    }

    pub fn validate_migration_snapshot(
        &self,
        expected_source: MigrationSourceEvidence,
        expected_record: &[u8; MIGRATION_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        let entries = enumerate_root(&self.path)?;
        if classify_state(&self.path, &entries)?
            != Some(MigrationFilesystemState::IntentWithSnapshot(
                *expected_record,
            ))
            || inspect_source_file(&self.path)? != expected_source
        {
            return Err(AuthorityError::ConflictingData);
        }
        let snapshot_fd = openat(
            self.path.library_root_fd(),
            SNAPSHOT_NAME,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let mut snapshot = File::from(snapshot_fd);
        validate_snapshot_file(
            &snapshot,
            self.owner_uid,
            self.path.root_device,
            expected_source.length,
            1,
        )?;
        let snapshot_evidence = evidence_for_open_file(&mut snapshot)?;
        if snapshot_evidence.length == expected_source.length
            && snapshot_evidence.sha256 == expected_source.sha256
        {
            Ok(())
        } else {
            Err(AuthorityError::ConflictingData)
        }
    }

    pub fn validate_migration_snapshot_manifest(
        &self,
        expected_length: u64,
        expected_sha256: [u8; 32],
        expected_record: &[u8; MIGRATION_INTENT_RECORD_LENGTH],
    ) -> Result<(), AuthorityError> {
        self.path.revalidate_chain()?;
        let entries = enumerate_root(&self.path)?;
        if !matches!(
            classify_state(&self.path, &entries)?,
            Some(MigrationFilesystemState::IntentWithSnapshot(record)
                | MigrationFilesystemState::IntentWithSnapshotAndRuntimeSidecars(record))
                if record == *expected_record
        ) {
            return Err(AuthorityError::ConflictingData);
        }
        let snapshot_fd = openat(
            self.path.library_root_fd(),
            SNAPSHOT_NAME,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let mut snapshot = File::from(snapshot_fd);
        validate_snapshot_file(
            &snapshot,
            self.owner_uid,
            self.path.root_device,
            Some(expected_length),
            1,
        )?;
        let observed = evidence_for_open_file(&mut snapshot)?;
        if observed.length == expected_length && observed.sha256 == expected_sha256 {
            Ok(())
        } else {
            Err(AuthorityError::ConflictingData)
        }
    }

    pub fn migration_snapshot_path(
        &self,
        expected_length: u64,
        expected_sha256: [u8; 32],
        expected_record: &[u8; MIGRATION_INTENT_RECORD_LENGTH],
    ) -> Result<FixedMigrationSnapshotPath<'_>, AuthorityError> {
        self.validate_migration_snapshot_manifest(
            expected_length,
            expected_sha256,
            expected_record,
        )?;
        let parent = self
            .path
            .canonical_sqlite_path
            .parent()
            .ok_or(AuthorityError::UnsafeConfiguration)?;
        Ok(FixedMigrationSnapshotPath {
            path: parent.join(SNAPSHOT_NAME),
            _authority: std::marker::PhantomData,
        })
    }
}

pub(super) fn classify_state(
    authority: &ValidatedAbsolutePath,
    entries: &[Vec<u8>],
) -> Result<Option<MigrationFilesystemState>, AuthorityError> {
    let has = |name: &[u8]| entries.iter().any(|entry| entry.as_slice() == name);
    let allowed = entries.iter().all(|entry| {
        matches!(
            entry.as_slice(),
            b".mengxia.lock"
                | b"library.sqlite3"
                | b"storage"
                | b".mengxia-migration-0002.intent"
                | b".library.sqlite3.pre-0002.snapshot.staging"
                | b".library.sqlite3.pre-0002.snapshot"
                | b"library.sqlite3-journal"
                | b"library.sqlite3-wal"
                | b"library.sqlite3-shm"
        )
    });
    if !allowed
        || !has(b".mengxia.lock")
        || !has(b"library.sqlite3")
        || !has(INTENT_NAME.as_bytes())
    {
        return Ok(None);
    }
    authority.validate_sqlite_child(super::SqliteChild::Canonical)?;
    if has(b"storage") {
        validate_storage_directory(authority)?;
    }
    let intent = read_intent(authority, entries)?;
    let staging = has(STAGING_NAME.as_bytes());
    let snapshot = has(SNAPSHOT_NAME.as_bytes());
    let journal = has(JOURNAL_NAME.as_bytes());
    let runtime_sidecars = has(b"library.sqlite3-wal") || has(b"library.sqlite3-shm");

    // SQLite's rollback journal is a valid crash prefix only after the final
    // immutable pre-migration snapshot has been published. All other mixed
    // states fail closed before SQLite is allowed to touch the canonical file.
    if journal && (staging || !snapshot || runtime_sidecars) {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    if journal {
        let file = open_internal(authority, JOURNAL_NAME)?;
        validate_snapshot_file(&file, authority.owner_uid, authority.root_device, None, 1)?;
    }
    if runtime_sidecars {
        authority.validate_sqlite_sidecars(super::SqliteChild::Canonical)?;
    }

    let state = match (staging, snapshot, runtime_sidecars) {
        (false, false, true) => MigrationFilesystemState::IntentOnlyAndRuntimeSidecars(intent),
        (false, false, false) => MigrationFilesystemState::IntentOnly(intent),
        (true, false, runtime_sidecars) => {
            let file = open_internal(authority, STAGING_NAME)?;
            validate_snapshot_file(&file, authority.owner_uid, authority.root_device, None, 1)?;
            if runtime_sidecars {
                MigrationFilesystemState::IntentWithStagingAndRuntimeSidecars(intent)
            } else {
                MigrationFilesystemState::IntentWithStaging(intent)
            }
        }
        (true, true, runtime_sidecars) => {
            let staging = open_internal(authority, STAGING_NAME)?;
            let snapshot = open_internal(authority, SNAPSHOT_NAME)?;
            let staging_security = inspect_internal_file(staging.as_fd(), authority.owner_uid)?;
            let snapshot_security = inspect_internal_file(snapshot.as_fd(), authority.owner_uid)?;
            validate_snapshot_file(
                &staging,
                authority.owner_uid,
                authority.root_device,
                None,
                2,
            )?;
            validate_snapshot_file(
                &snapshot,
                authority.owner_uid,
                authority.root_device,
                None,
                2,
            )?;
            if !staging_security.same_object(snapshot_security) {
                return Err(AuthorityError::ConflictingData);
            }
            if runtime_sidecars {
                MigrationFilesystemState::IntentWithPublishedSnapshotAndRuntimeSidecars(intent)
            } else {
                MigrationFilesystemState::IntentWithPublishedSnapshot(intent)
            }
        }
        (false, true, _) if journal => {
            let file = open_internal(authority, SNAPSHOT_NAME)?;
            validate_snapshot_file(&file, authority.owner_uid, authority.root_device, None, 1)?;
            MigrationFilesystemState::IntentWithSnapshotAndJournal(intent)
        }
        (false, true, true) => {
            let file = open_internal(authority, SNAPSHOT_NAME)?;
            validate_snapshot_file(&file, authority.owner_uid, authority.root_device, None, 1)?;
            MigrationFilesystemState::IntentWithSnapshotAndRuntimeSidecars(intent)
        }
        (false, true, false) => {
            let file = open_internal(authority, SNAPSHOT_NAME)?;
            validate_snapshot_file(&file, authority.owner_uid, authority.root_device, None, 1)?;
            MigrationFilesystemState::IntentWithSnapshot(intent)
        }
    };
    Ok(Some(state))
}

fn require_closed_runtime_namespace(
    authority: &ValidatedAbsolutePath,
) -> Result<(), AuthorityError> {
    let entries = enumerate_root(authority)?;
    let with_storage = entries.iter().any(|entry| entry == b"storage");
    if is_canonical_runtime_entries(&entries, with_storage)
        && !entries
            .iter()
            .any(|entry| entry == b"library.sqlite3-wal" || entry == b"library.sqlite3-shm")
    {
        if with_storage {
            validate_storage_directory(authority)?;
        }
        authority.validate_sqlite_child(super::SqliteChild::Canonical)?;
        Ok(())
    } else if matches!(
        classify_state(authority, &entries)?,
        Some(state)
            if !state.has_runtime_sidecars()
                && !matches!(state, MigrationFilesystemState::IntentWithSnapshotAndJournal(_))
    ) {
        // A durable migration prefix is still a closed namespace when SQLite
        // runtime sidecars and the rollback journal are absent. This permits a
        // restart to re-prove the canonical source before resuming snapshot
        // publication.
        Ok(())
    } else {
        Err(AuthorityError::UnsafeConfiguration)
    }
}

fn inspect_source(
    authority: &ValidatedAbsolutePath,
) -> Result<MigrationSourceEvidence, AuthorityError> {
    require_closed_runtime_namespace(authority)?;
    inspect_source_file(authority)
}

fn inspect_source_file(
    authority: &ValidatedAbsolutePath,
) -> Result<MigrationSourceEvidence, AuthorityError> {
    let fd = openat(
        authority.library_root_fd(),
        CANONICAL_NAME,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let mut file = File::from(fd);
    validate_source_file(&file, authority.owner_uid, authority.root_device)?;
    let evidence = evidence_for_open_file(&mut file)?;
    authority.revalidate_chain()?;
    let reopened = open_internal(authority, CANONICAL_NAME)?;
    validate_source_file(&reopened, authority.owner_uid, authority.root_device)?;
    let reopened_stat = fstat(reopened.as_fd()).map_err(|_| AuthorityError::Io)?;
    if stat_identity(&reopened_stat)? == (evidence.device, evidence.inode, evidence.length)
        && reopened_stat.st_mtime == evidence.mtime_seconds
        && reopened_stat.st_mtime_nsec == evidence.mtime_nanoseconds
        && reopened_stat.st_ctime == evidence.ctime_seconds
        && reopened_stat.st_ctime_nsec == evidence.ctime_nanoseconds
    {
        Ok(evidence)
    } else {
        Err(AuthorityError::ConflictingData)
    }
}

fn evidence_for_open_file(file: &mut File) -> Result<MigrationSourceEvidence, AuthorityError> {
    let before = fstat(file.as_fd()).map_err(|_| AuthorityError::Io)?;
    let (device, inode, length) = stat_identity(&before)?;
    let mut digest = Sha256::new();
    let mut read_length = 0_u64;
    let mut buffer = vec![0_u8; COPY_BUFFER_BYTES];
    loop {
        let count = file.read(&mut buffer).map_err(|_| AuthorityError::Io)?;
        if count == 0 {
            break;
        }
        read_length = read_length
            .checked_add(u64::try_from(count).map_err(|_| AuthorityError::Io)?)
            .ok_or(AuthorityError::UnsafeConfiguration)?;
        if read_length > length {
            return Err(AuthorityError::ConflictingData);
        }
        digest.update(&buffer[..count]);
    }
    let after = fstat(file.as_fd()).map_err(|_| AuthorityError::Io)?;
    if read_length != length
        || stat_identity(&after)? != (device, inode, length)
        || before.st_mtime != after.st_mtime
        || before.st_mtime_nsec != after.st_mtime_nsec
        || before.st_ctime != after.st_ctime
        || before.st_ctime_nsec != after.st_ctime_nsec
    {
        return Err(AuthorityError::ConflictingData);
    }
    Ok(MigrationSourceEvidence {
        device,
        inode,
        length,
        mtime_seconds: before.st_mtime,
        mtime_nanoseconds: before.st_mtime_nsec,
        ctime_seconds: before.st_ctime,
        ctime_nanoseconds: before.st_ctime_nsec,
        sha256: digest.finalize().into(),
    })
}

fn read_intent(
    authority: &ValidatedAbsolutePath,
    expected_entries: &[Vec<u8>],
) -> Result<[u8; MIGRATION_INTENT_RECORD_LENGTH], AuthorityError> {
    let mut file = open_internal(authority, INTENT_NAME)?;
    inspect_internal_file_with_size(
        file.as_fd(),
        authority.owner_uid,
        Some(MIGRATION_INTENT_RECORD_LENGTH as u64),
    )?;
    let mut record = [0_u8; MIGRATION_INTENT_RECORD_LENGTH];
    file.read_exact(&mut record)
        .map_err(|_| AuthorityError::Io)?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing).map_err(|_| AuthorityError::Io)? != 0 {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    authority.revalidate_chain()?;
    if enumerate_root(authority)? != expected_entries {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(record)
}

fn open_internal(authority: &ValidatedAbsolutePath, name: &str) -> Result<File, AuthorityError> {
    openat(
        authority.library_root_fd(),
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|_| AuthorityError::UnsafeConfiguration)
}

fn validate_source_file(
    file: &File,
    owner_uid: u32,
    root_device: u64,
) -> Result<(), AuthorityError> {
    inspect_internal_file(file.as_fd(), owner_uid)?;
    let stat = fstat(file.as_fd()).map_err(|_| AuthorityError::Io)?;
    let (device, _, _) = stat_identity(&stat)?;
    if device == root_device && stat.st_nlink == 1 {
        Ok(())
    } else {
        Err(AuthorityError::UnsafeConfiguration)
    }
}

fn validate_snapshot_file(
    file: &File,
    owner_uid: u32,
    root_device: u64,
    expected_length: impl Into<Option<u64>>,
    expected_links: u64,
) -> Result<(), AuthorityError> {
    inspect_internal_file(file.as_fd(), owner_uid)?;
    let stat = fstat(file.as_fd()).map_err(|_| AuthorityError::Io)?;
    let (device, _, length) = stat_identity(&stat)?;
    if device == root_device
        && expected_length
            .into()
            .is_none_or(|expected| expected == length)
        && u64::from(stat.st_nlink) == expected_links
    {
        Ok(())
    } else {
        Err(AuthorityError::UnsafeConfiguration)
    }
}

fn stat_identity(stat: &rustix::fs::Stat) -> Result<(u64, u64, u64), AuthorityError> {
    Ok((
        u64::try_from(stat.st_dev).map_err(|_| AuthorityError::UnsafeConfiguration)?,
        stat.st_ino,
        u64::try_from(stat.st_size).map_err(|_| AuthorityError::UnsafeConfiguration)?,
    ))
}

fn stat_matches_evidence(
    stat: &rustix::fs::Stat,
    evidence: MigrationSourceEvidence,
) -> Result<bool, AuthorityError> {
    Ok(
        stat_identity(stat)? == (evidence.device, evidence.inode, evidence.length)
            && stat.st_mtime == evidence.mtime_seconds
            && stat.st_mtime_nsec == evidence.mtime_nanoseconds
            && stat.st_ctime == evidence.ctime_seconds
            && stat.st_ctime_nsec == evidence.ctime_nanoseconds,
    )
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write as _;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{MigrationFilesystemState, classify_state};
    use crate::OpenedLibraryAuthority;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        parent: PathBuf,
        library: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .unwrap()
                .join("target/task-009-migration-snapshot")
                .join(format!(
                    "{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&parent)
                .unwrap();
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
            let library = parent.join("Library");
            Self { parent, library }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.parent);
        }
    }

    #[test]
    fn fixed_intent_and_snapshot_publish_are_durable_and_classified() {
        let fixture = Fixture::new();
        let authority = OpenedLibraryAuthority::acquire_bootstrap(&fixture.library).unwrap();
        let mut canonical = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(fixture.library.join("library.sqlite3"))
            .unwrap();
        canonical.write_all(&vec![0x5a; 8193]).unwrap();
        canonical.sync_all().unwrap();
        drop(canonical);

        let source = authority.inspect_migration_source().unwrap();
        let record = [0xa5; 512];
        authority
            .create_durable_migration_intent(source, &record)
            .unwrap();
        authority
            .create_and_publish_migration_snapshot(source, &record)
            .unwrap();
        authority
            .validate_migration_snapshot(source, &record)
            .unwrap();
        assert_eq!(
            classify_state(
                authority.path_authority(),
                &crate::enumerate_root(authority.path_authority()).unwrap()
            )
            .unwrap(),
            Some(MigrationFilesystemState::IntentWithSnapshot(record))
        );
        assert_eq!(
            fs::read(fixture.library.join("library.sqlite3")).unwrap(),
            fs::read(fixture.library.join(".library.sqlite3.pre-0002.snapshot")).unwrap()
        );
        assert!(
            !fixture
                .library
                .join(".library.sqlite3.pre-0002.snapshot.staging")
                .exists()
        );
        drop(authority);
        let (_reopened, state) =
            OpenedLibraryAuthority::acquire_bootstrap_state(&fixture.library).unwrap();
        assert_eq!(
            state,
            crate::BootstrapFilesystemState::CanonicalWithMigration(
                MigrationFilesystemState::IntentWithSnapshot(record)
            )
        );
    }
}
