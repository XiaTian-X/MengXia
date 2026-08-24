use mengxia_platform_fs::{
    BootstrapFilesystemState, OpenedLibraryAuthority, ValidatedAbsolutePath,
};

use super::error::map_authority_error;
use super::intent::BootstrapIntent;
use super::{StoreConfig, StoreError};

pub(crate) enum OpenedBootstrapState {
    LockOnly(OpenedLibraryAuthority),
    ValidIntent {
        authority: OpenedLibraryAuthority,
        intent: BootstrapIntent,
    },
    ValidIntentWithStaging {
        authority: OpenedLibraryAuthority,
        intent: BootstrapIntent,
    },
    ValidIntentWithPublishedStaging {
        authority: OpenedLibraryAuthority,
        intent: BootstrapIntent,
    },
    ValidIntentWithCanonical {
        authority: OpenedLibraryAuthority,
        intent: BootstrapIntent,
    },
    CanonicalOnly(OpenedLibraryAuthority),
}

/// Converts the source-free store DTO's lexical root into retained platform
/// authority. Platform diagnostics are deliberately collapsed to the one safe,
/// static configuration error at this pre-mutation boundary.
pub(crate) fn authorize_existing_root(
    config: &StoreConfig,
) -> Result<ValidatedAbsolutePath, StoreError> {
    ValidatedAbsolutePath::authorize_existing(config.library_root().as_path())
        .map_err(map_authority_error)
}

/// Performs the read-only whole-prefix proof required before fresh-bootstrap
/// clock and identity sampling. The final Library root is deliberately neither
/// opened nor created by this preflight.
pub(crate) fn authorize_bootstrap_parent(config: &StoreConfig) -> Result<(), StoreError> {
    ValidatedAbsolutePath::authorize_bootstrap_parent(config.library_root().as_path())
        .map_err(map_authority_error)
}

/// Creates or opens the bootstrap root and retains its exclusive durable lock.
pub(crate) fn acquire_bootstrap_authority(
    config: &StoreConfig,
) -> Result<OpenedLibraryAuthority, StoreError> {
    OpenedLibraryAuthority::acquire_bootstrap(config.library_root().as_path())
        .map_err(map_authority_error)
}

pub(crate) fn acquire_bootstrap_state(
    config: &StoreConfig,
) -> Result<OpenedBootstrapState, StoreError> {
    let (authority, state) =
        OpenedLibraryAuthority::acquire_bootstrap_state(config.library_root().as_path())
            .map_err(map_authority_error)?;
    match state {
        BootstrapFilesystemState::LockOnly => Ok(OpenedBootstrapState::LockOnly(authority)),
        BootstrapFilesystemState::CanonicalOnly => {
            Ok(OpenedBootstrapState::CanonicalOnly(authority))
        }
        BootstrapFilesystemState::IntentOnly(record) => {
            let intent = decode_intent(&authority, &record)?;
            Ok(OpenedBootstrapState::ValidIntent { authority, intent })
        }
        BootstrapFilesystemState::IntentWithStaging(record) => {
            let intent = decode_intent(&authority, &record)?;
            Ok(OpenedBootstrapState::ValidIntentWithStaging { authority, intent })
        }
        BootstrapFilesystemState::IntentWithPublishedStaging(record) => {
            let intent = decode_intent(&authority, &record)?;
            Ok(OpenedBootstrapState::ValidIntentWithPublishedStaging { authority, intent })
        }
        BootstrapFilesystemState::IntentWithCanonical(record) => {
            let intent = decode_intent(&authority, &record)?;
            Ok(OpenedBootstrapState::ValidIntentWithCanonical { authority, intent })
        }
        _ => Err(StoreError::Internal),
    }
}

fn decode_intent(
    authority: &OpenedLibraryAuthority,
    record: &[u8],
) -> Result<BootstrapIntent, StoreError> {
    let intent = BootstrapIntent::decode(record)?;
    intent.verify_authority(authority)?;
    Ok(intent)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        OpenedBootstrapState, acquire_bootstrap_authority, acquire_bootstrap_state,
        authorize_existing_root,
    };
    use crate::intent::BootstrapIntent;
    use crate::migration::LibraryIdentity;
    use crate::{ConfigSource, ResolvedStoreConfig, StoreError};
    use mengxia_types::{Id, Timestamp};

    #[test]
    fn platform_failure_is_redacted_to_configuration_error() {
        let config = ResolvedStoreConfig::from_selected(
            Some(PathBuf::from("/definitely/not/a/mengxia/library")),
            ConfigSource::Cli,
            256,
            ConfigSource::CompiledDefault,
            4,
            ConfigSource::CompiledDefault,
            5000,
            ConfigSource::CompiledDefault,
        )
        .validate()
        .expect("lexically valid DTO");
        assert!(matches!(
            authorize_existing_root(&config),
            Err(StoreError::Configuration)
        ));
    }

    #[test]
    fn absent_root_acquires_bootstrap_authority_without_exposing_the_lock() {
        use std::fs;
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repository = manifest
            .parent()
            .and_then(|path| path.parent())
            .expect("crate is inside workspace");
        let parent = repository.join(format!("target/task-004-store-lock-{}", std::process::id()));
        if parent.exists() {
            fs::remove_dir_all(&parent).expect("remove stale store lock fixture");
        }
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&parent)
            .expect("create secure fixture parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure fixture parent");
        let library = parent.join("Library");
        let config = ResolvedStoreConfig::from_selected(
            Some(library.clone()),
            ConfigSource::Cli,
            256,
            ConfigSource::CompiledDefault,
            4,
            ConfigSource::CompiledDefault,
            5000,
            ConfigSource::CompiledDefault,
        )
        .validate()
        .expect("valid store configuration");
        let authority = acquire_bootstrap_authority(&config).expect("acquire bootstrap authority");
        assert!(authority.authorizes_library_root(&library));
        drop(authority);
        fs::remove_dir_all(parent).expect("remove store lock fixture");
    }

    #[test]
    fn post_lock_state_is_typed_and_corruption_is_preserved() {
        use std::fs;
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repository = manifest
            .parent()
            .and_then(|path| path.parent())
            .expect("crate is inside workspace");
        let parent = repository.join(format!(
            "target/task-004-store-reopen-{}",
            std::process::id()
        ));
        if parent.exists() {
            fs::remove_dir_all(&parent).expect("remove stale reopen fixture");
        }
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&parent)
            .expect("create secure reopen fixture parent");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))
            .expect("secure reopen fixture parent");
        let library = parent.join("Library");
        let config = ResolvedStoreConfig::from_selected(
            Some(library.clone()),
            ConfigSource::Cli,
            256,
            ConfigSource::CompiledDefault,
            4,
            ConfigSource::CompiledDefault,
            5000,
            ConfigSource::CompiledDefault,
        )
        .validate()
        .expect("valid store configuration");
        let library_id = Id::<LibraryIdentity>::from_bytes([
            0x01, 0x89, 0x0f, 0x1d, 0xe0, 0x00, 0x70, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01,
        ])
        .expect("fixed UUIDv7");
        let created_at = Timestamp::from_unix_seconds_nanos(1_700_000_000, 123_456_789)
            .expect("fixed timestamp");

        let authority = acquire_bootstrap_authority(&config).expect("acquire initial authority");
        let expected = BootstrapIntent::create_durable(&authority, library_id, created_at)
            .expect("persist typed intent");
        drop(authority);

        let reopened = acquire_bootstrap_state(&config).expect("reopen valid intent state");
        match reopened {
            OpenedBootstrapState::ValidIntent { authority, intent } => {
                assert_eq!(intent, expected);
                assert_eq!(intent.library_id(), library_id);
                assert_eq!(intent.created_at(), created_at);
                drop(authority);
            }
            OpenedBootstrapState::LockOnly(_) => panic!("intent state was lost"),
            _ => panic!("unexpected post-lock state"),
        }

        let intent_path = library.join(".mengxia.bootstrap-intent");
        let mut corrupt = fs::read(&intent_path).expect("read intent for corruption fixture");
        corrupt[224] ^= 1;
        fs::write(&intent_path, &corrupt).expect("write checksum corruption fixture");
        assert!(matches!(
            acquire_bootstrap_state(&config),
            Err(StoreError::Configuration)
        ));
        assert_eq!(
            fs::read(&intent_path).expect("corrupt intent remains for inspection"),
            corrupt
        );
        assert!(!library.join(".library.sqlite3.bootstrap").exists());
        assert!(!library.join("library.sqlite3").exists());
        fs::remove_dir_all(parent).expect("remove reopen fixture");
    }
}
