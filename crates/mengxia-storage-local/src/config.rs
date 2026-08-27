use std::path::{Component, Path, PathBuf};

use mengxia_platform_fs::BlobRootRequest;
use mengxia_ports::BlobStorageError;

const MAX_LIBRARY_ROOT_BYTES_FOR_DEFAULT: usize = 929;
const MAX_BLOB_ROOT_BYTES: usize = 937;
const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const TIB: u64 = 1024 * GIB;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BlobConfigSource {
    Cli,
    Environment,
    Library,
    CompiledDefault,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBlobStorageConfig {
    library_root: Option<PathBuf>,
    library_root_source: BlobConfigSource,
    blob_root: Option<PathBuf>,
    blob_root_source: BlobConfigSource,
    storage_io_concurrency: Option<String>,
    storage_io_concurrency_source: BlobConfigSource,
    hash_concurrency: Option<String>,
    hash_concurrency_source: BlobConfigSource,
    max_concurrent_ingests: Option<String>,
    max_concurrent_ingests_source: BlobConfigSource,
    stream_buffer_bytes: Option<String>,
    stream_buffer_bytes_source: BlobConfigSource,
    max_ingest_bytes: Option<String>,
    max_ingest_bytes_source: BlobConfigSource,
    max_staging_bytes: Option<String>,
    max_staging_bytes_source: BlobConfigSource,
    min_free_bytes: Option<String>,
    min_free_bytes_source: BlobConfigSource,
    min_free_percent: Option<String>,
    min_free_percent_source: BlobConfigSource,
}

impl ResolvedBlobStorageConfig {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn from_selected(
        library_root: Option<PathBuf>,
        library_root_source: BlobConfigSource,
        blob_root: Option<PathBuf>,
        blob_root_source: BlobConfigSource,
        storage_io_concurrency: Option<String>,
        storage_io_concurrency_source: BlobConfigSource,
        hash_concurrency: Option<String>,
        hash_concurrency_source: BlobConfigSource,
        max_concurrent_ingests: Option<String>,
        max_concurrent_ingests_source: BlobConfigSource,
        stream_buffer_bytes: Option<String>,
        stream_buffer_bytes_source: BlobConfigSource,
        max_ingest_bytes: Option<String>,
        max_ingest_bytes_source: BlobConfigSource,
        max_staging_bytes: Option<String>,
        max_staging_bytes_source: BlobConfigSource,
        min_free_bytes: Option<String>,
        min_free_bytes_source: BlobConfigSource,
        min_free_percent: Option<String>,
        min_free_percent_source: BlobConfigSource,
    ) -> Self {
        Self {
            library_root,
            library_root_source,
            blob_root,
            blob_root_source,
            storage_io_concurrency,
            storage_io_concurrency_source,
            hash_concurrency,
            hash_concurrency_source,
            max_concurrent_ingests,
            max_concurrent_ingests_source,
            stream_buffer_bytes,
            stream_buffer_bytes_source,
            max_ingest_bytes,
            max_ingest_bytes_source,
            max_staging_bytes,
            max_staging_bytes_source,
            min_free_bytes,
            min_free_bytes_source,
            min_free_percent,
            min_free_percent_source,
        }
    }

    pub fn validate(self) -> Result<BlobStorageConfig, BlobStorageError> {
        let storage_io_concurrency = parse_usize(self.storage_io_concurrency.as_deref())?;
        let hash_concurrency = parse_usize(self.hash_concurrency.as_deref())?;
        let max_concurrent_ingests = parse_usize(self.max_concurrent_ingests.as_deref())?;
        let stream_buffer_bytes = parse_usize(self.stream_buffer_bytes.as_deref())?;
        let max_ingest_bytes = parse_u64(self.max_ingest_bytes.as_deref())?;
        let max_staging_bytes = parse_u64(self.max_staging_bytes.as_deref())?;
        let min_free_bytes = parse_u64(self.min_free_bytes.as_deref())?;
        let min_free_percent = u8::try_from(parse_u64(self.min_free_percent.as_deref())?)
            .map_err(|_| BlobStorageError::Configuration)?;
        let library_root = validate_unicode_absolute(self.library_root.as_deref(), 1023)?;
        let blob_root = validate_unicode_absolute(self.blob_root.as_deref(), MAX_BLOB_ROOT_BYTES)?;
        let default_root = library_root.join("storage");
        let is_default = blob_root == default_root;
        if is_default {
            if library_root.as_os_str().as_encoded_bytes().len()
                > MAX_LIBRARY_ROOT_BYTES_FOR_DEFAULT
            {
                return Err(BlobStorageError::Configuration);
            }
        } else if paths_overlap(&library_root, &blob_root) {
            return Err(BlobStorageError::Configuration);
        }
        if !(1..=8).contains(&storage_io_concurrency)
            || !(1..=8).contains(&hash_concurrency)
            || !(1..=8).contains(&max_concurrent_ingests)
            || !(MIB as usize..=32 * MIB as usize).contains(&stream_buffer_bytes)
            || !(1..=TIB).contains(&max_ingest_bytes)
            || !(1..=2 * TIB).contains(&max_staging_bytes)
            || max_staging_bytes < max_ingest_bytes
            || min_free_bytes < 10 * GIB
            || !(5..=100).contains(&min_free_percent)
        {
            return Err(BlobStorageError::Configuration);
        }
        let request = BlobRootRequest::from_absolute_path(&blob_root)
            .map_err(|_| BlobStorageError::Configuration)?;
        Ok(BlobStorageConfig {
            request,
            library_root_source: self.library_root_source,
            blob_root_source: self.blob_root_source,
            storage_io_concurrency,
            storage_io_concurrency_source: self.storage_io_concurrency_source,
            hash_concurrency,
            hash_concurrency_source: self.hash_concurrency_source,
            max_concurrent_ingests,
            max_concurrent_ingests_source: self.max_concurrent_ingests_source,
            stream_buffer_bytes,
            stream_buffer_bytes_source: self.stream_buffer_bytes_source,
            max_ingest_bytes,
            max_ingest_bytes_source: self.max_ingest_bytes_source,
            max_staging_bytes,
            max_staging_bytes_source: self.max_staging_bytes_source,
            min_free_bytes,
            min_free_bytes_source: self.min_free_bytes_source,
            min_free_percent,
            min_free_percent_source: self.min_free_percent_source,
        })
    }
}

pub struct BlobStorageConfig {
    request: BlobRootRequest,
    library_root_source: BlobConfigSource,
    blob_root_source: BlobConfigSource,
    storage_io_concurrency: usize,
    storage_io_concurrency_source: BlobConfigSource,
    hash_concurrency: usize,
    hash_concurrency_source: BlobConfigSource,
    max_concurrent_ingests: usize,
    max_concurrent_ingests_source: BlobConfigSource,
    stream_buffer_bytes: usize,
    stream_buffer_bytes_source: BlobConfigSource,
    max_ingest_bytes: u64,
    max_ingest_bytes_source: BlobConfigSource,
    max_staging_bytes: u64,
    max_staging_bytes_source: BlobConfigSource,
    min_free_bytes: u64,
    min_free_bytes_source: BlobConfigSource,
    min_free_percent: u8,
    min_free_percent_source: BlobConfigSource,
}

impl BlobStorageConfig {
    #[must_use]
    pub const fn blob_root_request(&self) -> &BlobRootRequest {
        &self.request
    }
    #[must_use]
    pub const fn storage_io_concurrency(&self) -> usize {
        self.storage_io_concurrency
    }
    #[must_use]
    pub const fn hash_concurrency(&self) -> usize {
        self.hash_concurrency
    }
    #[must_use]
    pub const fn max_concurrent_ingests(&self) -> usize {
        self.max_concurrent_ingests
    }
    #[must_use]
    pub const fn stream_buffer_bytes(&self) -> usize {
        self.stream_buffer_bytes
    }
    #[must_use]
    pub const fn max_ingest_bytes(&self) -> u64 {
        self.max_ingest_bytes
    }
    #[must_use]
    pub const fn max_staging_bytes(&self) -> u64 {
        self.max_staging_bytes
    }
    #[must_use]
    pub const fn min_free_bytes(&self) -> u64 {
        self.min_free_bytes
    }
    #[must_use]
    pub const fn min_free_percent(&self) -> u8 {
        self.min_free_percent
    }
    #[must_use]
    pub const fn library_root_source(&self) -> BlobConfigSource {
        self.library_root_source
    }
    #[must_use]
    pub const fn blob_root_source(&self) -> BlobConfigSource {
        self.blob_root_source
    }
    #[must_use]
    pub const fn storage_io_concurrency_source(&self) -> BlobConfigSource {
        self.storage_io_concurrency_source
    }
    #[must_use]
    pub const fn hash_concurrency_source(&self) -> BlobConfigSource {
        self.hash_concurrency_source
    }
    #[must_use]
    pub const fn max_concurrent_ingests_source(&self) -> BlobConfigSource {
        self.max_concurrent_ingests_source
    }
    #[must_use]
    pub const fn stream_buffer_bytes_source(&self) -> BlobConfigSource {
        self.stream_buffer_bytes_source
    }
    #[must_use]
    pub const fn max_ingest_bytes_source(&self) -> BlobConfigSource {
        self.max_ingest_bytes_source
    }
    #[must_use]
    pub const fn max_staging_bytes_source(&self) -> BlobConfigSource {
        self.max_staging_bytes_source
    }
    #[must_use]
    pub const fn min_free_bytes_source(&self) -> BlobConfigSource {
        self.min_free_bytes_source
    }
    #[must_use]
    pub const fn min_free_percent_source(&self) -> BlobConfigSource {
        self.min_free_percent_source
    }
}

fn parse_u64(value: Option<&str>) -> Result<u64, BlobStorageError> {
    let value = value.ok_or(BlobStorageError::Configuration)?;
    if value.is_empty() || !value.as_bytes().iter().all(u8::is_ascii_digit) {
        return Err(BlobStorageError::Configuration);
    }
    value
        .parse::<u64>()
        .map_err(|_| BlobStorageError::Configuration)
}

fn parse_usize(value: Option<&str>) -> Result<usize, BlobStorageError> {
    usize::try_from(parse_u64(value)?).map_err(|_| BlobStorageError::Configuration)
}

fn validate_unicode_absolute(
    path: Option<&Path>,
    limit: usize,
) -> Result<PathBuf, BlobStorageError> {
    let path = path.ok_or(BlobStorageError::Configuration)?;
    let text = path.to_str().ok_or(BlobStorageError::Configuration)?;
    if path == Path::new("/")
        || text.is_empty()
        || text.as_bytes().contains(&0)
        || text.len() > limit
        || !path.is_absolute() || path.components().any(|component| {
        matches!(component, Component::CurDir | Component::ParentDir)
            || matches!(component, Component::Normal(name) if name.as_encoded_bytes().len() > 255)
    })
        || text
            .split('/')
            .skip(1)
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(BlobStorageError::Configuration);
    }
    let normalized: PathBuf = path.components().collect();
    if normalized != path {
        return Err(BlobStorageError::Configuration);
    }
    Ok(path.to_path_buf())
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    use mengxia_ports::BlobStorageError;

    use super::{BlobConfigSource, ResolvedBlobStorageConfig};

    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    fn raw(value: impl ToString) -> Option<String> {
        Some(value.to_string())
    }

    fn resolved(library: Option<PathBuf>, blob: Option<PathBuf>) -> ResolvedBlobStorageConfig {
        let source = BlobConfigSource::CompiledDefault;
        ResolvedBlobStorageConfig::from_selected(
            library,
            BlobConfigSource::Cli,
            blob,
            BlobConfigSource::Environment,
            raw(2),
            source,
            raw(2),
            source,
            raw(2),
            source,
            raw(8 * MIB as usize),
            source,
            raw(TIB),
            source,
            raw(2 * TIB),
            source,
            raw(10 * GIB),
            source,
            raw(5),
            source,
        )
    }

    #[test]
    fn exact_default_and_custom_disjoint_roots_validate_without_io() {
        let default = resolved(
            Some(PathBuf::from("/Users/example/Library")),
            Some(PathBuf::from("/Users/example/Library/storage")),
        )
        .validate()
        .unwrap();
        assert_eq!(default.storage_io_concurrency(), 2);
        assert_eq!(default.max_ingest_bytes(), TIB);

        resolved(
            Some(PathBuf::from("/Users/example/Library")),
            Some(PathBuf::from("/Volumes/MengXiaStorage")),
        )
        .validate()
        .unwrap();
    }

    #[test]
    fn overlapping_missing_nonunicode_and_noncanonical_roots_fail_closed() {
        for (library, blob) in [
            (None, Some(PathBuf::from("/safe/storage"))),
            (Some(PathBuf::from("/safe/library")), None),
            (
                Some(PathBuf::from("/safe/library")),
                Some(PathBuf::from("/safe/library/custom")),
            ),
            (
                Some(PathBuf::from("/safe/library")),
                Some(PathBuf::from("/safe/../storage")),
            ),
            (
                Some(PathBuf::from("/safe/library")),
                Some(PathBuf::from(OsString::from_vec(vec![b'/', b's', 0xff]))),
            ),
        ] {
            assert_eq!(
                resolved(library, blob).validate().err(),
                Some(BlobStorageError::Configuration)
            );
        }
    }

    #[test]
    fn every_numeric_adjacent_boundary_is_enforced() {
        let library = Some(PathBuf::from("/Users/example/Library"));
        let blob = Some(PathBuf::from("/Users/example/Library/storage"));
        let source = BlobConfigSource::CompiledDefault;
        for values in [
            (0, 2, 2, 8 * MIB as usize, TIB, 2 * TIB, 10 * GIB, 5),
            (9, 2, 2, 8 * MIB as usize, TIB, 2 * TIB, 10 * GIB, 5),
            (2, 0, 2, 8 * MIB as usize, TIB, 2 * TIB, 10 * GIB, 5),
            (2, 2, 9, 8 * MIB as usize, TIB, 2 * TIB, 10 * GIB, 5),
            (2, 2, 2, MIB as usize - 1, TIB, 2 * TIB, 10 * GIB, 5),
            (2, 2, 2, 32 * MIB as usize + 1, TIB, 2 * TIB, 10 * GIB, 5),
            (2, 2, 2, 8 * MIB as usize, TIB + 1, 2 * TIB, 10 * GIB, 5),
            (2, 2, 2, 8 * MIB as usize, TIB, 2 * TIB + 1, 10 * GIB, 5),
            (2, 2, 2, 8 * MIB as usize, TIB, 2 * TIB, 10 * GIB - 1, 5),
            (2, 2, 2, 8 * MIB as usize, TIB, 2 * TIB, 10 * GIB, 4),
        ] {
            let (io, hash, ingests, buffer, max, staging, free, percent) = values;
            let candidate = ResolvedBlobStorageConfig::from_selected(
                library.clone(),
                source,
                blob.clone(),
                source,
                raw(io),
                source,
                raw(hash),
                source,
                raw(ingests),
                source,
                raw(buffer),
                source,
                raw(max),
                source,
                raw(staging),
                source,
                raw(free),
                source,
                raw(percent),
                source,
            );
            assert_eq!(
                candidate.validate().err(),
                Some(BlobStorageError::Configuration)
            );
        }
    }

    #[test]
    fn missing_signed_whitespace_nondecimal_and_overflow_text_is_rejected() {
        for invalid in [
            None,
            Some(String::new()),
            Some("+2".to_owned()),
            Some("-2".to_owned()),
            Some(" 2".to_owned()),
            Some("2 ".to_owned()),
            Some("2.0".to_owned()),
            Some("18446744073709551616".to_owned()),
        ] {
            let mut candidate = resolved(
                Some(PathBuf::from("/Users/example/Library")),
                Some(PathBuf::from("/Users/example/Library/storage")),
            );
            candidate.storage_io_concurrency = invalid;
            assert_eq!(
                candidate.validate().err(),
                Some(BlobStorageError::Configuration)
            );
        }
    }

    #[test]
    fn blob_and_default_library_path_headroom_is_exact() {
        fn path_with_last(last: usize) -> PathBuf {
            PathBuf::from(format!(
                "/{}/{}/{}/{}",
                "a".repeat(255),
                "b".repeat(255),
                "c".repeat(255),
                "d".repeat(last)
            ))
        }

        let exact_blob = path_with_last(168);
        assert_eq!(exact_blob.as_os_str().as_encoded_bytes().len(), 937);
        resolved(
            Some(PathBuf::from("/Users/example/Library")),
            Some(exact_blob),
        )
        .validate()
        .unwrap();

        let too_long_blob = path_with_last(169);
        assert_eq!(too_long_blob.as_os_str().as_encoded_bytes().len(), 938);
        assert_eq!(
            resolved(
                Some(PathBuf::from("/Users/example/Library")),
                Some(too_long_blob),
            )
            .validate()
            .err(),
            Some(BlobStorageError::Configuration)
        );

        let exact_library = path_with_last(160);
        assert_eq!(exact_library.as_os_str().as_encoded_bytes().len(), 929);
        resolved(
            Some(exact_library.clone()),
            Some(exact_library.join("storage")),
        )
        .validate()
        .unwrap();
        let too_long_library = path_with_last(161);
        assert_eq!(
            resolved(
                Some(too_long_library.clone()),
                Some(too_long_library.join("storage")),
            )
            .validate()
            .err(),
            Some(BlobStorageError::Configuration)
        );
    }
}
