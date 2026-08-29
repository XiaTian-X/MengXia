use mengxia_types::{Id, Sha256Digest, Timestamp};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use super::StoreError;
use super::error::map_reopen_error;
use super::error::map_sqlite_error;

pub(crate) const MIGRATION_SEQUENCE: i64 = 0;
pub(crate) const MIGRATION_NAME: &str = "0000_store_bootstrap";
const MIGRATION_SQL: &str = include_str!("../../../migrations/sqlite/0000_store_bootstrap.sql");
const ASSET_MIGRATION_SEQUENCE: i64 = 1;
const ASSET_MIGRATION_NAME: &str = "0001_library_assets";
const ASSET_MIGRATION_SQL: &str =
    include_str!("../../../migrations/sqlite/0001_library_assets.sql");
const ASSET_MIGRATION_SHA256: [u8; 32] = [
    0x91, 0xc7, 0x6e, 0x61, 0x5f, 0xe2, 0x48, 0xab, 0xd8, 0x52, 0x86, 0x0d, 0xcd, 0x42, 0xb3, 0x2a,
    0x01, 0xf6, 0xf0, 0x24, 0xe9, 0x1a, 0xc8, 0x38, 0x7f, 0x34, 0x06, 0x9b, 0xe2, 0x43, 0x5d, 0xb1,
];
type SchemaIdentity = (String, String, String, Option<String>);
pub(crate) const MIGRATION_SHA256: [u8; 32] = [
    0x35, 0xa6, 0x9e, 0x30, 0xb6, 0x27, 0xe9, 0x94, 0xa1, 0x72, 0xc9, 0x49, 0x0f, 0x39, 0x15, 0x52,
    0xa8, 0xd6, 0x02, 0x12, 0xc7, 0x5a, 0xd2, 0xf4, 0x78, 0xea, 0x10, 0x05, 0xc0, 0xb9, 0x4c, 0xe2,
];

const SCHEMA_MIGRATIONS_SQL: &str = r#"CREATE TABLE schema_migrations (
    migration_sequence INTEGER PRIMARY KEY NOT NULL
        CHECK (migration_sequence BETWEEN 0 AND 9999),
    migration_name TEXT NOT NULL UNIQUE,
    sha256 BLOB NOT NULL CHECK (length(sha256) = 32),
    applied_at_seconds INTEGER NOT NULL,
    applied_at_nanos INTEGER NOT NULL
        CHECK (applied_at_nanos BETWEEN 0 AND 999999999)
) STRICT"#;

const LIBRARY_META_SQL: &str = r#"CREATE TABLE library_meta (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    library_id BLOB NOT NULL UNIQUE CHECK (length(library_id) = 16),
    owner_uid INTEGER NOT NULL CHECK (owner_uid BETWEEN 0 AND 4294967295),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL
        CHECK (created_at_nanos BETWEEN 0 AND 999999999)
) STRICT"#;

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
    bootstrap_schema_inner(connection, library_id, owner_uid, created_at, |_| {})
}

fn bootstrap_schema_inner(
    connection: &mut Connection,
    library_id: Id<LibraryIdentity>,
    owner_uid: u32,
    created_at: Timestamp,
    mut boundary: impl FnMut(u8),
) -> Result<(), StoreError> {
    verify_embedded_migration()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sqlite_error)?;
    transaction
        .execute_batch(MIGRATION_SQL)
        .map_err(map_sqlite_error)?;
    boundary(14);

    let timestamp_seconds = created_at.unix_seconds();
    let timestamp_nanos = i64::from(created_at.subsec_nanoseconds());
    transaction
        .execute(
            "INSERT INTO library_meta (singleton, library_id, owner_uid, created_at_seconds, created_at_nanos) VALUES (1, ?1, ?2, ?3, ?4)",
            params![library_id.to_bytes().as_slice(), i64::from(owner_uid), timestamp_seconds, timestamp_nanos],
        )
        .map_err(map_sqlite_error)?;
    boundary(15);
    transaction
        .execute(
            "INSERT INTO schema_migrations (migration_sequence, migration_name, sha256, applied_at_seconds, applied_at_nanos) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![MIGRATION_SEQUENCE, MIGRATION_NAME, MIGRATION_SHA256.as_slice(), timestamp_seconds, timestamp_nanos],
        )
        .map_err(map_sqlite_error)?;
    boundary(16);

    verify_rows(&transaction, library_id, owner_uid, created_at)?;
    transaction.commit().map_err(map_sqlite_error)?;
    boundary(17);
    Ok(())
}

#[cfg(test)]
pub(crate) fn bootstrap_schema_with_crash_boundaries(
    connection: &mut Connection,
    library_id: Id<LibraryIdentity>,
    owner_uid: u32,
    created_at: Timestamp,
    boundary: impl FnMut(u8),
) -> Result<(), StoreError> {
    bootstrap_schema_inner(connection, library_id, owner_uid, created_at, boundary)
}

pub(crate) fn verify_bootstrap_schema(
    connection: &Connection,
) -> Result<OpenedLibraryMetadata, StoreError> {
    verify_embedded_migration()?;
    verify_quick_check(connection)?;
    verify_schema_allowlist(connection)?;
    verify_table_contracts(connection)?;
    let metadata = read_and_validate_rows(connection)?;
    verify_index_contents(connection, metadata)?;
    Ok(metadata)
}

pub(crate) fn verify_bootstrap_schema_matches(
    connection: &Connection,
    library_id: Id<LibraryIdentity>,
    owner_uid: u32,
    created_at: Timestamp,
) -> Result<OpenedLibraryMetadata, StoreError> {
    let opened = verify_bootstrap_schema(connection)?;
    if opened.library_id == library_id
        && opened.owner_uid == owner_uid
        && opened.created_at == created_at
    {
        Ok(opened)
    } else {
        Err(StoreError::Corruption)
    }
}

pub(crate) fn prepare_current_library_schema(
    connection: &mut Connection,
    expected: OpenedLibraryMetadata,
) -> Result<OpenedLibraryMetadata, StoreError> {
    verify_asset_migration()?;
    let migration_count = connection
        .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(map_reopen_error)?;
    match migration_count {
        1 => {
            let actual = read_and_validate_rows(connection)?;
            if actual != expected {
                return Err(StoreError::Corruption);
            }
            apply_asset_migration(connection)?;
        }
        2 => {
            verify_current_library_connection_metadata(connection, expected)?;
        }
        count if count > 2 => return classify_newer_migration_prefix(connection, count),
        _ => return Err(StoreError::Corruption),
    }
    verify_current_library_connection_metadata(connection, expected)
}

/// Revalidates the bounded identity and immutable migration prefix for a
/// connection after the owning Library open has already performed its one
/// full integrity/schema verification.
pub(crate) fn verify_current_library_connection_metadata(
    connection: &Connection,
    expected: OpenedLibraryMetadata,
) -> Result<OpenedLibraryMetadata, StoreError> {
    verify_asset_migration()?;
    verify_current_migration_rows(connection)?;
    let actual = read_current_metadata(connection)?;
    if actual == expected {
        Ok(actual)
    } else {
        Err(StoreError::Corruption)
    }
}

pub(crate) fn verify_current_library_schema_matches(
    connection: &Connection,
    expected: OpenedLibraryMetadata,
) -> Result<OpenedLibraryMetadata, StoreError> {
    let actual = verify_current_library_schema(connection)?;
    if actual == expected {
        Ok(actual)
    } else {
        Err(StoreError::Corruption)
    }
}

pub(crate) fn verify_current_library_schema(
    connection: &Connection,
) -> Result<OpenedLibraryMetadata, StoreError> {
    verify_asset_migration()?;
    verify_quick_check(connection)?;
    verify_current_migration_rows(connection)?;
    verify_current_schema_allowlist(connection)?;
    verify_current_singletons(connection)?;
    read_current_metadata(connection)
}

pub(crate) fn verify_reopen_library_schema(
    connection: &Connection,
) -> Result<OpenedLibraryMetadata, StoreError> {
    let migration_count = connection
        .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(map_reopen_error)?;
    match migration_count {
        1 => verify_bootstrap_schema(connection),
        2 => verify_current_library_schema(connection),
        count if count > 2 => {
            verify_quick_check(connection)?;
            classify_newer_migration_prefix(connection, count)
        }
        _ => Err(StoreError::Corruption),
    }
}

fn apply_asset_migration(connection: &mut Connection) -> Result<(), StoreError> {
    apply_asset_migration_inner(connection, |_| Ok(()))
}

fn apply_asset_migration_inner(
    connection: &mut Connection,
    mut boundary: impl FnMut(u8) -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    let applied_at = current_timestamp()?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sqlite_error)?;
    boundary(1)?;
    transaction
        .execute_batch(ASSET_MIGRATION_SQL)
        .map_err(map_sqlite_error)?;
    boundary(2)?;
    transaction.execute(
        "INSERT INTO schema_migrations (migration_sequence, migration_name, sha256, applied_at_seconds, applied_at_nanos) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![ASSET_MIGRATION_SEQUENCE, ASSET_MIGRATION_NAME, ASSET_MIGRATION_SHA256.as_slice(), applied_at.unix_seconds(), i64::from(applied_at.subsec_nanoseconds())],
    ).map_err(map_sqlite_error)?;
    boundary(3)?;
    let foreign_key_rows = transaction
        .prepare("PRAGMA foreign_key_check")
        .map_err(map_reopen_error)?
        .query_map([], |_| Ok(()))
        .map_err(map_reopen_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_reopen_error)?;
    if !foreign_key_rows.is_empty() {
        return Err(StoreError::Corruption);
    }
    boundary(4)?;
    verify_current_migration_rows(&transaction)?;
    verify_current_schema_allowlist(&transaction)?;
    verify_current_singletons(&transaction)?;
    boundary(5)?;
    transaction.commit().map_err(map_sqlite_error)?;
    boundary(6)?;
    Ok(())
}

#[cfg(test)]
fn apply_asset_migration_with_boundaries(
    connection: &mut Connection,
    boundary: impl FnMut(u8) -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    apply_asset_migration_inner(connection, boundary)
}

fn current_timestamp() -> Result<Timestamp, StoreError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StoreError::IdGenerationUnavailable)?;
    let seconds =
        i64::try_from(duration.as_secs()).map_err(|_| StoreError::IdGenerationUnavailable)?;
    Timestamp::from_unix_seconds_nanos(seconds, duration.subsec_nanos())
        .map_err(|_| StoreError::IdGenerationUnavailable)
}

fn verify_asset_migration() -> Result<(), StoreError> {
    if ASSET_MIGRATION_SQL.len() != 12_733 {
        return Err(StoreError::Internal);
    }
    let digest: [u8; 32] = Sha256::digest(ASSET_MIGRATION_SQL.as_bytes()).into();
    if digest == ASSET_MIGRATION_SHA256 {
        Ok(())
    } else {
        Err(StoreError::Internal)
    }
}

fn verify_current_migration_rows(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection.prepare(
        "SELECT migration_sequence, migration_name, sha256, applied_at_seconds, applied_at_nanos FROM schema_migrations ORDER BY migration_sequence"
    ).map_err(map_reopen_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Vec<u8>>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(map_reopen_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_reopen_error)?;
    let timestamps = rows
        .iter()
        .map(|row| {
            let nanos = u32::try_from(row.4).map_err(|_| StoreError::Corruption)?;
            Timestamp::from_unix_seconds_nanos(row.3, nanos).map_err(|_| StoreError::Corruption)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let metadata = read_current_metadata(connection)?;
    if rows.len() == 2
        && rows[0].0 == MIGRATION_SEQUENCE
        && rows[0].1 == MIGRATION_NAME
        && rows[0].2.as_slice() == MIGRATION_SHA256
        && rows[1].0 == ASSET_MIGRATION_SEQUENCE
        && rows[1].1 == ASSET_MIGRATION_NAME
        && rows[1].2.as_slice() == ASSET_MIGRATION_SHA256
        && timestamps[0] == metadata.created_at
    {
        Ok(())
    } else {
        Err(StoreError::Corruption)
    }
}

fn classify_newer_migration_prefix(
    connection: &Connection,
    count: i64,
) -> Result<OpenedLibraryMetadata, StoreError> {
    let rows = connection.prepare("SELECT migration_sequence, migration_name, sha256, applied_at_seconds, applied_at_nanos FROM schema_migrations ORDER BY migration_sequence")
        .map_err(map_reopen_error)?
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Vec<u8>>(2)?, row.get::<_, i64>(3)?, row.get::<_, i64>(4)?)))
        .map_err(map_reopen_error)?.collect::<Result<Vec<_>, _>>().map_err(map_reopen_error)?;
    let syntactically_valid = i64::try_from(rows.len()).ok() == Some(count)
        && rows.first().is_some_and(|row| {
            row.0 == MIGRATION_SEQUENCE
                && row.1 == MIGRATION_NAME
                && row.2.as_slice() == MIGRATION_SHA256
        })
        && rows.get(1).is_some_and(|row| {
            row.0 == ASSET_MIGRATION_SEQUENCE
                && row.1 == ASSET_MIGRATION_NAME
                && row.2.as_slice() == ASSET_MIGRATION_SHA256
        })
        && rows
            .iter()
            .enumerate()
            .all(|(index, (sequence, name, digest, seconds, nanos))| {
                *sequence == i64::try_from(index).unwrap_or(-1)
                    && name.len() >= 5
                    && name.len() <= 128
                    && name.is_ascii()
                    && name.as_bytes()[..4].iter().all(u8::is_ascii_digit)
                    && name.as_bytes()[4] == b'_'
                    && name.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
                    && digest.len() == 32
                    && u32::try_from(*nanos).ok().is_some_and(|nanos| {
                        Timestamp::from_unix_seconds_nanos(*seconds, nanos).is_ok()
                    })
            });
    if syntactically_valid {
        Err(StoreError::Configuration)
    } else {
        Err(StoreError::Corruption)
    }
}

fn read_current_metadata(connection: &Connection) -> Result<OpenedLibraryMetadata, StoreError> {
    let row = connection.query_row(
        "SELECT library_id, owner_uid, created_at_seconds, created_at_nanos FROM library_meta WHERE singleton = 1",
        [],
        |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get::<_, i64>(3)?)),
    ).map_err(map_reopen_error)?;
    if connection
        .query_row("SELECT count(*) FROM library_meta", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(map_reopen_error)?
        != 1
    {
        return Err(StoreError::Corruption);
    }
    let library_id = Id::from_bytes(row.0.try_into().map_err(|_| StoreError::Corruption)?)
        .map_err(|_| StoreError::Corruption)?;
    let owner_uid = u32::try_from(row.1).map_err(|_| StoreError::Corruption)?;
    let nanos = u32::try_from(row.3).map_err(|_| StoreError::Corruption)?;
    let created_at =
        Timestamp::from_unix_seconds_nanos(row.2, nanos).map_err(|_| StoreError::Corruption)?;
    Ok(OpenedLibraryMetadata {
        library_id,
        owner_uid,
        created_at,
    })
}

fn verify_current_singletons(connection: &Connection) -> Result<(), StoreError> {
    let sequence = connection
        .query_row(
            "SELECT last_sequence FROM event_commit_sequence WHERE singleton = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(map_reopen_error)?
        .ok_or(StoreError::Corruption)?;
    let count = connection
        .query_row("SELECT count(*) FROM event_commit_sequence", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(map_reopen_error)?;
    if count == 1 && sequence >= 0 {
        Ok(())
    } else {
        Err(StoreError::Corruption)
    }
}

fn verify_current_schema_allowlist(connection: &Connection) -> Result<(), StoreError> {
    let expected = expected_current_schema()?;
    let actual = read_schema_identity(connection)?;
    if actual == expected {
        Ok(())
    } else {
        Err(StoreError::Corruption)
    }
}

fn expected_current_schema() -> Result<Vec<SchemaIdentity>, StoreError> {
    let connection = Connection::open_in_memory().map_err(map_sqlite_error)?;
    connection
        .execute_batch(MIGRATION_SQL)
        .map_err(map_sqlite_error)?;
    connection
        .execute_batch(ASSET_MIGRATION_SQL)
        .map_err(map_sqlite_error)?;
    read_schema_identity(&connection)
}

fn read_schema_identity(connection: &Connection) -> Result<Vec<SchemaIdentity>, StoreError> {
    connection
        .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY type, name")
        .map_err(map_reopen_error)?
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(map_reopen_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_reopen_error)
}

/// Proves the one cleanup-safe incomplete state after SQLite has applied its
/// normal WAL recovery: the database passes `quick_check` and has no committed
/// schema object at all. Any object, including a partial/foreign bootstrap
/// shape, is treated as persistent evidence and must be preserved.
pub(crate) fn bootstrap_commit_is_absent(connection: &Connection) -> Result<bool, StoreError> {
    verify_quick_check(connection)?;
    let object_count = connection
        .query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(map_reopen_error)?;
    Ok(object_count == 0)
}

fn read_and_validate_rows(connection: &Connection) -> Result<OpenedLibraryMetadata, StoreError> {
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
        .map_err(map_reopen_error)?
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
        .map_err(map_reopen_error)?
        .ok_or(StoreError::Corruption)?;

    if connection
        .query_row("SELECT count(*) FROM library_meta", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(map_reopen_error)?
        != 1
        || connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(map_reopen_error)?
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

fn verify_quick_check(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection
        .prepare("PRAGMA quick_check")
        .map_err(map_reopen_error)?;
    let results = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(map_reopen_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_reopen_error)?;
    if results == ["ok"] {
        Ok(())
    } else {
        Err(StoreError::Corruption)
    }
}

fn verify_index_contents(
    connection: &Connection,
    metadata: OpenedLibraryMetadata,
) -> Result<(), StoreError> {
    let migration_matches = connection
        .query_row(
            "SELECT count(*) FROM schema_migrations INDEXED BY sqlite_autoindex_schema_migrations_1 WHERE migration_name = ?1 AND migration_sequence = ?2",
            params![MIGRATION_NAME, MIGRATION_SEQUENCE],
            |row| row.get::<_, i64>(0),
        )
        .map_err(map_reopen_error)?;
    let library_matches = connection
        .query_row(
            "SELECT count(*) FROM library_meta INDEXED BY sqlite_autoindex_library_meta_1 WHERE library_id = ?1 AND singleton = 1",
            params![metadata.library_id.to_bytes().as_slice()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(map_reopen_error)?;
    if migration_matches == 1 && library_matches == 1 {
        Ok(())
    } else {
        Err(StoreError::Corruption)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SchemaRow {
    object_type: String,
    name: String,
    table_name: String,
    root_page: i64,
    sql: Option<String>,
}

fn verify_schema_allowlist(connection: &Connection) -> Result<(), StoreError> {
    let mut statement = connection
        .prepare(
            "SELECT type, name, tbl_name, rootpage, sql FROM sqlite_schema ORDER BY type, name",
        )
        .map_err(map_reopen_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SchemaRow {
                object_type: row.get(0)?,
                name: row.get(1)?,
                table_name: row.get(2)?,
                root_page: row.get(3)?,
                sql: row.get(4)?,
            })
        })
        .map_err(map_reopen_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_reopen_error)?;
    let expected = [
        SchemaRow {
            object_type: "index".to_owned(),
            name: "sqlite_autoindex_library_meta_1".to_owned(),
            table_name: "library_meta".to_owned(),
            root_page: 0,
            sql: None,
        },
        SchemaRow {
            object_type: "index".to_owned(),
            name: "sqlite_autoindex_schema_migrations_1".to_owned(),
            table_name: "schema_migrations".to_owned(),
            root_page: 0,
            sql: None,
        },
        SchemaRow {
            object_type: "table".to_owned(),
            name: "library_meta".to_owned(),
            table_name: "library_meta".to_owned(),
            root_page: 0,
            sql: Some(LIBRARY_META_SQL.to_owned()),
        },
        SchemaRow {
            object_type: "table".to_owned(),
            name: "schema_migrations".to_owned(),
            table_name: "schema_migrations".to_owned(),
            root_page: 0,
            sql: Some(SCHEMA_MIGRATIONS_SQL.to_owned()),
        },
    ];
    if rows.len() != expected.len()
        || rows.iter().zip(expected).any(|(actual, expected)| {
            actual.object_type != expected.object_type
                || actual.name != expected.name
                || actual.table_name != expected.table_name
                || actual.root_page <= 0
                || actual.sql != expected.sql
        })
    {
        return Err(StoreError::Corruption);
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct ColumnContract {
    id: i64,
    name: String,
    declared_type: String,
    not_null: i64,
    default_value: Option<String>,
    primary_key: i64,
    hidden: i64,
}

#[derive(Debug, Eq, PartialEq)]
struct IndexListContract {
    sequence: i64,
    name: String,
    unique: i64,
    origin: String,
    partial: i64,
}

#[derive(Debug, Eq, PartialEq)]
struct IndexColumnContract {
    sequence: i64,
    column_id: i64,
    name: Option<String>,
    descending: i64,
    collation: String,
    key: i64,
}

fn verify_table_contracts(connection: &Connection) -> Result<(), StoreError> {
    const SCHEMA_COLUMNS: &[(&str, &str, i64)] = &[
        ("migration_sequence", "INTEGER", 1),
        ("migration_name", "TEXT", 0),
        ("sha256", "BLOB", 0),
        ("applied_at_seconds", "INTEGER", 0),
        ("applied_at_nanos", "INTEGER", 0),
    ];
    const META_COLUMNS: &[(&str, &str, i64)] = &[
        ("singleton", "INTEGER", 1),
        ("library_id", "BLOB", 0),
        ("owner_uid", "INTEGER", 0),
        ("created_at_seconds", "INTEGER", 0),
        ("created_at_nanos", "INTEGER", 0),
    ];
    verify_one_table(
        connection,
        "PRAGMA main.table_xinfo('schema_migrations')",
        "PRAGMA main.index_list('schema_migrations')",
        "PRAGMA main.index_xinfo('sqlite_autoindex_schema_migrations_1')",
        "PRAGMA main.foreign_key_list('schema_migrations')",
        "PRAGMA main.table_list('schema_migrations')",
        "schema_migrations",
        "sqlite_autoindex_schema_migrations_1",
        "migration_name",
        SCHEMA_COLUMNS,
    )?;
    verify_one_table(
        connection,
        "PRAGMA main.table_xinfo('library_meta')",
        "PRAGMA main.index_list('library_meta')",
        "PRAGMA main.index_xinfo('sqlite_autoindex_library_meta_1')",
        "PRAGMA main.foreign_key_list('library_meta')",
        "PRAGMA main.table_list('library_meta')",
        "library_meta",
        "sqlite_autoindex_library_meta_1",
        "library_id",
        META_COLUMNS,
    )
}

#[allow(clippy::too_many_arguments)]
fn verify_one_table(
    connection: &Connection,
    table_xinfo_sql: &'static str,
    index_list_sql: &'static str,
    index_xinfo_sql: &'static str,
    foreign_key_list_sql: &'static str,
    table_list_sql: &'static str,
    table_name: &'static str,
    index_name: &'static str,
    indexed_column: &'static str,
    expected_columns: &[(&str, &str, i64)],
) -> Result<(), StoreError> {
    let mut statement = connection
        .prepare(table_xinfo_sql)
        .map_err(map_reopen_error)?;
    let columns = statement
        .query_map([], |row| {
            Ok(ColumnContract {
                id: row.get(0)?,
                name: row.get(1)?,
                declared_type: row.get(2)?,
                not_null: row.get(3)?,
                default_value: row.get(4)?,
                primary_key: row.get(5)?,
                hidden: row.get(6)?,
            })
        })
        .map_err(map_reopen_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_reopen_error)?;
    let expected_columns: Vec<_> = expected_columns
        .iter()
        .enumerate()
        .map(|(id, (name, declared_type, primary_key))| ColumnContract {
            id: id as i64,
            name: (*name).to_owned(),
            declared_type: (*declared_type).to_owned(),
            not_null: 1,
            default_value: None,
            primary_key: *primary_key,
            hidden: 0,
        })
        .collect();
    if columns != expected_columns {
        return Err(StoreError::Corruption);
    }

    let mut statement = connection
        .prepare(index_list_sql)
        .map_err(map_reopen_error)?;
    let indexes = statement
        .query_map([], |row| {
            Ok(IndexListContract {
                sequence: row.get(0)?,
                name: row.get(1)?,
                unique: row.get(2)?,
                origin: row.get(3)?,
                partial: row.get(4)?,
            })
        })
        .map_err(map_reopen_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_reopen_error)?;
    if indexes
        != [IndexListContract {
            sequence: 0,
            name: index_name.to_owned(),
            unique: 1,
            origin: "u".to_owned(),
            partial: 0,
        }]
    {
        return Err(StoreError::Corruption);
    }

    let mut statement = connection
        .prepare(index_xinfo_sql)
        .map_err(map_reopen_error)?;
    let index_columns = statement
        .query_map([], |row| {
            Ok(IndexColumnContract {
                sequence: row.get(0)?,
                column_id: row.get(1)?,
                name: row.get(2)?,
                descending: row.get(3)?,
                collation: row.get(4)?,
                key: row.get(5)?,
            })
        })
        .map_err(map_reopen_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_reopen_error)?;
    if index_columns
        != [
            IndexColumnContract {
                sequence: 0,
                column_id: 1,
                name: Some(indexed_column.to_owned()),
                descending: 0,
                collation: "BINARY".to_owned(),
                key: 1,
            },
            IndexColumnContract {
                sequence: 1,
                column_id: -1,
                name: None,
                descending: 0,
                collation: "BINARY".to_owned(),
                key: 0,
            },
        ]
    {
        return Err(StoreError::Corruption);
    }

    let mut statement = connection
        .prepare(foreign_key_list_sql)
        .map_err(map_reopen_error)?;
    let mut foreign_keys = statement.query([]).map_err(map_reopen_error)?;
    if foreign_keys.next().map_err(map_reopen_error)?.is_some() {
        return Err(StoreError::Corruption);
    }

    let table_list = connection
        .query_row(table_list_sql, [], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(map_reopen_error)?;
    if table_list
        != (
            "main".to_owned(),
            table_name.to_owned(),
            "table".to_owned(),
            5,
            0,
            1,
        )
    {
        return Err(StoreError::Corruption);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    let opened = read_and_validate_rows(connection)?;
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

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use mengxia_types::{Id, Timestamp};
    use rusqlite::Connection;
    use sha2::{Digest, Sha256};

    use super::{
        ASSET_MIGRATION_SQL, LibraryIdentity, MIGRATION_SHA256,
        apply_asset_migration_with_boundaries, bootstrap_schema, migration_digest,
        prepare_current_library_schema, read_schema_identity, verify_asset_migration,
        verify_bootstrap_schema, verify_bootstrap_schema_matches, verify_current_library_schema,
        verify_quick_check, verify_reopen_library_schema,
    };
    use crate::StoreError;
    use crate::runtime::verify_and_harden;

    #[test]
    fn fresh_bootstrap_and_reopen_preserve_exact_typed_singletons() {
        let (directory, mut connection) = file_connection("bootstrap");
        verify_and_harden(&connection, Duration::from_millis(5000)).expect("harden connection");
        let library_id = Id::<LibraryIdentity>::from_bytes([
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ])
        .expect("fixed UUIDv7");
        let timestamp = Timestamp::from_unix_seconds_nanos(1_777_000_000, 123_456_789)
            .expect("fixed timestamp");

        bootstrap_schema(&mut connection, library_id, 501, timestamp).expect("bootstrap");
        drop(connection);
        let reopened = Connection::open(directory.join("library.sqlite3"))
            .expect("reopen file-backed SQLite database");
        verify_and_harden(&reopened, Duration::from_millis(5000))
            .expect("harden reopened connection");
        let opened = verify_bootstrap_schema(&reopened).expect("reopen validation");
        assert_eq!(opened.library_id, library_id);
        assert_eq!(opened.owner_uid, 501);
        assert_eq!(opened.created_at, timestamp);
        assert_eq!(migration_digest().to_bytes(), MIGRATION_SHA256);
        drop(reopened);
        fs::remove_dir_all(directory).expect("remove bootstrap directory");
    }

    #[test]
    fn asset_migration_candidate_has_exact_identity_and_parses() {
        assert_eq!(ASSET_MIGRATION_SQL.len(), 12_733);
        let observed: [u8; 32] = Sha256::digest(ASSET_MIGRATION_SQL.as_bytes()).into();
        assert_eq!(observed, super::ASSET_MIGRATION_SHA256);
        verify_asset_migration().expect("exact asset migration digest");
        let connection = Connection::open_in_memory().expect("open in-memory SQLite");
        connection
            .execute_batch(super::MIGRATION_SQL)
            .expect("bootstrap DDL parses");
        connection
            .execute_batch(ASSET_MIGRATION_SQL)
            .expect("asset DDL parses");
        connection
            .execute_batch("PRAGMA trusted_schema=OFF")
            .expect("disable trusted schema");
        connection
            .query_row("SELECT count(*) FROM library_meta", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("ordinary query remains valid under trusted-schema denial");
        let rows = read_schema_identity(&connection).expect("schema identity reads");
        assert!(rows.iter().any(|row| row.1 == "commands"));
        assert!(rows.iter().any(|row| row.1 == "domain_events_no_update"));
    }

    #[test]
    fn asset_migration_sigkill_child_entrypoint() {
        let Some(database) = std::env::var_os("MENGXIA_TASK006_MIGRATION_CRASH_DB") else {
            return;
        };
        let boundary = std::env::var("MENGXIA_TASK006_MIGRATION_CRASH_BOUNDARY")
            .expect("migration crash boundary")
            .parse::<u8>()
            .expect("numeric migration crash boundary");
        let mut connection = Connection::open(database).expect("open migration crash database");
        verify_and_harden(&connection, Duration::from_millis(5000))
            .expect("harden migration crash connection");
        apply_asset_migration_with_boundaries(&mut connection, |observed| {
            if observed == boundary {
                println!("TASK006-MIGRATION-BOUNDARY-{observed}");
                std::io::stdout()
                    .flush()
                    .expect("flush crash acknowledgement");
                loop {
                    thread::park();
                }
            }
            Ok(())
        })
        .expect("migration reaches selected crash boundary");
    }

    #[test]
    fn asset_migration_sigkill_before_and_after_commit_recovers_exactly() {
        for boundary in [5_u8, 6_u8] {
            let (directory, mut connection) =
                file_connection(&format!("task006-migration-sigkill-{boundary}"));
            verify_and_harden(&connection, Duration::from_millis(5000))
                .expect("harden migration SIGKILL fixture");
            bootstrap_schema(&mut connection, fixed_library_id(), 501, fixed_timestamp())
                .expect("create exact 0000 migration prefix");
            drop(connection);

            let mut child = Command::new(std::env::current_exe().expect("current test executable"))
                .arg("migration::tests::asset_migration_sigkill_child_entrypoint")
                .arg("--exact")
                .arg("--nocapture")
                .env(
                    "MENGXIA_TASK006_MIGRATION_CRASH_DB",
                    directory.join("library.sqlite3"),
                )
                .env(
                    "MENGXIA_TASK006_MIGRATION_CRASH_BOUNDARY",
                    boundary.to_string(),
                )
                .stdout(Stdio::piped())
                .spawn()
                .expect("spawn migration crash child");
            let stdout = child.stdout.take().expect("migration crash child stdout");
            let (sender, receiver) = mpsc::sync_channel(1);
            let expected = format!("TASK006-MIGRATION-BOUNDARY-{boundary}\n");
            let expected_reader = expected.clone();
            let reader = thread::spawn(move || {
                let mut stdout = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match stdout.read_line(&mut line) {
                        Ok(0) | Err(_) => {
                            let _ = sender.send(String::new());
                            break;
                        }
                        Ok(_) if line == expected_reader => {
                            let _ = sender.send(line);
                            break;
                        }
                        Ok(_) => {}
                    }
                }
            });
            let acknowledgement = receiver
                .recv_timeout(Duration::from_secs(30))
                .unwrap_or_else(|_| {
                    let _ = child.kill();
                    panic!("migration crash child timed out at boundary {boundary}")
                });
            assert_eq!(acknowledgement, expected);
            child.kill().expect("SIGKILL migration crash child");
            let status = child.wait().expect("wait for migration crash child");
            assert!(!status.success());
            reader
                .join()
                .expect("join migration acknowledgement reader");

            let mut reopened = Connection::open(directory.join("library.sqlite3"))
                .expect("reopen migration crash database");
            verify_and_harden(&reopened, Duration::from_millis(5000))
                .expect("recover migration WAL");
            let metadata = verify_reopen_library_schema(&reopened)
                .expect("crash leaves exact 0000 or exact committed 0001");
            prepare_current_library_schema(&mut reopened, metadata)
                .expect("rolled-back migration reapplies exactly once");
            verify_current_library_schema(&reopened).expect("current schema is exact");
            let migration_count: i64 = reopened
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .expect("count recovered migrations");
            assert_eq!(migration_count, 2);
            drop(reopened);
            fs::remove_dir_all(directory).expect("remove migration SIGKILL fixture");
        }
    }

    #[test]
    fn asset_migration_fault_boundaries_rollback_to_exact_0000() {
        for boundary in 1_u8..=5 {
            let (directory, mut connection) =
                file_connection(&format!("task006-migration-fault-{boundary}"));
            verify_and_harden(&connection, Duration::from_millis(5000))
                .expect("harden migration fault fixture");
            bootstrap_schema(&mut connection, fixed_library_id(), 501, fixed_timestamp())
                .expect("create exact 0000 fault prefix");
            assert_eq!(
                apply_asset_migration_with_boundaries(&mut connection, |observed| {
                    if observed == boundary {
                        Err(StoreError::Io)
                    } else {
                        Ok(())
                    }
                }),
                Err(StoreError::Io),
                "fault boundary {boundary}"
            );
            let metadata =
                verify_bootstrap_schema(&connection).expect("failed migration remains exact 0000");
            assert_eq!(metadata.library_id, fixed_library_id());
            let migration_count: i64 = connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .expect("count rolled-back migrations");
            assert_eq!(migration_count, 1);
            let product_table_count: i64 = connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name='assets'",
                    [],
                    |row| row.get(0),
                )
                .expect("check rolled-back product schema");
            assert_eq!(product_table_count, 0);
            drop(connection);
            fs::remove_dir_all(directory).expect("remove migration fault fixture");
        }
    }

    #[test]
    fn migration_digest_tamper_fails_reopen() {
        let (directory, mut connection) = file_connection("tamper");
        verify_and_harden(&connection, Duration::from_millis(5000)).expect("harden connection");
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

    #[test]
    fn every_extra_schema_object_fails_the_complete_allowlist() {
        for (case, sql) in [
            (
                "extra-table",
                "CREATE TABLE extra_table (value INTEGER) STRICT",
            ),
            (
                "extra-view",
                "CREATE VIEW extra_view AS SELECT singleton FROM library_meta",
            ),
            (
                "extra-trigger",
                "CREATE TRIGGER extra_trigger AFTER UPDATE ON library_meta BEGIN SELECT 1; END",
            ),
            (
                "extra-index",
                "CREATE INDEX extra_index ON library_meta(owner_uid)",
            ),
            (
                "partial-index",
                "CREATE INDEX partial_index ON library_meta(owner_uid) WHERE owner_uid > 0",
            ),
            (
                "expression-index",
                "CREATE INDEX expression_index ON library_meta(owner_uid + 1)",
            ),
            (
                "sqlite-sequence",
                "CREATE TABLE autoincrement_probe (value INTEGER PRIMARY KEY AUTOINCREMENT) STRICT",
            ),
        ] {
            let (directory, connection) = bootstrapped_connection(case);
            connection
                .execute_batch(sql)
                .expect("add forbidden schema object");
            assert_eq!(
                verify_bootstrap_schema(&connection).map(|_| ()),
                Err(StoreError::Corruption),
                "case {case}"
            );
            drop(connection);
            fs::remove_dir_all(directory).expect("remove schema allowlist fixture");
        }
    }

    #[test]
    fn missing_or_wrong_bootstrap_schema_shape_fails_reopen() {
        for (case, sql) in [
            ("missing-meta", "DROP TABLE library_meta"),
            ("missing-migrations", "DROP TABLE schema_migrations"),
            (
                "missing-autoindex",
                "DROP TABLE library_meta; CREATE TABLE library_meta (singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1), library_id BLOB NOT NULL CHECK (length(library_id) = 16), owner_uid INTEGER NOT NULL CHECK (owner_uid BETWEEN 0 AND 4294967295), created_at_seconds INTEGER NOT NULL, created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999)) STRICT",
            ),
            (
                "renamed-autoindex",
                "DROP TABLE library_meta; CREATE TABLE library_meta (singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1), library_id BLOB NOT NULL CHECK (length(library_id) = 16), owner_uid INTEGER NOT NULL CHECK (owner_uid BETWEEN 0 AND 4294967295), created_at_seconds INTEGER NOT NULL, created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999)) STRICT; CREATE UNIQUE INDEX renamed_library_id ON library_meta(library_id)",
            ),
            (
                "wrong-index-column",
                "DROP TABLE library_meta; CREATE TABLE library_meta (singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1), library_id BLOB NOT NULL CHECK (length(library_id) = 16), owner_uid INTEGER NOT NULL UNIQUE CHECK (owner_uid BETWEEN 0 AND 4294967295), created_at_seconds INTEGER NOT NULL, created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999)) STRICT",
            ),
            (
                "non-strict-meta",
                "DROP TABLE library_meta; CREATE TABLE library_meta (singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1), library_id BLOB NOT NULL UNIQUE CHECK (length(library_id) = 16), owner_uid INTEGER NOT NULL CHECK (owner_uid BETWEEN 0 AND 4294967295), created_at_seconds INTEGER NOT NULL, created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999))",
            ),
        ] {
            let (directory, connection) = bootstrapped_connection(case);
            connection.execute_batch(sql).expect("mutate schema shape");
            assert_corruption(&connection, case);
            drop(connection);
            fs::remove_dir_all(directory).expect("remove schema-shape fixture");
        }
    }

    #[test]
    fn column_shape_and_typed_singleton_tamper_fail_reopen() {
        let (directory, connection) = bootstrapped_connection("extra-column");
        connection
            .execute_batch("ALTER TABLE library_meta ADD COLUMN extra INTEGER")
            .expect("add forbidden column");
        assert_eq!(
            verify_bootstrap_schema(&connection).map(|_| ()),
            Err(StoreError::Corruption)
        );
        drop(connection);
        fs::remove_dir_all(directory).expect("remove column-shape fixture");

        for (case, sql) in [
            (
                "invalid-library-id",
                "UPDATE library_meta SET library_id = zeroblob(16)",
            ),
            (
                "timestamp-mismatch",
                "UPDATE schema_migrations SET applied_at_seconds = applied_at_seconds + 1",
            ),
        ] {
            let (directory, connection) = bootstrapped_connection(case);
            connection
                .execute_batch(sql)
                .expect("tamper typed singleton");
            assert_eq!(
                verify_bootstrap_schema(&connection).map(|_| ()),
                Err(StoreError::Corruption),
                "case {case}"
            );
            drop(connection);
            fs::remove_dir_all(directory).expect("remove typed singleton fixture");
        }
    }

    #[test]
    fn singleton_and_migration_row_matrix_fails_reopen() {
        for (case, sql) in [
            ("missing-library-row", "DELETE FROM library_meta"),
            ("missing-migration-row", "DELETE FROM schema_migrations"),
            (
                "extra-library-row",
                "PRAGMA ignore_check_constraints = ON; INSERT INTO library_meta SELECT 2, x'018d442fc0007a118022334455667789', owner_uid, created_at_seconds, created_at_nanos FROM library_meta WHERE singleton = 1",
            ),
            (
                "extra-migration-row",
                "INSERT INTO schema_migrations SELECT 1, '0001_unexpected', sha256, applied_at_seconds, applied_at_nanos FROM schema_migrations WHERE migration_sequence = 0",
            ),
            (
                "wrong-migration-sequence",
                "UPDATE schema_migrations SET migration_sequence = 1",
            ),
            (
                "wrong-migration-name",
                "UPDATE schema_migrations SET migration_name = '0000_wrong_name'",
            ),
            (
                "sequence-name-prefix-mismatch",
                "UPDATE schema_migrations SET migration_sequence = 1",
            ),
            (
                "name-sequence-prefix-mismatch",
                "UPDATE schema_migrations SET migration_name = '0001_store_bootstrap'",
            ),
            (
                "non-contiguous-migration-order",
                "INSERT INTO schema_migrations SELECT 2, '0002_unexpected', sha256, applied_at_seconds, applied_at_nanos FROM schema_migrations WHERE migration_sequence = 0",
            ),
            (
                "invalid-migration-name-grammar",
                "UPDATE schema_migrations SET migration_name = '../0000_store_bootstrap.sql'",
            ),
            (
                "invalid-library-id",
                "UPDATE library_meta SET library_id = zeroblob(16)",
            ),
            (
                "timestamp-before-year-one",
                "UPDATE library_meta SET created_at_seconds = -62135596801; UPDATE schema_migrations SET applied_at_seconds = -62135596801",
            ),
            (
                "timestamp-after-year-9999",
                "UPDATE library_meta SET created_at_seconds = 253402300800; UPDATE schema_migrations SET applied_at_seconds = 253402300800",
            ),
            (
                "invalid-meta-nanos",
                "PRAGMA ignore_check_constraints = ON; UPDATE library_meta SET created_at_nanos = 1000000000",
            ),
            (
                "invalid-migration-nanos",
                "PRAGMA ignore_check_constraints = ON; UPDATE schema_migrations SET applied_at_nanos = -1",
            ),
            (
                "timestamp-seconds-mismatch",
                "UPDATE schema_migrations SET applied_at_seconds = applied_at_seconds + 1",
            ),
            (
                "timestamp-nanos-mismatch",
                "UPDATE schema_migrations SET applied_at_nanos = applied_at_nanos + 1",
            ),
            (
                "migration-checksum-mismatch",
                "UPDATE schema_migrations SET sha256 = zeroblob(32)",
            ),
        ] {
            let (directory, connection) = bootstrapped_connection(case);
            connection
                .execute_batch(sql)
                .expect("mutate singleton rows");
            assert_corruption(&connection, case);
            drop(connection);
            fs::remove_dir_all(directory).expect("remove singleton-row fixture");
        }
    }

    #[test]
    fn duplicate_migration_sequence_or_name_fails_reopen() {
        for (case, duplicate_values) in [
            (
                "duplicate-migration-sequence",
                "0, '0001_duplicate_sequence'",
            ),
            ("duplicate-migration-name", "1, '0000_store_bootstrap'"),
        ] {
            let (directory, connection) = bootstrapped_connection(case);
            connection
                .execute_batch(&format!(
                    "DROP TABLE schema_migrations;
                     CREATE TABLE schema_migrations (
                         migration_sequence INTEGER NOT NULL,
                         migration_name TEXT NOT NULL,
                         sha256 BLOB NOT NULL,
                         applied_at_seconds INTEGER NOT NULL,
                         applied_at_nanos INTEGER NOT NULL
                     ) STRICT;
                     INSERT INTO schema_migrations VALUES
                         (0, '0000_store_bootstrap', x'35a69e30b627e994a172c9490f391552a8d60212c75ad2f478ea1005c0b94ce2', 1777000000, 123456789),
                         ({duplicate_values}, x'35a69e30b627e994a172c9490f391552a8d60212c75ad2f478ea1005c0b94ce2', 1777000000, 123456789);"
                ))
                .expect("construct duplicate migration corruption fixture");
            assert_corruption(&connection, case);
            drop(connection);
            fs::remove_dir_all(directory).expect("remove duplicate-migration fixture");
        }
    }

    #[test]
    fn expected_owner_and_intent_identity_mismatches_fail_reopen() {
        let (directory, connection) = bootstrapped_connection("expected-identity");
        let expected_id = Id::<LibraryIdentity>::from_bytes([
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ])
        .expect("fixed UUIDv7");
        let different_id = Id::<LibraryIdentity>::from_bytes([
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x89,
        ])
        .expect("different fixed UUIDv7");
        let timestamp = Timestamp::from_unix_seconds_nanos(1_777_000_000, 123_456_789)
            .expect("fixed timestamp");
        let different_timestamp = Timestamp::from_unix_seconds_nanos(1_777_000_001, 123_456_789)
            .expect("different fixed timestamp");

        for (case, id, owner_uid, created_at) in [
            ("intent-library-id", different_id, 501, timestamp),
            ("owner-uid", expected_id, 502, timestamp),
            ("intent-timestamp", expected_id, 501, different_timestamp),
        ] {
            assert_eq!(
                verify_bootstrap_schema_matches(&connection, id, owner_uid, created_at).map(|_| ()),
                Err(StoreError::Corruption),
                "case {case}"
            );
        }
        drop(connection);
        fs::remove_dir_all(directory).expect("remove expected-identity fixture");
    }

    #[test]
    fn valid_intent_timestamp_mismatch_with_either_or_both_rows_fails_reopen() {
        for (case, sql) in [
            (
                "intent-differs-from-library-meta",
                "UPDATE library_meta SET created_at_seconds = created_at_seconds + 1",
            ),
            (
                "intent-differs-from-schema-migrations",
                "UPDATE schema_migrations SET applied_at_seconds = applied_at_seconds + 1",
            ),
            (
                "intent-differs-from-both-rows",
                "UPDATE library_meta SET created_at_seconds = created_at_seconds + 1; UPDATE schema_migrations SET applied_at_seconds = applied_at_seconds + 1",
            ),
        ] {
            let (directory, connection) = bootstrapped_connection(case);
            connection
                .execute_batch(sql)
                .expect("tamper persisted bootstrap timestamp");
            assert_eq!(
                verify_bootstrap_schema_matches(
                    &connection,
                    fixed_library_id(),
                    501,
                    fixed_timestamp(),
                )
                .map(|_| ()),
                Err(StoreError::Corruption),
                "case {case}"
            );
            drop(connection);
            fs::remove_dir_all(directory).expect("remove intent-timestamp fixture");
        }
    }

    #[test]
    fn malformed_header_and_truncated_database_map_to_corruption() {
        for (case, mutation) in [("malformed-header", "header"), ("truncated", "truncate")] {
            let (directory, connection) = bootstrapped_connection(case);
            drop(connection);
            let path = directory.join("library.sqlite3");
            match mutation {
                "header" => {
                    let mut file = OpenOptions::new()
                        .write(true)
                        .open(&path)
                        .expect("open database for header mutation");
                    file.seek(SeekFrom::Start(0)).expect("seek database header");
                    file.write_all(b"not-a-sqlite-db!")
                        .expect("overwrite SQLite header");
                    file.sync_all().expect("sync malformed header");
                }
                "truncate" => OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .expect("open database for truncation")
                    .set_len(512)
                    .expect("truncate database"),
                _ => unreachable!(),
            }
            let reopened = Connection::open(&path).expect("open corrupted SQLite file handle");
            assert_corruption(&reopened, case);
            drop(reopened);
            fs::remove_dir_all(directory).expect("remove physical-corruption fixture");
        }
    }

    #[test]
    fn malformed_page_and_non_ok_quick_check_map_to_corruption() {
        let (directory, connection) = bootstrapped_connection("malformed-page");
        let (page_size, root_page): (i64, i64) = connection
            .query_row(
                "SELECT (SELECT page_size FROM pragma_page_size), rootpage FROM sqlite_schema WHERE name = 'library_meta'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read table root page");
        drop(connection);
        overwrite_byte(
            &directory.join("library.sqlite3"),
            u64::try_from((root_page - 1) * page_size).expect("root-page offset"),
            0,
        );
        let reopened = Connection::open(directory.join("library.sqlite3"))
            .expect("open malformed-page database handle");
        assert_corruption(&reopened, "malformed-page");
        drop(reopened);
        fs::remove_dir_all(directory).expect("remove malformed-page fixture");

        let (directory, connection) = bootstrapped_connection("quick-check-non-ok");
        drop(connection);
        let path = directory.join("library.sqlite3");
        write_bytes(&path, 36, &1_u32.to_be_bytes());
        let reopened = Connection::open(&path).expect("open freelist-corruption database handle");
        assert_eq!(
            verify_quick_check(&reopened),
            Err(StoreError::Corruption),
            "a non-ok PRAGMA quick_check result must fail closed"
        );
        assert_corruption(&reopened, "quick-check-non-ok");
        drop(reopened);
        fs::remove_dir_all(directory).expect("remove quick-check fixture");
    }

    #[test]
    fn bit_flipped_table_and_index_cells_map_to_corruption() {
        for (case, object_name) in [
            ("bit-flipped-table", "library_meta"),
            ("bit-flipped-index", "sqlite_autoindex_schema_migrations_1"),
        ] {
            let (directory, connection) = bootstrapped_connection(case);
            let (page_size, root_page): (i64, i64) = connection
                .query_row(
                    "SELECT (SELECT page_size FROM pragma_page_size), rootpage FROM sqlite_schema WHERE name = ?1",
                    [object_name],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("read b-tree page location");
            drop(connection);
            flip_first_btree_cell(&directory.join("library.sqlite3"), page_size, root_page);
            let reopened = Connection::open(directory.join("library.sqlite3"))
                .expect("open bit-flipped SQLite file handle");
            assert_corruption(&reopened, case);
            drop(reopened);
            fs::remove_dir_all(directory).expect("remove bit-flip fixture");
        }
    }

    #[test]
    fn injected_virtual_and_shadow_schema_entries_fail_the_allowlist() {
        for (case, name, sql) in [
            (
                "virtual-table",
                "virtual_probe",
                "CREATE VIRTUAL TABLE virtual_probe USING unavailable_module(value)",
            ),
            (
                "shadow-table",
                "virtual_probe_shadow",
                "CREATE TABLE virtual_probe_shadow(value BLOB)",
            ),
        ] {
            let (directory, connection) = bootstrapped_connection(case);
            drop(connection);
            let connection = Connection::open(directory.join("library.sqlite3"))
                .expect("open isolated schema-corruption connection");
            connection
                .execute_batch("PRAGMA writable_schema = 1")
                .expect("enable deterministic sqlite_schema mutation");
            connection
                .execute(
                    "INSERT INTO sqlite_schema(type, name, tbl_name, rootpage, sql) VALUES ('table', ?1, ?1, 0, ?2)",
                    [name, sql],
                )
                .expect("inject forbidden schema entry");
            assert_corruption(&connection, case);
            drop(connection);
            fs::remove_dir_all(directory).expect("remove injected-schema fixture");
        }
    }

    fn assert_corruption(connection: &Connection, case: &str) {
        assert_eq!(
            verify_bootstrap_schema(connection).map(|_| ()),
            Err(StoreError::Corruption),
            "case {case}"
        );
    }

    fn flip_first_btree_cell(path: &std::path::Path, page_size: i64, root_page: i64) {
        assert!(page_size >= 512);
        assert!(root_page > 1, "fixture object must not use SQLite page one");
        let page_start = u64::try_from((root_page - 1) * page_size).expect("page offset");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .expect("open SQLite b-tree page");
        file.seek(SeekFrom::Start(page_start))
            .expect("seek b-tree header");
        let mut header = [0_u8; 10];
        std::io::Read::read_exact(&mut file, &mut header).expect("read b-tree header");
        assert!(
            matches!(header[0], 0x0a | 0x0d),
            "expected leaf b-tree page"
        );
        let cell_count = u16::from_be_bytes([header[3], header[4]]);
        assert!(cell_count > 0, "fixture b-tree page must contain a cell");
        let cell_offset = usize::from(u16::from_be_bytes([header[8], header[9]]));
        let mut page = vec![0_u8; usize::try_from(page_size).expect("page size")];
        file.seek(SeekFrom::Start(page_start))
            .expect("reseek b-tree page");
        std::io::Read::read_exact(&mut file, &mut page).expect("read b-tree page");
        let (payload_bytes, payload_size_bytes) =
            parse_sqlite_varint(&page[cell_offset..]).expect("read cell payload size");
        let rowid_bytes = if header[0] == 0x0d {
            parse_sqlite_varint(&page[cell_offset + payload_size_bytes..])
                .expect("read table leaf rowid")
                .1
        } else {
            0
        };
        let payload_start = cell_offset + payload_size_bytes + rowid_bytes;
        let payload_end = payload_start + usize::try_from(payload_bytes).expect("payload length");
        assert!(
            payload_end <= page.len(),
            "cell payload must fit its leaf page"
        );
        let mutation_offset =
            page_start + u64::try_from(payload_end - 1).expect("cell byte offset");
        let mut byte = [page[payload_end - 1]];
        byte[0] ^= 0x80;
        file.seek(SeekFrom::Start(mutation_offset))
            .expect("reseek first b-tree cell");
        file.write_all(&byte).expect("flip b-tree cell bit");
        file.sync_all().expect("sync bit-flipped b-tree page");
    }

    fn overwrite_byte(path: &std::path::Path, offset: u64, value: u8) {
        write_bytes(path, offset, &[value]);
    }

    fn write_bytes(path: &std::path::Path, offset: u64, bytes: &[u8]) {
        let mut file = OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open SQLite file for deterministic corruption");
        file.seek(SeekFrom::Start(offset))
            .expect("seek deterministic corruption offset");
        file.write_all(bytes)
            .expect("write deterministic corruption bytes");
        file.sync_all()
            .expect("sync deterministic corruption bytes");
    }

    fn parse_sqlite_varint(bytes: &[u8]) -> Option<(u64, usize)> {
        let mut value = 0_u64;
        for (index, byte) in bytes.iter().copied().take(9).enumerate() {
            if index == 8 {
                return Some(((value << 8) | u64::from(byte), 9));
            }
            value = (value << 7) | u64::from(byte & 0x7f);
            if byte & 0x80 == 0 {
                return Some((value, index + 1));
            }
        }
        None
    }

    fn bootstrapped_connection(case: &str) -> (std::path::PathBuf, Connection) {
        let (directory, mut connection) = file_connection(case);
        verify_and_harden(&connection, Duration::from_millis(5000))
            .expect("harden fixture connection");
        let library_id = fixed_library_id();
        let timestamp = fixed_timestamp();
        bootstrap_schema(&mut connection, library_id, 501, timestamp).expect("bootstrap fixture");
        (directory, connection)
    }

    fn fixed_library_id() -> Id<LibraryIdentity> {
        Id::<LibraryIdentity>::from_bytes([
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88,
        ])
        .expect("fixed UUIDv7")
    }

    fn fixed_timestamp() -> Timestamp {
        Timestamp::from_unix_seconds_nanos(1_777_000_000, 123_456_789).expect("fixed timestamp")
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
