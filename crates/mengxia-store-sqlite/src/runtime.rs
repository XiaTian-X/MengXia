use std::collections::BTreeSet;

use rusqlite::Connection;
use rusqlite::config::DbConfig;

use super::StoreError;

const SQLITE_VERSION_NUMBER: i32 = 3_053_004;
const SQLITE_SOURCE_ID: &str =
    "2026-07-24 19:02:57 bf7c7f30031888f4e796e429ab3978879485813aaca6f641c7b33e4e09459bcc";

const REQUIRED_OPTIONS: &[&str] = &[
    "THREADSAFE=1",
    "DQS=0",
    "DEFAULT_FOREIGN_KEYS",
    "DEFAULT_WAL_SYNCHRONOUS=2",
    "OMIT_LOAD_EXTENSION",
];

const FORBIDDEN_OPTIONS: &[&str] = &[
    "ENABLE_COLUMN_METADATA",
    "ENABLE_DBSTAT_VTAB",
    "ENABLE_FTS1",
    "ENABLE_FTS2",
    "ENABLE_FTS3",
    "ENABLE_FTS4",
    "ENABLE_FTS5",
    "ENABLE_LOAD_EXTENSION",
    "ENABLE_RTREE",
    "ENABLE_STAT4",
    "USE_URI",
];

pub(crate) fn verify_and_harden(connection: &Connection) -> Result<(), StoreError> {
    if rusqlite::version_number() != SQLITE_VERSION_NUMBER {
        return Err(StoreError::Configuration);
    }
    let source_id: String = connection
        .query_row("SELECT sqlite_source_id()", [], |row| row.get(0))
        .map_err(|_| StoreError::Internal)?;
    if source_id != SQLITE_SOURCE_ID {
        return Err(StoreError::Configuration);
    }

    let options = compile_options(connection)?;
    if REQUIRED_OPTIONS
        .iter()
        .any(|required| !options.contains(*required))
        || FORBIDDEN_OPTIONS
            .iter()
            .any(|forbidden| options.contains(*forbidden))
    {
        return Err(StoreError::Configuration);
    }
    verify_complete_option_allowlist(&options)?;

    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(|_| StoreError::Internal)?;
    let journal_mode: String = connection
        .pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))
        .map_err(|_| StoreError::Internal)?;
    if journal_mode != "wal" {
        return Err(StoreError::Configuration);
    }
    connection
        .pragma_update(None, "synchronous", "FULL")
        .map_err(|_| StoreError::Internal)?;
    connection
        .pragma_update(None, "trusted_schema", false)
        .map_err(|_| StoreError::Internal)?;
    let defensive = connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
        .map_err(|_| StoreError::Internal)?;
    let trusted_schema = connection
        .set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)
        .map_err(|_| StoreError::Internal)?;
    if !defensive || trusted_schema {
        return Err(StoreError::Configuration);
    }

    verify_pragma_i64(connection, "foreign_keys", 1)?;
    verify_pragma_i64(connection, "synchronous", 2)?;
    verify_pragma_i64(connection, "trusted_schema", 0)?;
    Ok(())
}

fn compile_options(connection: &Connection) -> Result<BTreeSet<String>, StoreError> {
    let mut statement = connection
        .prepare("PRAGMA compile_options")
        .map_err(|_| StoreError::Internal)?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|_| StoreError::Internal)?;
    rows.collect::<Result<BTreeSet<_>, _>>()
        .map_err(|_| StoreError::Internal)
}

fn verify_complete_option_allowlist(options: &BTreeSet<String>) -> Result<(), StoreError> {
    let mut non_diagnostic = options.clone();
    let compiler_entries: Vec<_> = non_diagnostic
        .iter()
        .filter(|option| option.starts_with("COMPILER="))
        .cloned()
        .collect();
    if compiler_entries.len() != 1 {
        return Err(StoreError::Configuration);
    }
    non_diagnostic.remove(&compiler_entries[0]);

    let accepted: BTreeSet<_> = include_str!("../sqlite-compile-options-allowlist.txt")
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    if non_diagnostic == accepted {
        Ok(())
    } else {
        Err(StoreError::Configuration)
    }
}

fn verify_pragma_i64(connection: &Connection, name: &str, expected: i64) -> Result<(), StoreError> {
    let value: i64 = connection
        .pragma_query_value(None, name, |row| row.get(0))
        .map_err(|_| StoreError::Internal)?;
    if value == expected {
        Ok(())
    } else {
        Err(StoreError::Configuration)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rusqlite::Connection;

    use super::{SQLITE_SOURCE_ID, SQLITE_VERSION_NUMBER, compile_options, verify_and_harden};

    #[test]
    fn source_pinned_runtime_identity_and_hardening_pass() {
        let directory = tempfile_directory();
        let database = directory.join("runtime.sqlite3");
        let connection = Connection::open(&database).expect("open temporary SQLite");
        verify_and_harden(&connection).expect("accepted runtime and hardening");
        assert_eq!(rusqlite::version_number(), SQLITE_VERSION_NUMBER);
        let source_id: String = connection
            .query_row("SELECT sqlite_source_id()", [], |row| row.get(0))
            .expect("source ID");
        assert_eq!(source_id, SQLITE_SOURCE_ID);
        let options = compile_options(&connection).expect("compile options");
        assert!(!options.is_empty());
        drop(connection);
        fs::remove_dir_all(directory).expect("remove temporary SQLite directory");
    }

    fn tempfile_directory() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "mengxia-task004-runtime-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        fs::create_dir(&directory).expect("create unique temporary SQLite directory");
        directory
    }
}
