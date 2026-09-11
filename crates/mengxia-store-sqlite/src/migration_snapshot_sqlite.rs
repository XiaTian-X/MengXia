use std::os::unix::ffi::OsStrExt as _;

use mengxia_platform_fs::OpenedLibraryAuthority;
use rusqlite::{Connection as SqliteSnapshot, OpenFlags};

use super::StoreError;
use super::bootstrap::close;
use super::error::{map_authority_error, map_sqlite_error};
use super::migration::{OpenedLibraryMetadata, verify_asset_prefix_schema};
use super::migration_intent::MigrationIntent;

const URI_SUFFIX: &str = "?mode=ro&immutable=1";

pub(crate) fn validate_immutable_migration_snapshot(
    authority: &OpenedLibraryAuthority,
    intent: MigrationIntent,
    expected_metadata: OpenedLibraryMetadata,
) -> Result<(), StoreError> {
    let record = intent.encode();
    let token = authority
        .migration_snapshot_path(
            intent.source_identity().2,
            intent.source_sha256().to_bytes(),
            &record,
        )
        .map_err(map_authority_error)?;
    let uri = immutable_file_uri(token.as_ref().as_os_str().as_bytes());
    // This is the TASK-009 accepted immutable-snapshot exception to the normal
    // descriptor-first mutable database opener. The authority token fixes the
    // child name and revalidates the complete namespace before and after this
    // read-only URI connection.
    let connection = SqliteSnapshot::open_with_flags(
        uri,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX
            | OpenFlags::SQLITE_OPEN_PRIVATE_CACHE
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_EXRESCODE,
    )
    .map_err(map_sqlite_error)?;
    connection
        .pragma_update(None, "query_only", true)
        .map_err(map_sqlite_error)?;
    let query_only: i64 = connection
        .pragma_query_value(None, "query_only", |row| row.get(0))
        .map_err(map_sqlite_error)?;
    if query_only != 1 || verify_asset_prefix_schema(&connection)? != expected_metadata {
        return Err(StoreError::Corruption);
    }
    close(connection)?;
    authority
        .validate_migration_snapshot_manifest(
            intent.source_identity().2,
            intent.source_sha256().to_bytes(),
            &record,
        )
        .map_err(map_authority_error)
}

fn immutable_file_uri(path: &[u8]) -> String {
    let mut uri = String::with_capacity(path.len().saturating_mul(3).saturating_add(32));
    uri.push_str("file:");
    for &byte in path {
        if byte == b'/' || byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
        {
            uri.push(char::from(byte));
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            uri.push('%');
            uri.push(char::from(HEX[usize::from(byte >> 4)]));
            uri.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    uri.push_str(URI_SUFFIX);
    uri
}

#[cfg(test)]
mod tests {
    use super::immutable_file_uri;

    #[test]
    fn immutable_uri_has_fixed_query_and_percent_encoding() {
        assert_eq!(
            immutable_file_uri(b"/tmp/a b?c#d%"),
            "file:/tmp/a%20b%3Fc%23d%25?mode=ro&immutable=1"
        );
    }
}
