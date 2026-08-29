//! SQLite persistence adapter boundary for MengXia.
//!
//! Runtime integration of the SQLite 3.53.4 pin is owned by TASK-004.

#![forbid(unsafe_code)]

mod asset_repository;
#[allow(dead_code)]
mod bootstrap;
mod config;
mod error;
#[allow(dead_code)]
mod intent;
#[allow(dead_code)]
mod lifecycle;
#[allow(dead_code)]
mod migration;
#[allow(dead_code)]
mod path_authority;
#[allow(dead_code)]
mod runtime;
#[allow(dead_code)]
mod stock_sqlite_open;
mod wal;

pub use asset_repository::SqliteAssetStoreHandle;
pub use config::{ConfigSource, LibraryRoot, ResolvedStoreConfig, StoreConfig};
pub use error::StoreError;
use mengxia_platform_fs::{BlobRootRequest, OpenedBlobRootAuthority};

/// Opaque opened Library owner retained by the daemon composition root.
pub struct OpenedLibrary {
    owner: lifecycle::OpenedLibraryOwner,
}

/// Copyable non-secret identity view used to bind local IPC authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenedLibraryIdentity {
    library_id: [u8; 16],
    owner_uid: u32,
}

impl OpenedLibrary {
    /// Opens, recovers or bootstraps exactly one Library and retains its lock.
    pub fn open_or_bootstrap(config: &StoreConfig) -> Result<Self, StoreError> {
        bootstrap::open_or_bootstrap_library(config).map(|owner| Self { owner })
    }

    /// Returns the narrow identity required by TASK-003 IPC composition.
    #[must_use]
    pub fn identity(&self) -> OpenedLibraryIdentity {
        let metadata = self.owner.metadata();
        OpenedLibraryIdentity {
            library_id: metadata.library_id.to_bytes(),
            owner_uid: metadata.owner_uid,
        }
    }

    /// Mints the opaque TASK-005 Blob-root authority while Library ownership lives.
    pub fn authorize_blob_root(
        &self,
        request: &BlobRootRequest,
    ) -> Result<OpenedBlobRootAuthority, StoreError> {
        self.owner.authorize_blob_root(request)
    }

    /// Returns the opaque TASK-006 persistence capability.
    #[must_use]
    pub fn asset_store_handle(&self) -> SqliteAssetStoreHandle {
        SqliteAssetStoreHandle::new(self.owner.handle())
    }

    /// Joins all store workers, validates/closes SQLite and releases the lock last.
    pub fn shutdown(self) -> Result<(), StoreError> {
        self.owner.shutdown()
    }
}

impl OpenedLibraryIdentity {
    /// Durable Library owner UID used for local peer authentication.
    #[must_use]
    pub const fn owner_uid(self) -> u32 {
        self.owner_uid
    }

    /// Durable Library UUID bytes used only to bind the runtime namespace marker.
    #[must_use]
    pub const fn library_id_bytes(self) -> [u8; 16] {
        self.library_id
    }
}

#[cfg(test)]
mod opened_library_tests {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{ConfigSource, OpenedLibrary, ResolvedStoreConfig, StoreError};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .unwrap()
                .join("target/task-003-opened-library-tests")
                .join(format!(
                    "{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&root)
                .unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            Self { root }
        }

        fn config(&self) -> super::StoreConfig {
            ResolvedStoreConfig::from_selected(
                Some(self.root.join("Library")),
                ConfigSource::Cli,
                16,
                ConfigSource::CompiledDefault,
                1,
                ConfigSource::CompiledDefault,
                100,
                ConfigSource::CompiledDefault,
            )
            .validate()
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn opaque_owner_retains_lock_and_reopens_the_same_identity() {
        let fixture = Fixture::new();
        let config = fixture.config();
        let opened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
        let identity = opened.identity();
        assert_eq!(
            OpenedLibrary::open_or_bootstrap(&config).err(),
            Some(StoreError::Conflict)
        );
        opened.shutdown().unwrap();

        let reopened = OpenedLibrary::open_or_bootstrap(&config).unwrap();
        assert_eq!(reopened.identity(), identity);
        reopened.shutdown().unwrap();
    }
}
