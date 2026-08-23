use mengxia_platform_fs::ValidatedAbsolutePath;

use super::{StoreConfig, StoreError};

/// Converts the source-free store DTO's lexical root into retained platform
/// authority. Platform diagnostics are deliberately collapsed to the one safe,
/// static configuration error at this pre-mutation boundary.
pub(crate) fn authorize_existing_root(
    config: &StoreConfig,
) -> Result<ValidatedAbsolutePath, StoreError> {
    ValidatedAbsolutePath::authorize_existing(config.library_root().as_path())
        .map_err(|_| StoreError::Configuration)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::authorize_existing_root;
    use crate::{ConfigSource, ResolvedStoreConfig, StoreError};

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
}
