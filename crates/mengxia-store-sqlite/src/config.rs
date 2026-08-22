use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use super::StoreError;

const MIN_WRITE_QUEUE: usize = 16;
const MAX_WRITE_QUEUE: usize = 4096;
const MIN_READ_CONNECTIONS: usize = 1;
const MAX_READ_CONNECTIONS: usize = 16;
const MIN_BUSY_TIMEOUT_MS: u64 = 1;
const MAX_BUSY_TIMEOUT_MS: u64 = 5000;

/// Non-secret origin selected by the future composition resolver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ConfigSource {
    Cli,
    Environment,
    Library,
    CompiledDefault,
}

/// Already-selected typed inputs passed to TASK-004's pure validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedStoreConfig {
    library_root: Option<PathBuf>,
    library_root_source: ConfigSource,
    write_queue_capacity: usize,
    write_queue_source: ConfigSource,
    read_connection_count: usize,
    read_connection_source: ConfigSource,
    busy_timeout_ms: u64,
    busy_timeout_source: ConfigSource,
}

impl ResolvedStoreConfig {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_selected(
        library_root: Option<PathBuf>,
        library_root_source: ConfigSource,
        write_queue_capacity: usize,
        write_queue_source: ConfigSource,
        read_connection_count: usize,
        read_connection_source: ConfigSource,
        busy_timeout_ms: u64,
        busy_timeout_source: ConfigSource,
    ) -> Self {
        Self {
            library_root,
            library_root_source,
            write_queue_capacity,
            write_queue_source,
            read_connection_count,
            read_connection_source,
            busy_timeout_ms,
            busy_timeout_source,
        }
    }

    pub fn validate(self) -> Result<StoreConfig, StoreError> {
        let root = self
            .library_root
            .ok_or(StoreError::Configuration)
            .and_then(LibraryRoot::try_from)?;
        if !(MIN_WRITE_QUEUE..=MAX_WRITE_QUEUE).contains(&self.write_queue_capacity)
            || !(MIN_READ_CONNECTIONS..=MAX_READ_CONNECTIONS).contains(&self.read_connection_count)
            || !(MIN_BUSY_TIMEOUT_MS..=MAX_BUSY_TIMEOUT_MS).contains(&self.busy_timeout_ms)
        {
            return Err(StoreError::Configuration);
        }

        Ok(StoreConfig {
            library_root: root,
            library_root_source: self.library_root_source,
            write_queue_capacity: self.write_queue_capacity,
            write_queue_source: self.write_queue_source,
            read_connection_count: self.read_connection_count,
            read_connection_source: self.read_connection_source,
            busy_timeout: Duration::from_millis(self.busy_timeout_ms),
            busy_timeout_source: self.busy_timeout_source,
        })
    }
}

/// Lexically normalized absolute Library root awaiting platform authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibraryRoot(PathBuf);

impl TryFrom<PathBuf> for LibraryRoot {
    type Error = StoreError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        let text = path.to_str().ok_or(StoreError::Configuration)?;
        if text.is_empty()
            || text.as_bytes().contains(&0)
            || path == Path::new("/")
            || text
                .split('/')
                .skip(1)
                .any(|component| component.is_empty() || component == "." || component == "..")
        {
            return Err(StoreError::Configuration);
        }
        if !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(StoreError::Configuration);
        }

        let normalized: PathBuf = path.components().collect();
        if normalized != path {
            return Err(StoreError::Configuration);
        }
        Ok(Self(path))
    }
}

impl LibraryRoot {
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// Immutable, validated TASK-004 configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreConfig {
    library_root: LibraryRoot,
    library_root_source: ConfigSource,
    write_queue_capacity: usize,
    write_queue_source: ConfigSource,
    read_connection_count: usize,
    read_connection_source: ConfigSource,
    busy_timeout: Duration,
    busy_timeout_source: ConfigSource,
}

impl StoreConfig {
    #[must_use]
    pub const fn library_root(&self) -> &LibraryRoot {
        &self.library_root
    }

    #[must_use]
    pub const fn library_root_source(&self) -> ConfigSource {
        self.library_root_source
    }

    #[must_use]
    pub const fn write_queue_capacity(&self) -> usize {
        self.write_queue_capacity
    }

    #[must_use]
    pub const fn write_queue_source(&self) -> ConfigSource {
        self.write_queue_source
    }

    #[must_use]
    pub const fn read_connection_count(&self) -> usize {
        self.read_connection_count
    }

    #[must_use]
    pub const fn read_connection_source(&self) -> ConfigSource {
        self.read_connection_source
    }

    #[must_use]
    pub const fn busy_timeout(&self) -> Duration {
        self.busy_timeout
    }

    #[must_use]
    pub const fn busy_timeout_source(&self) -> ConfigSource {
        self.busy_timeout_source
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{ConfigSource, ResolvedStoreConfig};
    use crate::StoreError;

    fn resolved(
        root: Option<PathBuf>,
        write_queue: usize,
        read_connections: usize,
        busy_timeout_ms: u64,
    ) -> ResolvedStoreConfig {
        ResolvedStoreConfig::from_selected(
            root,
            ConfigSource::Cli,
            write_queue,
            ConfigSource::Environment,
            read_connections,
            ConfigSource::Library,
            busy_timeout_ms,
            ConfigSource::CompiledDefault,
        )
    }

    #[test]
    fn exact_boundaries_validate_without_source_io() {
        for (write_queue, read_connections, busy_timeout_ms) in [(16, 1, 1), (4096, 16, 5000)] {
            let config = resolved(
                Some(PathBuf::from("/Users/example/MengXiaLibrary")),
                write_queue,
                read_connections,
                busy_timeout_ms,
            )
            .validate()
            .expect("accepted boundary");
            assert_eq!(config.write_queue_capacity(), write_queue);
            assert_eq!(config.read_connection_count(), read_connections);
            assert_eq!(
                config.busy_timeout(),
                Duration::from_millis(busy_timeout_ms)
            );
        }
    }

    #[test]
    fn missing_or_unsafe_roots_fail_with_one_redacted_error() {
        for root in [
            None,
            Some(PathBuf::new()),
            Some(PathBuf::from("relative")),
            Some(PathBuf::from("/")),
            Some(PathBuf::from("/safe/../escape")),
            Some(PathBuf::from("/safe/./child")),
            Some(PathBuf::from("/safe//child")),
            Some(PathBuf::from("/safe/\0child")),
            Some(PathBuf::from(OsString::from_vec(vec![b'/', b's', 0xff]))),
        ] {
            assert_eq!(
                resolved(root, 256, 4, 5000).validate(),
                Err(StoreError::Configuration)
            );
        }
    }

    #[test]
    fn each_numeric_adjacent_out_of_range_value_fails() {
        for (write_queue, read_connections, busy_timeout_ms) in [
            (0, 4, 5000),
            (15, 4, 5000),
            (4097, 4, 5000),
            (256, 0, 5000),
            (256, 17, 5000),
            (256, 4, 0),
            (256, 4, 5001),
            (usize::MAX, usize::MAX, u64::MAX),
        ] {
            assert_eq!(
                resolved(
                    Some(PathBuf::from("/Users/example/MengXiaLibrary")),
                    write_queue,
                    read_connections,
                    busy_timeout_ms,
                )
                .validate(),
                Err(StoreError::Configuration)
            );
        }
    }
}
