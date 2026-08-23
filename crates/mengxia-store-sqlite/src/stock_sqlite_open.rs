use mengxia_platform_fs::{FixedSqliteChildPath, SqliteChild, ValidatedAbsolutePath};
use rusqlite::{Connection, OpenFlags};

use super::StoreError;

#[derive(Clone, Copy)]
pub(crate) enum ConnectionAccess {
    ReadOnly,
    ReadWrite,
}

/// The sole production consumer allowed to borrow a fixed SQLite child path.
/// Every stock SQLite open is enclosed by whole-prefix edge revalidation.
pub(crate) fn open(
    authority: &ValidatedAbsolutePath,
    child: SqliteChild,
    access: ConnectionAccess,
) -> Result<Connection, StoreError> {
    authority
        .revalidate_chain()
        .map_err(|_| StoreError::Configuration)?;
    authority
        .validate_sqlite_child(child)
        .map_err(|_| StoreError::Configuration)?;
    let token = authority.sqlite_child(child);
    let connection = open_with_flags(&token, access).map_err(map_open_error)?;
    if authority.revalidate_chain().is_err() || authority.validate_sqlite_child(child).is_err() {
        drop(connection);
        return Err(StoreError::Configuration);
    }
    Ok(connection)
}

fn open_with_flags(
    token: &FixedSqliteChildPath<'_>,
    access: ConnectionAccess,
) -> rusqlite::Result<Connection> {
    let access_flag = match access {
        ConnectionAccess::ReadOnly => OpenFlags::SQLITE_OPEN_READ_ONLY,
        ConnectionAccess::ReadWrite => OpenFlags::SQLITE_OPEN_READ_WRITE,
    };
    Connection::open_with_flags(
        token,
        access_flag
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_EXRESCODE,
    )
}

fn map_open_error(error: rusqlite::Error) -> StoreError {
    use rusqlite::ErrorCode;

    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy) => StoreError::Busy,
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => StoreError::Corruption,
        Some(
            ErrorCode::CannotOpen
            | ErrorCode::SystemIoFailure
            | ErrorCode::DiskFull
            | ErrorCode::ReadOnly
            | ErrorCode::PermissionDenied
            | ErrorCode::OutOfMemory,
        ) => StoreError::Io,
        Some(ErrorCode::DatabaseLocked) | None => StoreError::Internal,
        Some(_) => StoreError::Internal,
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use mengxia_platform_fs::{SqliteChild, ValidatedAbsolutePath};

    use super::{ConnectionAccess, open};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|path| path.parent())
                .expect("crate is inside workspace")
                .to_path_buf();
            let common = repository.join("target/task-004-store-path-tests");
            fs::create_dir_all(&common).expect("create fixture parent");
            fs::set_permissions(&common, fs::Permissions::from_mode(0o700))
                .expect("secure fixture parent");
            let root = common.join(format!(
                "{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&root)
                .expect("create Library root");
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn fixed_existing_staging_file_opens_between_two_chain_proofs() {
        let fixture = Fixture::new();
        let staging = fixture.0.join(".library.sqlite3.bootstrap");
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staging)
            .expect("create fixed staging file");
        let authority =
            ValidatedAbsolutePath::authorize_existing(&fixture.0).expect("safe authority");
        let connection = open(
            &authority,
            SqliteChild::BootstrapStaging,
            ConnectionAccess::ReadWrite,
        )
        .expect("fixed SQLite open");
        let value: i64 = connection
            .query_row("SELECT 1", [], |row| row.get(0))
            .expect("observable SQLite query");
        assert_eq!(value, 1);
    }
}
