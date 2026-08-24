use std::time::{SystemTime, UNIX_EPOCH};

use mengxia_platform_fs::{OpenedLibraryAuthority, SqliteChild};
use mengxia_types::{Id, IdGenerationError, Timestamp};
use rusqlite::Connection;

use super::error::{map_authority_error, map_sqlite_error};
use super::intent::BootstrapIntent;
use super::lifecycle::OpenedLibraryOwner;
use super::migration::{
    LibraryIdentity, OpenedLibraryMetadata, bootstrap_commit_is_absent, bootstrap_schema,
    verify_bootstrap_schema, verify_bootstrap_schema_matches,
};
use super::runtime::verify_and_harden;
use super::stock_sqlite_open::{self, ConnectionAccess};
use super::wal::{BootstrapWalEvidence, inspect_bootstrap_wal};
use super::{StoreConfig, StoreError};
use crate::path_authority::{
    OpenedBootstrapState, acquire_bootstrap_authority, acquire_bootstrap_state,
    authorize_bootstrap_parent,
};

trait BootstrapClock {
    fn now(&mut self) -> Result<(i64, u32), ()>;
}

trait LibraryIdSource {
    fn next(&mut self) -> Result<Id<LibraryIdentity>, IdGenerationError>;
}

struct SystemBootstrapClock;

impl BootstrapClock for SystemBootstrapClock {
    fn now(&mut self) -> Result<(i64, u32), ()> {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ())?;
        let seconds = i64::try_from(elapsed.as_secs()).map_err(|_| ())?;
        Ok((seconds, elapsed.subsec_nanos()))
    }
}

struct SystemLibraryIdSource;

impl LibraryIdSource for SystemLibraryIdSource {
    fn next(&mut self) -> Result<Id<LibraryIdentity>, IdGenerationError> {
        Id::try_new()
    }
}

/// Creates and opens a fresh Library after all fallible clock and identity
/// sampling has completed ahead of root mutation.
pub(crate) fn create_fresh_library(config: &StoreConfig) -> Result<OpenedLibraryOwner, StoreError> {
    create_fresh_library_with_sources(
        config,
        &mut SystemBootstrapClock,
        &mut SystemLibraryIdSource,
    )
}

fn create_fresh_library_with_sources<Clock, Ids>(
    config: &StoreConfig,
    clock: &mut Clock,
    ids: &mut Ids,
) -> Result<OpenedLibraryOwner, StoreError>
where
    Clock: BootstrapClock,
    Ids: LibraryIdSource,
{
    authorize_bootstrap_parent(config)?;
    let (seconds, nanos) = clock
        .now()
        .map_err(|()| StoreError::IdGenerationUnavailable)?;
    let created_at = Timestamp::from_unix_seconds_nanos(seconds, nanos)
        .map_err(|_| StoreError::IdGenerationUnavailable)?;
    let library_id = ids
        .next()
        .map_err(|_| StoreError::IdGenerationUnavailable)?;

    let authority = acquire_bootstrap_authority(config)?;
    let intent = BootstrapIntent::create_durable(&authority, library_id, created_at)?;
    bootstrap_staging_database(config, &authority, intent)?;
    let metadata = publish_bootstrapped_staging(config, &authority, intent)?;
    OpenedLibraryOwner::start(config, authority, metadata)
}

/// Completes the SQLite portion of one already-authorized bootstrap attempt.
/// Publishing and restart cleanup remain separate recovery transitions.
pub(crate) fn bootstrap_staging_database(
    config: &StoreConfig,
    authority: &OpenedLibraryAuthority,
    intent: BootstrapIntent,
) -> Result<(), StoreError> {
    verify_config_authority(config, authority)?;
    intent.verify_authority(authority)?;
    intent.create_staging(authority)?;

    let mut connection = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::BootstrapStaging,
        ConnectionAccess::ReadWrite,
    )?;
    verify_and_harden(&connection, config.busy_timeout())?;
    bootstrap_schema(
        &mut connection,
        intent.library_id(),
        authority.owner_uid(),
        intent.created_at(),
    )?;
    checkpoint_truncate(&connection)?;
    close(connection)?;

    validate_and_close_staging(config, authority, intent)
}

/// Revalidates and publishes a complete staging database, then proves the final
/// canonical file through a read-only reopen. Restart recovery remains separate.
pub(crate) fn publish_bootstrapped_staging(
    config: &StoreConfig,
    authority: &OpenedLibraryAuthority,
    intent: BootstrapIntent,
) -> Result<OpenedLibraryMetadata, StoreError> {
    verify_config_authority(config, authority)?;
    intent.verify_authority(authority)?;
    validate_and_close_staging(config, authority, intent)?;

    authority
        .publish_verified_staging(&intent.encode())
        .map_err(map_authority_error)?;

    let canonical = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::Canonical,
        ConnectionAccess::ReadOnly,
    )?;
    verify_and_harden(&canonical, config.busy_timeout())?;
    let metadata = verify_bootstrap_schema_matches(
        &canonical,
        intent.library_id(),
        authority.owner_uid(),
        intent.created_at(),
    )?;
    close(canonical)?;

    // As with the staging read-only proof, stock WAL mode may leave recognized
    // sidecars. A bounded checkpoint/close removes them before returning a
    // closed canonical state to the future opened-Library owner.
    let cleanup = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::Canonical,
        ConnectionAccess::ReadWrite,
    )?;
    verify_and_harden(&cleanup, config.busy_timeout())?;
    checkpoint_truncate(&cleanup)?;
    close(cleanup)?;
    authority
        .sync_closed_canonical_database()
        .map_err(map_authority_error)?;
    Ok(metadata)
}

pub(crate) enum RecoveryOutcome {
    Opened {
        authority: OpenedLibraryAuthority,
        metadata: OpenedLibraryMetadata,
    },
    NeedsFreshBootstrap(OpenedLibraryAuthority),
}

/// Reacquires the durable lock and completes one identity-bearing closed
/// bootstrap state. A proven empty/rolled-back staging transaction is removed
/// under its valid intent and returned as a retained lock-only authority.
pub(crate) fn recover_closed_library(config: &StoreConfig) -> Result<RecoveryOutcome, StoreError> {
    match acquire_bootstrap_state(config)? {
        OpenedBootstrapState::ValidIntent { authority, intent } => {
            bootstrap_staging_database(config, &authority, intent)?;
            let metadata = publish_bootstrapped_staging(config, &authority, intent)?;
            Ok(RecoveryOutcome::Opened {
                authority,
                metadata,
            })
        }
        OpenedBootstrapState::ValidIntentWithStaging { authority, intent } => {
            recover_staging(config, authority, intent)
        }
        OpenedBootstrapState::ValidIntentWithPublishedStaging { authority, intent } => {
            validate_and_close_canonical(config, &authority, Some(intent))?;
            authority
                .finish_published_staging(&intent.encode())
                .map_err(map_authority_error)?;
            let metadata = validate_and_close_canonical(config, &authority, Some(intent))?;
            authority
                .sync_closed_canonical_database()
                .map_err(map_authority_error)?;
            Ok(RecoveryOutcome::Opened {
                authority,
                metadata,
            })
        }
        OpenedBootstrapState::ValidIntentWithCanonical { authority, intent } => {
            validate_and_close_canonical(config, &authority, Some(intent))?;
            authority
                .finish_canonical_intent(&intent.encode())
                .map_err(map_authority_error)?;
            let metadata = validate_and_close_canonical(config, &authority, Some(intent))?;
            authority
                .sync_closed_canonical_database()
                .map_err(map_authority_error)?;
            Ok(RecoveryOutcome::Opened {
                authority,
                metadata,
            })
        }
        OpenedBootstrapState::CanonicalOnly(authority) => {
            let metadata = validate_and_close_canonical(config, &authority, None)?;
            authority
                .sync_closed_canonical_database()
                .map_err(map_authority_error)?;
            Ok(RecoveryOutcome::Opened {
                authority,
                metadata,
            })
        }
        OpenedBootstrapState::LockOnly(authority) => {
            Ok(RecoveryOutcome::NeedsFreshBootstrap(authority))
        }
    }
}

fn recover_staging(
    config: &StoreConfig,
    authority: OpenedLibraryAuthority,
    intent: BootstrapIntent,
) -> Result<RecoveryOutcome, StoreError> {
    verify_config_authority(config, &authority)?;
    intent.verify_authority(&authority)?;

    let wal_evidence = inspect_bootstrap_wal(&authority)?;
    if wal_evidence == BootstrapWalEvidence::CorruptBeforeCommit {
        return Err(StoreError::Corruption);
    }

    let connection = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::BootstrapStaging,
        ConnectionAccess::ReadWrite,
    )?;
    verify_and_harden(&connection, config.busy_timeout())?;
    match verify_bootstrap_schema_matches(
        &connection,
        intent.library_id(),
        authority.owner_uid(),
        intent.created_at(),
    ) {
        Ok(_) => {
            checkpoint_truncate(&connection)?;
            close(connection)?;
            authority
                .sync_closed_staging_database(&intent.encode())
                .map_err(map_authority_error)?;
            let metadata = publish_bootstrapped_staging(config, &authority, intent)?;
            Ok(RecoveryOutcome::Opened {
                authority,
                metadata,
            })
        }
        Err(StoreError::Corruption)
            if wal_evidence == BootstrapWalEvidence::AbsentOrUncommitted
                && bootstrap_commit_is_absent(&connection)? =>
        {
            checkpoint_truncate(&connection)?;
            close(connection)?;
            authority
                .cleanup_authorized_incomplete_staging(&intent.encode())
                .map_err(map_authority_error)?;
            Ok(RecoveryOutcome::NeedsFreshBootstrap(authority))
        }
        Err(StoreError::Corruption) => {
            close(connection)?;
            Err(StoreError::Corruption)
        }
        Err(error) => {
            close(connection)?;
            Err(error)
        }
    }
}

fn validate_and_close_canonical(
    config: &StoreConfig,
    authority: &OpenedLibraryAuthority,
    expected_intent: Option<BootstrapIntent>,
) -> Result<OpenedLibraryMetadata, StoreError> {
    verify_config_authority(config, authority)?;
    let canonical = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::Canonical,
        ConnectionAccess::ReadOnly,
    )?;
    verify_and_harden(&canonical, config.busy_timeout())?;
    let metadata = if let Some(intent) = expected_intent {
        intent.verify_authority(authority)?;
        verify_bootstrap_schema_matches(
            &canonical,
            intent.library_id(),
            authority.owner_uid(),
            intent.created_at(),
        )?
    } else {
        let metadata = verify_bootstrap_schema(&canonical)?;
        if metadata.owner_uid != authority.owner_uid() {
            return Err(StoreError::Corruption);
        }
        metadata
    };
    close(canonical)?;

    let cleanup = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::Canonical,
        ConnectionAccess::ReadWrite,
    )?;
    verify_and_harden(&cleanup, config.busy_timeout())?;
    checkpoint_truncate(&cleanup)?;
    close(cleanup)?;
    Ok(metadata)
}

pub(crate) fn finalize_opened_canonical(
    config: &StoreConfig,
    authority: &OpenedLibraryAuthority,
    metadata: OpenedLibraryMetadata,
) -> Result<(), StoreError> {
    verify_config_authority(config, authority)?;
    let connection = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::Canonical,
        ConnectionAccess::ReadWrite,
    )?;
    verify_and_harden(&connection, config.busy_timeout())?;
    verify_bootstrap_schema_matches(
        &connection,
        metadata.library_id,
        metadata.owner_uid,
        metadata.created_at,
    )?;
    checkpoint_truncate(&connection)?;
    close(connection)?;
    authority
        .sync_closed_canonical_database()
        .map_err(map_authority_error)
}

fn validate_and_close_staging(
    config: &StoreConfig,
    authority: &OpenedLibraryAuthority,
    intent: BootstrapIntent,
) -> Result<(), StoreError> {
    let record = intent.encode();

    authority
        .sync_closed_staging_database(&record)
        .map_err(map_authority_error)?;

    let reopened = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::BootstrapStaging,
        ConnectionAccess::ReadOnly,
    )?;
    verify_and_harden(&reopened, config.busy_timeout())?;
    verify_bootstrap_schema_matches(
        &reopened,
        intent.library_id(),
        authority.owner_uid(),
        intent.created_at(),
    )?;
    close(reopened)?;

    // A stock read-only WAL connection can legitimately create SHM/WAL names
    // that it cannot remove. Reopen the same fixed staging path read-write only
    // to checkpoint and close those validated sidecars before publication.
    let cleanup = stock_sqlite_open::open(
        authority.path_authority(),
        SqliteChild::BootstrapStaging,
        ConnectionAccess::ReadWrite,
    )?;
    verify_and_harden(&cleanup, config.busy_timeout())?;
    checkpoint_truncate(&cleanup)?;
    close(cleanup)?;
    authority
        .sync_closed_staging_database(&record)
        .map_err(map_authority_error)
}

fn verify_config_authority(
    config: &StoreConfig,
    authority: &OpenedLibraryAuthority,
) -> Result<(), StoreError> {
    if authority.authorizes_library_root(config.library_root().as_path()) {
        Ok(())
    } else {
        Err(StoreError::Configuration)
    }
}

fn checkpoint_truncate(connection: &Connection) -> Result<(), StoreError> {
    let (busy, log_frames, checkpointed_frames): (i64, i64, i64) = connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .map_err(map_sqlite_error)?;
    if busy != 0 {
        Err(StoreError::Busy)
    } else if log_frames == 0 && checkpointed_frames == 0 {
        Ok(())
    } else {
        Err(StoreError::Internal)
    }
}

fn close(connection: Connection) -> Result<(), StoreError> {
    connection
        .close()
        .map_err(|(_connection, error)| map_sqlite_error(error))
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File, OpenOptions};
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::thread;
    use std::time::Duration;

    use mengxia_types::{Id, IdGenerationError, Timestamp};

    use super::close;
    use super::{
        BootstrapClock, LibraryIdSource, RecoveryOutcome, bootstrap_staging_database,
        create_fresh_library_with_sources, publish_bootstrapped_staging, recover_closed_library,
    };
    use crate::intent::BootstrapIntent;
    use crate::migration::{
        LibraryIdentity, bootstrap_schema_with_crash_boundaries, verify_bootstrap_schema_matches,
    };
    use crate::path_authority::acquire_bootstrap_authority;
    use crate::stock_sqlite_open::{self, ConnectionAccess};
    use crate::{ConfigSource, ResolvedStoreConfig, StoreConfig, StoreError};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        parent: PathBuf,
        library: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|path| path.parent())
                .expect("crate is inside workspace")
                .to_path_buf();
            let parent = repository.join(format!(
                "target/task-004-staging-bootstrap-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&parent)
                .expect("create secure fixture parent");
            fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
                .expect("secure fixture parent");
            let library = parent.join("Library");
            Self { parent, library }
        }

        fn config(&self) -> StoreConfig {
            ResolvedStoreConfig::from_selected(
                Some(self.library.clone()),
                ConfigSource::Cli,
                256,
                ConfigSource::CompiledDefault,
                4,
                ConfigSource::CompiledDefault,
                37,
                ConfigSource::CompiledDefault,
            )
            .validate()
            .expect("valid store config")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.parent);
        }
    }

    struct CountingClock<'a> {
        calls: &'a AtomicUsize,
        result: Result<(i64, u32), ()>,
    }

    impl BootstrapClock for CountingClock<'_> {
        fn now(&mut self) -> Result<(i64, u32), ()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.result
        }
    }

    struct CountingIds<'a> {
        calls: &'a AtomicUsize,
        result: Result<Id<LibraryIdentity>, IdGenerationError>,
    }

    impl LibraryIdSource for CountingIds<'_> {
        fn next(&mut self) -> Result<Id<LibraryIdentity>, IdGenerationError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.result
        }
    }

    #[test]
    fn fresh_bootstrap_samples_clock_and_identity_once_before_opening_the_owner() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let clock_calls = AtomicUsize::new(0);
        let id_calls = AtomicUsize::new(0);
        let mut clock = CountingClock {
            calls: &clock_calls,
            result: Ok((1_700_000_000, 123_456_789)),
        };
        let mut ids = CountingIds {
            calls: &id_calls,
            result: Ok(fixed_library_id()),
        };

        let owner = create_fresh_library_with_sources(&config, &mut clock, &mut ids)
            .expect("fresh bootstrap opens the retained owner");
        assert_eq!(clock_calls.load(Ordering::SeqCst), 1);
        assert_eq!(id_calls.load(Ordering::SeqCst), 1);
        let writer = owner
            .handle()
            .verify_on_writer()
            .expect("fresh writer verification is admitted");
        assert_eq!(writer.blocking_recv(), Ok(Ok(())));
        owner.shutdown().expect("joined fresh owner shutdown");

        let RecoveryOutcome::Opened {
            authority,
            metadata,
        } = recover_closed_library(&config).expect("restart reuses persisted identity")
        else {
            panic!("published fresh Library must reopen");
        };
        assert_eq!(metadata.library_id, fixed_library_id());
        assert_eq!(metadata.created_at, fixed_timestamp());
        drop(authority);
        assert_eq!(clock_calls.load(Ordering::SeqCst), 1);
        assert_eq!(id_calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn every_clock_or_timestamp_failure_precedes_identity_and_root_mutation() {
        for (case, clock_result) in [
            ("clock-source", Err(())),
            ("timestamp-year", Ok((i64::MAX, 0))),
            ("timestamp-nanos", Ok((1_700_000_000, 1_000_000_000))),
        ] {
            for existing_empty_root in [false, true] {
                let fixture = Fixture::new();
                if existing_empty_root {
                    fs::DirBuilder::new()
                        .mode(0o700)
                        .create(&fixture.library)
                        .expect("create empty Library fixture");
                }
                let config = fixture.config();
                let clock_calls = AtomicUsize::new(0);
                let id_calls = AtomicUsize::new(0);
                let mut clock = CountingClock {
                    calls: &clock_calls,
                    result: clock_result,
                };
                let mut ids = CountingIds {
                    calls: &id_calls,
                    result: Ok(fixed_library_id()),
                };

                assert!(matches!(
                    create_fresh_library_with_sources(&config, &mut clock, &mut ids),
                    Err(StoreError::IdGenerationUnavailable)
                ));
                assert_eq!(clock_calls.load(Ordering::SeqCst), 1, "{case}");
                assert_eq!(id_calls.load(Ordering::SeqCst), 0, "{case}");
                assert_eq!(fixture.library.exists(), existing_empty_root, "{case}");
                if existing_empty_root {
                    assert_eq!(
                        fs::read_dir(&fixture.library)
                            .expect("read unchanged empty root")
                            .count(),
                        0,
                        "{case}"
                    );
                }
            }
        }
    }

    #[test]
    fn unsafe_parent_fails_before_clock_identity_or_root_mutation() {
        let fixture = Fixture::new();
        fs::set_permissions(&fixture.parent, fs::Permissions::from_mode(0o777))
            .expect("make bootstrap parent unsafe");
        let config = fixture.config();
        let clock_calls = AtomicUsize::new(0);
        let id_calls = AtomicUsize::new(0);
        let mut clock = CountingClock {
            calls: &clock_calls,
            result: Ok((1_700_000_000, 123_456_789)),
        };
        let mut ids = CountingIds {
            calls: &id_calls,
            result: Ok(fixed_library_id()),
        };

        assert!(matches!(
            create_fresh_library_with_sources(&config, &mut clock, &mut ids),
            Err(StoreError::Configuration)
        ));
        assert_eq!(clock_calls.load(Ordering::SeqCst), 0);
        assert_eq!(id_calls.load(Ordering::SeqCst), 0);
        assert!(!fixture.library.exists());
    }

    #[test]
    fn every_uuid_generation_failure_precedes_root_mutation() {
        for error in [
            IdGenerationError::ClockBeforeUnixEpoch,
            IdGenerationError::TimestampOutOfRange,
            IdGenerationError::EntropyUnavailable,
        ] {
            let fixture = Fixture::new();
            let config = fixture.config();
            let clock_calls = AtomicUsize::new(0);
            let id_calls = AtomicUsize::new(0);
            let mut clock = CountingClock {
                calls: &clock_calls,
                result: Ok((1_700_000_000, 123_456_789)),
            };
            let mut ids = CountingIds {
                calls: &id_calls,
                result: Err(error),
            };

            assert!(matches!(
                create_fresh_library_with_sources(&config, &mut clock, &mut ids),
                Err(StoreError::IdGenerationUnavailable)
            ));
            assert_eq!(clock_calls.load(Ordering::SeqCst), 1);
            assert_eq!(id_calls.load(Ordering::SeqCst), 1);
            assert!(!fixture.library.exists());
        }
    }

    #[test]
    fn valid_intent_bootstraps_only_the_closed_verified_staging_database() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");

        bootstrap_staging_database(&config, &authority, intent)
            .expect("verified staging bootstrap");

        assert!(fixture.library.join(".library.sqlite3.bootstrap").is_file());
        assert!(
            !fixture
                .library
                .join(".library.sqlite3.bootstrap-wal")
                .exists()
        );
        assert!(
            !fixture
                .library
                .join(".library.sqlite3.bootstrap-shm")
                .exists()
        );
        assert!(!fixture.library.join("library.sqlite3").exists());
        assert!(fixture.library.join(".mengxia.bootstrap-intent").is_file());
        assert_owner_only(&fixture.library.join(".library.sqlite3.bootstrap"));

        let reopened = stock_sqlite_open::open(
            authority.path_authority(),
            mengxia_platform_fs::SqliteChild::BootstrapStaging,
            ConnectionAccess::ReadOnly,
        )
        .expect("observable read-only staging reopen");
        verify_bootstrap_schema_matches(
            &reopened,
            fixed_library_id(),
            authority.owner_uid(),
            fixed_timestamp(),
        )
        .expect("observable persisted bootstrap metadata");
        close(reopened).expect("close observable staging reopen");
    }

    #[test]
    fn mismatched_config_authority_fails_before_staging_mutation() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");
        let other_config = ResolvedStoreConfig::from_selected(
            Some(fixture.parent.join("OtherLibrary")),
            ConfigSource::Cli,
            256,
            ConfigSource::CompiledDefault,
            4,
            ConfigSource::CompiledDefault,
            37,
            ConfigSource::CompiledDefault,
        )
        .validate()
        .expect("other lexically valid config");

        assert_eq!(
            bootstrap_staging_database(&other_config, &authority, intent),
            Err(crate::StoreError::Configuration)
        );
        assert!(!fixture.library.join(".library.sqlite3.bootstrap").exists());
        assert!(fixture.library.join(".mengxia.bootstrap-intent").is_file());
        assert!(!fixture.library.join("library.sqlite3").exists());
    }

    #[test]
    fn verified_staging_publishes_one_canonical_inode_and_removes_recovery_names() {
        use std::os::unix::fs::MetadataExt;

        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");
        bootstrap_staging_database(&config, &authority, intent)
            .expect("verified staging bootstrap");
        let staging = fs::metadata(fixture.library.join(".library.sqlite3.bootstrap"))
            .expect("staging metadata before publish");

        publish_bootstrapped_staging(&config, &authority, intent)
            .expect("verified canonical publish");

        let canonical = fs::metadata(fixture.library.join("library.sqlite3"))
            .expect("canonical metadata after publish");
        assert_eq!(staging.dev(), canonical.dev());
        assert_eq!(staging.ino(), canonical.ino());
        assert!(!fixture.library.join(".library.sqlite3.bootstrap").exists());
        assert!(!fixture.library.join(".mengxia.bootstrap-intent").exists());
        assert!(!fixture.library.join("library.sqlite3-wal").exists());
        assert!(!fixture.library.join("library.sqlite3-shm").exists());
        assert!(fixture.library.join(".mengxia.lock").is_file());
        assert_owner_only(&fixture.library.join("library.sqlite3"));

        let reopened = stock_sqlite_open::open(
            authority.path_authority(),
            mengxia_platform_fs::SqliteChild::Canonical,
            ConnectionAccess::ReadOnly,
        )
        .expect("observable canonical reopen");
        verify_bootstrap_schema_matches(
            &reopened,
            fixed_library_id(),
            authority.owner_uid(),
            fixed_timestamp(),
        )
        .expect("observable canonical metadata");
        close(reopened).expect("close observable canonical reopen");
    }

    #[test]
    fn tampered_staging_fails_before_canonical_publish() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");
        bootstrap_staging_database(&config, &authority, intent)
            .expect("verified staging bootstrap");

        let tamper = rusqlite::Connection::open(fixture.library.join(".library.sqlite3.bootstrap"))
            .expect("open staging tamper fixture");
        tamper
            .execute("UPDATE schema_migrations SET sha256 = zeroblob(32)", [])
            .expect("tamper migration digest");
        drop(tamper);

        assert!(matches!(
            publish_bootstrapped_staging(&config, &authority, intent),
            Err(crate::StoreError::Corruption)
        ));
        assert!(!fixture.library.join("library.sqlite3").exists());
        assert!(fixture.library.join(".library.sqlite3.bootstrap").exists());
        assert!(fixture.library.join(".mengxia.bootstrap-intent").exists());
    }

    #[test]
    fn every_proven_closed_restart_state_recovers_to_canonical_only() {
        for state in [
            RestartFixture::IntentOnly,
            RestartFixture::Staging,
            RestartFixture::PublishedStaging,
            RestartFixture::CanonicalIntent,
            RestartFixture::CanonicalOnly,
        ] {
            let fixture = Fixture::new();
            let config = fixture.config();
            let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
            let intent =
                BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                    .expect("durable intent");
            if state != RestartFixture::IntentOnly {
                bootstrap_staging_database(&config, &authority, intent)
                    .expect("verified staging bootstrap");
            }
            match state {
                RestartFixture::IntentOnly | RestartFixture::Staging => {}
                RestartFixture::PublishedStaging => {
                    fs::hard_link(
                        fixture.library.join(".library.sqlite3.bootstrap"),
                        fixture.library.join("library.sqlite3"),
                    )
                    .expect("simulate returned publish link");
                }
                RestartFixture::CanonicalIntent => {
                    fs::hard_link(
                        fixture.library.join(".library.sqlite3.bootstrap"),
                        fixture.library.join("library.sqlite3"),
                    )
                    .expect("simulate returned publish link");
                    fs::remove_file(fixture.library.join(".library.sqlite3.bootstrap"))
                        .expect("simulate returned staging unlink");
                }
                RestartFixture::CanonicalOnly => {
                    publish_bootstrapped_staging(&config, &authority, intent)
                        .expect("complete initial publish");
                }
            }
            drop(authority);

            let RecoveryOutcome::Opened {
                authority: recovered,
                metadata,
            } = recover_closed_library(&config).expect("recover closed restart state")
            else {
                panic!("complete state unexpectedly requested a fresh bootstrap")
            };
            assert_eq!(metadata.library_id, fixed_library_id(), "{state:?}");
            assert_eq!(metadata.owner_uid, recovered.owner_uid(), "{state:?}");
            assert_eq!(metadata.created_at, fixed_timestamp(), "{state:?}");
            assert_eq!(
                root_names(&fixture.library),
                [".mengxia.lock", "library.sqlite3"],
                "{state:?}"
            );
            drop(recovered);
        }
    }

    #[test]
    fn lock_only_restart_returns_retained_fresh_bootstrap_authority() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        drop(authority);

        let RecoveryOutcome::NeedsFreshBootstrap(recovered) =
            recover_closed_library(&config).expect("recover lock-only state")
        else {
            panic!("lock-only state unexpectedly opened a Library")
        };
        assert_eq!(root_names(&fixture.library), [".mengxia.lock"]);
        drop(recovered);
    }

    #[test]
    fn valid_intent_authorizes_empty_staging_cleanup_to_lock_only() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");
        intent
            .create_staging(&authority)
            .expect("durable empty staging");
        drop(authority);

        let RecoveryOutcome::NeedsFreshBootstrap(recovered) =
            recover_closed_library(&config).expect("clean incomplete staging")
        else {
            panic!("empty staging unexpectedly opened a Library")
        };
        assert_eq!(root_names(&fixture.library), [".mengxia.lock"]);
        assert!(!fixture.library.join(".mengxia.bootstrap-intent").exists());
        assert!(!fixture.library.join(".library.sqlite3.bootstrap").exists());
        drop(recovered);
    }

    #[test]
    fn killed_wal_writer_recovers_commit_or_cleans_rolled_back_staging() {
        for (case, child_mode, sidecar_mutation, expect_opened) in [
            (
                "CommittedWalMissingShm",
                "commit",
                WalMutation::RemoveShm,
                true,
            ),
            (
                "CommittedWalMalformedShm",
                "commit",
                WalMutation::MalformedShm,
                true,
            ),
            (
                "CommittedWalTrailingBytes",
                "commit",
                WalMutation::TrailingWalBytes,
                true,
            ),
            (
                "CommittedThenUncommittedWal",
                "commit-then-uncommitted",
                WalMutation::RemoveShm,
                true,
            ),
            (
                "RolledBackWal",
                "uncommitted",
                WalMutation::RemoveShm,
                false,
            ),
        ] {
            let fixture = Fixture::new();
            let config = fixture.config();
            let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
            let intent =
                BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                    .expect("durable intent");
            intent
                .create_staging(&authority)
                .expect("durable empty staging");
            drop(authority);

            let staging = fixture.library.join(".library.sqlite3.bootstrap");
            let ready = fixture.parent.join(format!("{case}.ready"));
            let mut child = Command::new(std::env::current_exe().expect("current test executable"))
                .args([
                    "--exact",
                    "bootstrap::tests::wal_writer_child_process_entrypoint",
                    "--nocapture",
                ])
                .env("MENGXIA_TASK004_WAL_CHILD", child_mode)
                .env("MENGXIA_TASK004_WAL_PATH", &staging)
                .env("MENGXIA_TASK004_WAL_READY", &ready)
                .spawn()
                .expect("start WAL writer child");
            for _ in 0..500 {
                if ready.is_file() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            assert!(ready.is_file(), "{case} child did not reach crash point");
            child.kill().expect("kill WAL writer at crash point");
            let status = child.wait().expect("reap killed WAL writer");
            assert!(!status.success(), "{case} child was not killed");
            let wal = fixture.library.join(".library.sqlite3.bootstrap-wal");
            let shm = fixture.library.join(".library.sqlite3.bootstrap-shm");
            match sidecar_mutation {
                WalMutation::RemoveShm => {
                    if shm.exists() {
                        fs::remove_file(&shm).expect("simulate recoverable missing SHM");
                    }
                }
                WalMutation::MalformedShm => {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(&shm)
                        .expect("open retained SHM for malformed-byte fixture");
                    file.write_all(&[0xa5; 32_768])
                        .expect("write malformed SHM bytes");
                    file.sync_all().expect("sync malformed SHM fixture");
                }
                WalMutation::TrailingWalBytes => {
                    let mut file = OpenOptions::new()
                        .append(true)
                        .open(&wal)
                        .expect("open retained WAL for trailing-byte fixture");
                    file.write_all(&[0x5a; 17])
                        .expect("append incomplete WAL frame bytes");
                    file.sync_all().expect("sync trailing WAL bytes");
                    if shm.exists() {
                        fs::remove_file(&shm).expect("remove SHM before WAL rescan");
                    }
                }
            }
            assert!(wal.is_file(), "{case} must retain WAL evidence");

            let recovered = recover_closed_library(&config).expect("recover killed WAL writer");
            match (expect_opened, recovered) {
                (
                    true,
                    RecoveryOutcome::Opened {
                        authority,
                        metadata,
                    },
                ) => {
                    assert_eq!(metadata.library_id, fixed_library_id());
                    assert_eq!(metadata.owner_uid, authority.owner_uid());
                    assert_eq!(metadata.created_at, fixed_timestamp());
                    assert_eq!(
                        root_names(&fixture.library),
                        [".mengxia.lock", "library.sqlite3"]
                    );
                }
                (false, RecoveryOutcome::NeedsFreshBootstrap(authority)) => {
                    assert_eq!(root_names(&fixture.library), [".mengxia.lock"]);
                    drop(authority);
                }
                _ => panic!("{case} produced the wrong recovery outcome"),
            }
        }
    }

    #[test]
    fn killed_writer_required_commit_wal_damage_fails_closed() {
        for (case, mutation) in [
            ("RequiredFramePayload", RequiredWalMutation::Payload),
            ("RequiredFrameSalt", RequiredWalMutation::Salt),
            ("RequiredFrameChecksum", RequiredWalMutation::Checksum),
        ] {
            let fixture = Fixture::new();
            let config = fixture.config();
            let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
            let intent =
                BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                    .expect("durable intent");
            intent
                .create_staging(&authority)
                .expect("durable empty staging");
            drop(authority);

            let staging = fixture.library.join(".library.sqlite3.bootstrap");
            let ready = fixture.parent.join(format!("{case}.ready"));
            let mut child = Command::new(std::env::current_exe().expect("current test executable"))
                .args([
                    "--exact",
                    "bootstrap::tests::wal_writer_child_process_entrypoint",
                    "--nocapture",
                ])
                .env("MENGXIA_TASK004_WAL_CHILD", "commit")
                .env("MENGXIA_TASK004_WAL_PATH", &staging)
                .env("MENGXIA_TASK004_WAL_READY", &ready)
                .spawn()
                .expect("start WAL writer child");
            for _ in 0..500 {
                if ready.is_file() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            assert!(
                ready.is_file(),
                "{case} child did not reach committed crash point"
            );
            child
                .kill()
                .expect("kill WAL writer at committed crash point");
            let status = child.wait().expect("reap killed WAL writer");
            assert!(!status.success(), "{case} child was not killed");

            let wal = fixture.library.join(".library.sqlite3.bootstrap-wal");
            let shm = fixture.library.join(".library.sqlite3.bootstrap-shm");
            let mut wal_bytes = fs::read(&wal).expect("read committed WAL fixture");
            let mutation_offset = match mutation {
                RequiredWalMutation::Payload => 56,
                RequiredWalMutation::Salt => 40,
                RequiredWalMutation::Checksum => 48,
            };
            assert!(
                wal_bytes.len() > mutation_offset,
                "{case} fixture must contain a complete WAL frame"
            );
            wal_bytes[mutation_offset] ^= 0x80;
            let mut wal_file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&wal)
                .expect("open committed WAL for corruption fixture");
            wal_file
                .write_all(&wal_bytes)
                .expect("corrupt required committed WAL content");
            wal_file.sync_all().expect("sync corrupted WAL fixture");
            if shm.exists() {
                fs::remove_file(&shm).expect("remove SHM before corrupted WAL rescan");
            }

            assert!(
                matches!(
                    recover_closed_library(&config),
                    Err(crate::StoreError::Corruption)
                ),
                "{case} must fail closed as corruption"
            );
            assert!(
                fixture.library.join(".mengxia.bootstrap-intent").is_file(),
                "{case} must preserve intent evidence"
            );
            assert!(staging.is_file(), "{case} must preserve staging evidence");
            assert!(wal.is_file(), "{case} must preserve WAL evidence");
        }
    }

    #[test]
    fn exact_same_os_sigkill_recovery_matrix() {
        for point in 1_u8..=23 {
            let prefixes: &[usize] = if point == 7 {
                &[0, 37, 74, 111, 148, 185, 222]
            } else {
                &[0]
            };
            for &prefix in prefixes {
                let case = if point == 7 {
                    format!("point-{point}-prefix-{prefix}")
                } else {
                    format!("point-{point}")
                };
                let fixture = Fixture::new();
                let ready = fixture.parent.join(format!("{case}.ready"));
                let record = fixture.parent.join(format!("{case}.intent"));
                let result = fixture.parent.join(format!("{case}.result"));
                let mut producer = crash_matrix_command()
                    .env("MENGXIA_TASK004_CRASH_MODE", "producer")
                    .env("MENGXIA_TASK004_CRASH_POINT", point.to_string())
                    .env("MENGXIA_TASK004_CRASH_PREFIX", prefix.to_string())
                    .env("MENGXIA_TASK004_CRASH_LIBRARY", &fixture.library)
                    .env("MENGXIA_TASK004_CRASH_READY", &ready)
                    .env("MENGXIA_TASK004_CRASH_RECORD", &record)
                    .spawn()
                    .expect("start crash-point producer");
                wait_for_file(&ready, &case);
                producer.kill().expect("SIGKILL crash-point producer");
                let status = wait_for_child(&mut producer, &case);
                assert!(!status.success(), "{case} producer was not killed");

                assert_crash_observation(point, prefix, &fixture, &record, &case);

                let mut recovery = crash_matrix_command()
                    .env("MENGXIA_TASK004_CRASH_MODE", "recover")
                    .env("MENGXIA_TASK004_CRASH_LIBRARY", &fixture.library)
                    .env("MENGXIA_TASK004_CRASH_RESULT", &result)
                    .spawn()
                    .expect("start crash-point recovery process");
                let status = wait_for_child(&mut recovery, &case);
                assert!(status.success(), "{case} recovery subprocess failed");
                let outcome = fs::read_to_string(&result).expect("read recovery outcome");
                let expected = match point {
                    7 => "configuration",
                    1..=6 | 11..=16 => "needs-fresh",
                    _ => "opened",
                };
                assert_eq!(outcome, expected, "{case}");
                match expected {
                    "configuration" => {
                        assert_eq!(
                            root_names(&fixture.library),
                            [".mengxia.bootstrap-intent", ".mengxia.lock"],
                            "{case}"
                        );
                        let expected_record =
                            fs::read(&record).expect("read expected preserved intent");
                        assert_eq!(
                            fs::read(fixture.library.join(".mengxia.bootstrap-intent"))
                                .expect("read preserved invalid intent"),
                            expected_record[..prefix],
                            "{case}"
                        );
                    }
                    "needs-fresh" => {
                        assert_eq!(root_names(&fixture.library), [".mengxia.lock"], "{case}")
                    }
                    "opened" => assert_eq!(
                        root_names(&fixture.library),
                        [".mengxia.lock", "library.sqlite3"],
                        "{case}"
                    ),
                    _ => unreachable!("fixed expected outcome"),
                }
            }
        }
    }

    #[test]
    fn crash_matrix_child_process_entrypoint() {
        let Some(mode) = std::env::var_os("MENGXIA_TASK004_CRASH_MODE") else {
            return;
        };
        match mode.to_str().expect("ASCII crash mode") {
            "producer" => crash_matrix_producer(),
            "recover" => crash_matrix_recover(),
            _ => panic!("unknown crash matrix child mode"),
        }
    }

    #[test]
    fn wal_writer_child_process_entrypoint() {
        let Some(mode) = std::env::var_os("MENGXIA_TASK004_WAL_CHILD") else {
            return;
        };
        let path = PathBuf::from(
            std::env::var_os("MENGXIA_TASK004_WAL_PATH").expect("WAL child staging path"),
        );
        let ready = PathBuf::from(
            std::env::var_os("MENGXIA_TASK004_WAL_READY").expect("WAL child ready path"),
        );
        let mut connection = rusqlite::Connection::open(&path).expect("open child staging DB");
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .expect("enable child WAL mode");
        connection
            .pragma_update(None, "wal_autocheckpoint", 0_i64)
            .expect("disable child auto-checkpoint");
        if mode == "commit" || mode == "commit-then-uncommitted" {
            crate::migration::bootstrap_schema(
                &mut connection,
                fixed_library_id(),
                fs::metadata(&path).expect("staging metadata").uid(),
                fixed_timestamp(),
            )
            .expect("commit child bootstrap transaction");
        }
        if mode == "uncommitted" || mode == "commit-then-uncommitted" {
            connection
                .execute_batch("BEGIN IMMEDIATE; CREATE TABLE uncommitted_fixture (value INTEGER)")
                .expect("create uncommitted WAL frames");
        }
        fs::write(&ready, b"ready").expect("publish child crash point");
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn crash_matrix_command() -> Command {
        let mut command = Command::new(std::env::current_exe().expect("current test executable"));
        command.args([
            "--exact",
            "bootstrap::tests::crash_matrix_child_process_entrypoint",
            "--nocapture",
        ]);
        command
    }

    fn crash_matrix_producer() -> ! {
        let point: u8 = std::env::var("MENGXIA_TASK004_CRASH_POINT")
            .expect("crash point")
            .parse()
            .expect("numeric crash point");
        let prefix: usize = std::env::var("MENGXIA_TASK004_CRASH_PREFIX")
            .expect("crash prefix")
            .parse()
            .expect("numeric crash prefix");
        let library = PathBuf::from(
            std::env::var_os("MENGXIA_TASK004_CRASH_LIBRARY").expect("crash Library path"),
        );
        let ready = PathBuf::from(
            std::env::var_os("MENGXIA_TASK004_CRASH_READY").expect("crash ready path"),
        );
        let record_path = PathBuf::from(
            std::env::var_os("MENGXIA_TASK004_CRASH_RECORD").expect("crash record path"),
        );

        if point <= 6 {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&library)
                .expect("create crash-matrix Library root");
            let root = File::open(&library).expect("open crash-matrix Library root");
            root.sync_all().expect("sync new Library root");
            if point == 1 {
                acknowledge_and_wait(&ready);
            }
            File::open(library.parent().expect("Library parent"))
                .expect("open Library parent")
                .sync_all()
                .expect("sync Library parent");
            if point == 2 {
                acknowledge_and_wait(&ready);
            }
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(library.join(".mengxia.lock"))
                .expect("create crash-matrix lock");
            lock.try_lock().expect("acquire crash-matrix lock");
            if point == 3 {
                acknowledge_and_wait(&ready);
            }
            lock.sync_all().expect("sync crash-matrix lock");
            if point == 4 {
                acknowledge_and_wait(&ready);
            }
            root.sync_all().expect("sync lock name in Library root");
            if point == 5 {
                acknowledge_and_wait(&ready);
            }
            assert_eq!(root_names(&library), [".mengxia.lock"]);
            acknowledge_and_wait(&ready);
        }

        let config = config_for_library(&library);
        let authority = acquire_bootstrap_authority(&config).expect("crash-matrix authority");
        let intent =
            BootstrapIntent::for_authority(&authority, fixed_library_id(), fixed_timestamp());
        let record = intent.encode();
        fs::write(&record_path, record).expect("publish expected intent fixture");

        if point <= 9 {
            let mut intent_file = OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(library.join(".mengxia.bootstrap-intent"))
                .expect("create crash-matrix intent");
            let write_length = if point == 7 { prefix } else { record.len() };
            intent_file
                .write_all(&record[..write_length])
                .expect("write acknowledged intent prefix");
            if point == 7 || point == 8 {
                acknowledge_and_wait(&ready);
            }
            intent_file.sync_all().expect("sync complete intent");
            acknowledge_and_wait(&ready);
        }

        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("create durable crash-matrix intent");
        if point == 10 {
            acknowledge_and_wait(&ready);
        }
        if point == 11 {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(library.join(".library.sqlite3.bootstrap"))
                .expect("create acknowledged empty staging");
            acknowledge_and_wait(&ready);
        }
        if point <= 17 {
            intent
                .create_staging(&authority)
                .expect("create durable crash-matrix staging");
            if point == 12 {
                acknowledge_and_wait(&ready);
            }
            let staging = library.join(".library.sqlite3.bootstrap");
            let mut connection =
                rusqlite::Connection::open(&staging).expect("open crash-matrix staging connection");
            crate::runtime::verify_and_harden(&connection, config.busy_timeout())
                .expect("harden crash-matrix staging connection");
            if point == 13 {
                acknowledge_and_wait(&ready);
            }
            bootstrap_schema_with_crash_boundaries(
                &mut connection,
                fixed_library_id(),
                authority.owner_uid(),
                fixed_timestamp(),
                |boundary| {
                    if boundary == point {
                        acknowledge_and_wait(&ready);
                    }
                },
            )
            .expect("reach requested SQLite crash boundary");
            panic!("SQLite crash boundary was not selected");
        }

        bootstrap_staging_database(&config, &authority, intent)
            .expect("prepare complete crash-matrix staging");
        if point == 18 {
            acknowledge_and_wait(&ready);
        }
        fs::hard_link(
            library.join(".library.sqlite3.bootstrap"),
            library.join("library.sqlite3"),
        )
        .expect("publish crash-matrix canonical hard link");
        if point == 19 {
            acknowledge_and_wait(&ready);
        }
        let root = File::open(&library).expect("open Library root for publish sync");
        root.sync_all().expect("sync canonical hard link");
        if point == 20 {
            acknowledge_and_wait(&ready);
        }
        fs::remove_file(library.join(".library.sqlite3.bootstrap"))
            .expect("unlink crash-matrix staging name");
        if point == 21 {
            acknowledge_and_wait(&ready);
        }
        root.sync_all().expect("sync staging unlink");
        if point == 22 {
            acknowledge_and_wait(&ready);
        }
        fs::remove_file(library.join(".mengxia.bootstrap-intent"))
            .expect("unlink crash-matrix intent");
        acknowledge_and_wait(&ready);
    }

    fn crash_matrix_recover() {
        let library = PathBuf::from(
            std::env::var_os("MENGXIA_TASK004_CRASH_LIBRARY").expect("recovery Library path"),
        );
        let result = PathBuf::from(
            std::env::var_os("MENGXIA_TASK004_CRASH_RESULT").expect("recovery result path"),
        );
        let config = config_for_library(&library);
        let outcome = match recover_closed_library(&config) {
            Ok(RecoveryOutcome::NeedsFreshBootstrap(authority)) => {
                drop(authority);
                "needs-fresh"
            }
            Ok(RecoveryOutcome::Opened {
                authority,
                metadata,
            }) => {
                assert_eq!(metadata.library_id, fixed_library_id());
                assert_eq!(metadata.owner_uid, authority.owner_uid());
                assert_eq!(metadata.created_at, fixed_timestamp());
                drop(authority);
                "opened"
            }
            Err(crate::StoreError::Configuration) => "configuration",
            Err(error) => panic!("unexpected crash recovery error: {error}"),
        };
        fs::write(result, outcome).expect("write crash recovery outcome");
    }

    fn config_for_library(library: &Path) -> StoreConfig {
        ResolvedStoreConfig::from_selected(
            Some(library.to_path_buf()),
            ConfigSource::Cli,
            256,
            ConfigSource::CompiledDefault,
            4,
            ConfigSource::CompiledDefault,
            37,
            ConfigSource::CompiledDefault,
        )
        .validate()
        .expect("valid crash-matrix store config")
    }

    fn acknowledge_and_wait(ready: &Path) -> ! {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(ready)
            .expect("create crash-point acknowledgement");
        file.sync_all().expect("sync crash-point acknowledgement");
        loop {
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn wait_for_file(path: &Path, case: &str) {
        for _ in 0..3_000 {
            if path.is_file() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("{case} did not acknowledge within 30 seconds");
    }

    fn wait_for_child(child: &mut std::process::Child, case: &str) -> std::process::ExitStatus {
        for _ in 0..3_000 {
            if let Some(status) = child.try_wait().expect("poll crash-matrix child") {
                return status;
            }
            thread::sleep(Duration::from_millis(10));
        }
        child.kill().expect("kill timed-out crash-matrix child");
        let _ = child.wait();
        panic!("{case} child exceeded 30-second watchdog");
    }

    fn assert_crash_observation(
        point: u8,
        prefix: usize,
        fixture: &Fixture,
        record_path: &Path,
        case: &str,
    ) {
        assert!(fixture.library.is_dir(), "{case} Library root missing");
        assert_eq!(
            fs::metadata(&fixture.library)
                .expect("Library root metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700,
            "{case}"
        );
        let expected_names: &[&str] = match point {
            1 | 2 => &[],
            3..=6 => &[".mengxia.lock"],
            7..=10 => &[".mengxia.bootstrap-intent", ".mengxia.lock"],
            11..=13 | 18 => &[
                ".library.sqlite3.bootstrap",
                ".mengxia.bootstrap-intent",
                ".mengxia.lock",
            ],
            14..=17 => &[
                ".library.sqlite3.bootstrap",
                ".library.sqlite3.bootstrap-shm",
                ".library.sqlite3.bootstrap-wal",
                ".mengxia.bootstrap-intent",
                ".mengxia.lock",
            ],
            19 | 20 => &[
                ".library.sqlite3.bootstrap",
                ".mengxia.bootstrap-intent",
                ".mengxia.lock",
                "library.sqlite3",
            ],
            21 | 22 => &[
                ".mengxia.bootstrap-intent",
                ".mengxia.lock",
                "library.sqlite3",
            ],
            23 => &[".mengxia.lock", "library.sqlite3"],
            _ => unreachable!("point range is fixed"),
        };
        assert_eq!(root_names(&fixture.library), expected_names, "{case}");
        if point >= 3 {
            assert_owner_only(&fixture.library.join(".mengxia.lock"));
        }
        if (7..=22).contains(&point) {
            let expected_record = fs::read(record_path).expect("read expected intent record");
            let observed = fs::read(fixture.library.join(".mengxia.bootstrap-intent"))
                .expect("read observed intent record");
            if point == 7 {
                assert_eq!(observed, expected_record[..prefix], "{case}");
            } else {
                assert_eq!(observed, expected_record, "{case}");
            }
        }
        if (11..=20).contains(&point) {
            assert_owner_only(&fixture.library.join(".library.sqlite3.bootstrap"));
        }
        if point == 19 || point == 20 {
            let staging = fs::metadata(fixture.library.join(".library.sqlite3.bootstrap"))
                .expect("staging metadata");
            let canonical =
                fs::metadata(fixture.library.join("library.sqlite3")).expect("canonical metadata");
            assert_eq!(
                (staging.dev(), staging.ino()),
                (canonical.dev(), canonical.ino())
            );
        }
    }

    #[test]
    fn restart_preserves_committed_but_tampered_staging_for_inspection() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");
        bootstrap_staging_database(&config, &authority, intent)
            .expect("verified staging bootstrap");
        let tamper = rusqlite::Connection::open(fixture.library.join(".library.sqlite3.bootstrap"))
            .expect("open staging tamper fixture");
        tamper
            .execute("UPDATE schema_migrations SET sha256 = zeroblob(32)", [])
            .expect("tamper committed migration row");
        drop(tamper);
        drop(authority);

        assert!(matches!(
            recover_closed_library(&config),
            Err(crate::StoreError::Corruption)
        ));
        assert_eq!(
            root_names(&fixture.library),
            [
                ".library.sqlite3.bootstrap",
                ".mengxia.bootstrap-intent",
                ".mengxia.lock",
            ]
        );
    }

    #[test]
    fn restart_rejects_unsafe_staging_sidecar_without_cleanup() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");
        intent
            .create_staging(&authority)
            .expect("durable empty staging");
        let target = fixture.parent.join("outside-sidecar-target");
        fs::write(&target, b"outside").expect("create sidecar symlink target");
        symlink(
            &target,
            fixture.library.join(".library.sqlite3.bootstrap-wal"),
        )
        .expect("create unsafe WAL symlink");
        drop(authority);

        assert!(matches!(
            recover_closed_library(&config),
            Err(crate::StoreError::Configuration)
        ));
        assert_eq!(
            root_names(&fixture.library),
            [
                ".library.sqlite3.bootstrap",
                ".library.sqlite3.bootstrap-wal",
                ".mengxia.bootstrap-intent",
                ".mengxia.lock",
            ]
        );
        assert_eq!(
            fs::read(&target).expect("read preserved target"),
            b"outside"
        );
    }

    #[test]
    fn restart_rejects_distinct_published_inode_without_cleanup() {
        use std::fs::OpenOptions;
        use std::os::unix::fs::OpenOptionsExt;

        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");
        bootstrap_staging_database(&config, &authority, intent)
            .expect("verified staging bootstrap");
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(fixture.library.join("library.sqlite3"))
            .expect("create conflicting canonical inode");
        drop(authority);

        assert!(matches!(
            recover_closed_library(&config),
            Err(crate::StoreError::Corruption)
        ));
        assert_eq!(
            root_names(&fixture.library),
            [
                ".library.sqlite3.bootstrap",
                ".mengxia.bootstrap-intent",
                ".mengxia.lock",
                "library.sqlite3",
            ]
        );
    }

    #[test]
    fn restart_preserves_tampered_canonical_and_intent_for_inspection() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let authority = acquire_bootstrap_authority(&config).expect("bootstrap authority");
        let intent =
            BootstrapIntent::create_durable(&authority, fixed_library_id(), fixed_timestamp())
                .expect("durable intent");
        bootstrap_staging_database(&config, &authority, intent)
            .expect("verified staging bootstrap");
        fs::hard_link(
            fixture.library.join(".library.sqlite3.bootstrap"),
            fixture.library.join("library.sqlite3"),
        )
        .expect("simulate publish link");
        fs::remove_file(fixture.library.join(".library.sqlite3.bootstrap"))
            .expect("simulate staging unlink");
        let tamper = rusqlite::Connection::open(fixture.library.join("library.sqlite3"))
            .expect("open canonical tamper fixture");
        tamper
            .execute("UPDATE library_meta SET owner_uid = owner_uid + 1", [])
            .expect("tamper canonical owner");
        drop(tamper);
        drop(authority);

        assert!(matches!(
            recover_closed_library(&config),
            Err(crate::StoreError::Corruption)
        ));
        assert!(fixture.library.join("library.sqlite3").is_file());
        assert!(fixture.library.join(".mengxia.bootstrap-intent").is_file());
        assert!(!fixture.library.join(".library.sqlite3.bootstrap").exists());
    }

    fn fixed_library_id() -> Id<LibraryIdentity> {
        Id::from_bytes([
            0x01, 0x89, 0x0f, 0x1d, 0xe0, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01,
        ])
        .expect("fixed UUIDv7")
    }

    fn fixed_timestamp() -> Timestamp {
        Timestamp::from_unix_seconds_nanos(1_700_000_000, 123_456_789).expect("fixed timestamp")
    }

    fn assert_owner_only(path: &Path) {
        let mode = fs::metadata(path)
            .expect("staging metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    fn root_names(library: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(library)
            .expect("enumerate Library root")
            .map(|entry| {
                entry
                    .expect("Library entry")
                    .file_name()
                    .into_string()
                    .expect("fixed ASCII entry")
            })
            .collect();
        names.sort_unstable();
        names
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RestartFixture {
        IntentOnly,
        Staging,
        PublishedStaging,
        CanonicalIntent,
        CanonicalOnly,
    }

    #[derive(Clone, Copy)]
    enum WalMutation {
        RemoveShm,
        MalformedShm,
        TrailingWalBytes,
    }

    #[derive(Clone, Copy)]
    enum RequiredWalMutation {
        Payload,
        Salt,
        Checksum,
    }
}
