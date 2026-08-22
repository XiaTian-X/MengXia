use mengxia_types::{Id, Sha256Digest, Timestamp};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use super::StoreError;

const MIGRATION_SEQUENCE: i64 = 0;
const MIGRATION_NAME: &str = "0000_store_bootstrap";
const MIGRATION_SQL: &str = include_str!("../../../migrations/sqlite/0000_store_bootstrap.sql");
const MIGRATION_SHA256: [u8; 32] = [
    0x35, 0xa6, 0x9e, 0x30, 0xb6, 0x27, 0xe9, 0x94, 0xa1, 0x72, 0xc9, 0x49, 0x0f, 0x39, 0x15, 0x52,
    0xa8, 0xd6, 0x02, 0x12, 0xc7, 0x5a, 0xd2, 0xf4, 0x78, 0xea, 0x10, 0x05, 0xc0, 0xb9, 0x4c, 0xe2,
];

pub(crate) enum LibraryIdentity {}

pub(crate) fn migration_digest() -> Sha256Digest {
    Sha256Digest::from_bytes(MIGRATION_SHA256)
}

pub(crate) fn bootstrap_schema(
    connection: &mut Connection,
    library_id: Id<LibraryIdentity>,
    owner_uid: u32,
    created_at: Timestamp,
) -> Result<(), StoreError> {
    verify_embedded_migration()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sqlite_write_error)?;
    transaction
        .execute_batch(MIGRATION_SQL)
        .map_err(map_sqlite_write_error)?;

    let timestamp_seconds = created_at.unix_seconds();
    let timestamp_nanos = i64::from(created_at.subsec_nanoseconds());
    transaction
        .execute(
            "INSERT INTO library_meta (singleton, library_id, owner_uid, created_at_seconds, created_at_nanos) VALUES (1, ?1, ?2, ?3, ?4)",
            params![library_id.to_bytes().as_slice(), i64::from(owner_uid), timestamp_seconds, timestamp_nanos],
        )
        .map_err(map_sqlite_write_error)?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (migration_sequence, migration_name, sha256, applied_at_seconds, applied_at_nanos) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![MIGRATION_SEQUENCE, MIGRATION_NAME, MIGRATION_SHA256.as_slice(), timestamp_seconds, timestamp_nanos],
        )
        .map_err(map_sqlite_write_error)?;

    verify_rows(&transaction, library_id, owner_uid, created_at)?;
    transaction.commit().map_err(map_sqlite_write_error)
}

pub(crate) fn verify_bootstrap_schema(
    connection: &Connection,
) -> Result<OpenedLibraryMetadata, StoreError> {
    verify_embedded_migration()?;
    let library_row = connection
        .query_row(
            "SELECT library_id, owner_uid, created_at_seconds, created_at_nanos FROM library_meta WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| StoreError::Corruption)?
        .ok_or(StoreError::Corruption)?;
    let migration_row = connection
        .query_row(
            "SELECT migration_name, sha256, applied_at_seconds, applied_at_nanos FROM schema_migrations WHERE migration_sequence = 0",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| StoreError::Corruption)?
        .ok_or(StoreError::Corruption)?;

    if connection
        .query_row("SELECT count(*) FROM library_meta", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|_| StoreError::Corruption)?
        != 1
        || connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(|_| StoreError::Corruption)?
            != 1
        || migration_row.0 != MIGRATION_NAME
        || migration_row.1.as_slice() != MIGRATION_SHA256
        || library_row.2 != migration_row.2
        || library_row.3 != migration_row.3
    {
        return Err(StoreError::Corruption);
    }

    let library_bytes: [u8; 16] = library_row
        .0
        .try_into()
        .map_err(|_| StoreError::Corruption)?;
    let library_id = Id::from_bytes(library_bytes).map_err(|_| StoreError::Corruption)?;
    let owner_uid = u32::try_from(library_row.1).map_err(|_| StoreError::Corruption)?;
    let nanos = u32::try_from(library_row.3).map_err(|_| StoreError::Corruption)?;
    let created_at = Timestamp::from_unix_seconds_nanos(library_row.2, nanos)
        .map_err(|_| StoreError::Corruption)?;

    Ok(OpenedLibraryMetadata {
        library_id,
        owner_uid,
        created_at,
    })
}

pub(crate) struct OpenedLibraryMetadata {
    pub(crate) library_id: Id<LibraryIdentity>,
    pub(crate) owner_uid: u32,
    pub(crate) created_at: Timestamp,
}

fn verify_rows(
    connection: &Connection,
    library_id: Id<LibraryIdentity>,
    owner_uid: u32,
    created_at: Timestamp,
) -> Result<(), StoreError> {
    let opened = verify_bootstrap_schema(connection)?;
    if opened.library_id == library_id
        && opened.owner_uid == owner_uid
        && opened.created_at == created_at
    {
        Ok(())
    } else {
        Err(StoreError::Corruption)
    }
}

fn verify_embedded_migration() -> Result<(), StoreError> {
    let actual: [u8; 32] = Sha256::digest(MIGRATION_SQL.as_bytes()).into();
    if actual == MIGRATION_SHA256 {
        Ok(())
    } else {
        Err(StoreError::Internal)
    }
}

fn map_sqlite_write_error(error: rusqlite::Error) -> StoreError {
    use rusqlite::ErrorCode;

    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy) => StoreError::Busy,
        Some(ErrorCode::DatabaseLocked) => StoreError::Internal,
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => StoreError::Corruption,
        Some(
            ErrorCode::CannotOpen
            | ErrorCode::SystemIoFailure
            | ErrorCode::DiskFull
            | ErrorCode::ReadOnly
            | ErrorCode::PermissionDenied
            | ErrorCode::OutOfMemory,
        ) => StoreError::Io,
        _ => StoreError::Internal,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use mengxia_types::{Id, Timestamp};
    use rusqlite::Connection;

    use super::{
        LibraryIdentity, MIGRATION_SHA256, bootstrap_schema, migration_digest,
        verify_bootstrap_schema,
    };
    use crate::StoreError;
    use crate::runtime::verify_and_harden;

    #[test]
    fn fresh_bootstrap_and_reopen_preserve_exact_typed_singletons() {
        let (directory, mut connection) = file_connection("bootstrap");
        verify_and_harden(&connection).expect("harden connection");
        let library_id = Id::<LibraryIdentity>::from_bytes([
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ])
        .expect("fixed UUIDv7");
        let timestamp = Timestamp::from_unix_seconds_nanos(1_777_000_000, 123_456_789)
            .expect("fixed timestamp");

        bootstrap_schema(&mut connection, library_id, 501, timestamp).expect("bootstrap");
        let opened = verify_bootstrap_schema(&connection).expect("reopen validation");
        assert_eq!(opened.library_id, library_id);
        assert_eq!(opened.owner_uid, 501);
        assert_eq!(opened.created_at, timestamp);
        assert_eq!(migration_digest().to_bytes(), MIGRATION_SHA256);
        drop(connection);
        fs::remove_dir_all(directory).expect("remove bootstrap directory");
    }

    #[test]
    fn migration_digest_tamper_fails_reopen() {
        let (directory, mut connection) = file_connection("tamper");
        verify_and_harden(&connection).expect("harden connection");
        let library_id = Id::<LibraryIdentity>::from_bytes([
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ])
        .expect("fixed UUIDv7");
        let timestamp =
            Timestamp::from_unix_seconds_nanos(1_777_000_000, 0).expect("fixed timestamp");
        bootstrap_schema(&mut connection, library_id, 501, timestamp).expect("bootstrap");
        connection
            .execute("UPDATE schema_migrations SET sha256 = zeroblob(32)", [])
            .expect("tamper migration digest");
        assert!(matches!(
            verify_bootstrap_schema(&connection),
            Err(StoreError::Corruption)
        ));
        drop(connection);
        fs::remove_dir_all(directory).expect("remove tamper directory");
    }

    fn file_connection(case: &str) -> (std::path::PathBuf, Connection) {
        let directory = std::env::temp_dir().join(format!(
            "mengxia-task004-migration-{}-{case}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create migration test directory");
        let connection = Connection::open(directory.join("library.sqlite3"))
            .expect("open file-backed temporary SQLite");
        (directory, connection)
    }
}
