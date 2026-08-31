use std::ffi::OsString;
use std::fs::File;
use std::os::fd::AsFd as _;
use std::os::unix::fs::FileExt as _;
use std::path::{Component, Path};

use rustix::fs::{FileType, Mode, OFlags, fstat, open, openat};
use rustix::process::geteuid;

use super::{
    AuthorityError, ComponentRole, inspect_directory, inspect_security, revalidate_components,
    validate_component_policy, validate_lexical_absolute_path,
};

const MAX_CONFIG_BYTES: usize = 16_384;

/// Reads one owner-only Library configuration through retained, no-follow descriptors.
pub fn read_library_config(path: &Path) -> Result<Vec<u8>, AuthorityError> {
    validate_lexical_absolute_path(path)?;
    let names: Vec<OsString> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_os_string()),
            _ => None,
        })
        .collect();
    if names.len() < 2 {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let owner_uid = geteuid().as_raw();
    let root = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let root_security = inspect_directory(root.as_fd())?;
    validate_component_policy(root_security, ComponentRole::Ancestor, owner_uid)?;
    let mut retained = vec![super::RetainedComponent {
        name: None,
        fd: root,
        security: root_security,
        role: ComponentRole::Ancestor,
    }];
    for (index, name) in names.iter().enumerate().take(names.len() - 1) {
        let role = if index + 2 == names.len() {
            ComponentRole::FinalParent
        } else {
            ComponentRole::Ancestor
        };
        let parent = retained.last().ok_or(AuthorityError::UnsafeConfiguration)?;
        let fd = openat(
            parent.fd.as_fd(),
            name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| AuthorityError::UnsafeConfiguration)?;
        let security = inspect_directory(fd.as_fd())?;
        validate_component_policy(security, role, owner_uid)?;
        if matches!(role, ComponentRole::FinalParent)
            && (security.mode != 0o700 || !security.acl.is_empty())
        {
            return Err(AuthorityError::UnsafeConfiguration);
        }
        retained.push(super::RetainedComponent {
            name: Some(name.clone()),
            fd,
            security,
            role,
        });
    }
    let parent = retained.last().ok_or(AuthorityError::UnsafeConfiguration)?;
    let name = names.last().ok_or(AuthorityError::UnsafeConfiguration)?;
    let fd = openat(
        parent.fd.as_fd(),
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let before = fstat(fd.as_fd()).map_err(|_| AuthorityError::Io)?;
    if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_nlink != 1
        || before.st_uid != owner_uid
        || Mode::from_raw_mode(before.st_mode).as_raw_mode() != 0o600
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let security = inspect_security(
        fd.as_fd(),
        before.st_dev as u64,
        before.st_ino as u64,
        before.st_uid,
        before.st_mode,
    )?;
    if !security.acl.is_empty() || before.st_size < 0 {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let length =
        usize::try_from(before.st_size).map_err(|_| AuthorityError::UnsafeConfiguration)?;
    if length > MAX_CONFIG_BYTES {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let file = File::from(fd);
    let mut bytes = vec![0_u8; length];
    let mut read = 0;
    while read < length {
        let count = file
            .read_at(
                &mut bytes[read..],
                u64::try_from(read).map_err(|_| AuthorityError::Io)?,
            )
            .map_err(|_| AuthorityError::Io)?;
        if count == 0 {
            return Err(AuthorityError::Io);
        }
        read += count;
    }
    let mut extra = [0_u8; 1];
    if file
        .read_at(
            &mut extra,
            u64::try_from(length).map_err(|_| AuthorityError::Io)?,
        )
        .map_err(|_| AuthorityError::Io)?
        != 0
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    let after = fstat(file.as_fd()).map_err(|_| AuthorityError::Io)?;
    if before.st_dev != after.st_dev
        || before.st_ino != after.st_ino
        || before.st_size != after.st_size
        || before.st_mtime != after.st_mtime
        || before.st_mtime_nsec != after.st_mtime_nsec
    {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    revalidate_components(&retained, owner_uid)?;
    let reopened = openat(
        parent.fd.as_fd(),
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| AuthorityError::UnsafeConfiguration)?;
    let edge = fstat(reopened.as_fd()).map_err(|_| AuthorityError::Io)?;
    if edge.st_dev != before.st_dev || edge.st_ino != before.st_ino {
        return Err(AuthorityError::UnsafeConfiguration);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write as _;
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _, symlink};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{MAX_CONFIG_BYTES, read_library_config};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn fixture(name: &str) -> PathBuf {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("workspace root")
            .join("target")
            .join(format!(
                "mengxia-task007-config-{name}-{}-{}",
                std::process::id(),
                FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed),
            ));
        fs::create_dir(&path).expect("create config fixture parent");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("secure config fixture parent");
        path
    }

    fn write_config(parent: &Path, bytes: &[u8]) -> PathBuf {
        let path = parent.join("library.conf");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .expect("create config fixture");
        file.write_all(bytes).expect("write config fixture");
        file.sync_all().expect("sync config fixture");
        path
    }

    #[test]
    fn owner_only_regular_config_is_read_exactly() {
        let parent = fixture("valid");
        let expected = b"MENGXIA_LIBRARY_CONFIG_V1\nMENGXIA_MAX_FRAME_BYTES=65536\n";
        let path = write_config(&parent, expected);
        assert_eq!(read_library_config(&path), Ok(expected.to_vec()));
        fs::remove_dir_all(parent).expect("remove valid config fixture");
    }

    #[test]
    fn unsafe_parent_file_link_type_and_size_fail_closed() {
        let parent = fixture("matrix");

        let wrong_mode = write_config(&parent, b"MENGXIA_LIBRARY_CONFIG_V1\n");
        fs::set_permissions(&wrong_mode, fs::Permissions::from_mode(0o640))
            .expect("set unsafe config mode");
        assert!(read_library_config(&wrong_mode).is_err());
        fs::remove_file(&wrong_mode).expect("remove wrong-mode config");

        let linked = write_config(&parent, b"MENGXIA_LIBRARY_CONFIG_V1\n");
        fs::hard_link(&linked, parent.join("second-link")).expect("create config hard link");
        assert!(read_library_config(&linked).is_err());
        fs::remove_file(&linked).expect("remove hard-linked config");
        fs::remove_file(parent.join("second-link")).expect("remove second config link");

        let oversized = write_config(&parent, &vec![b'x'; MAX_CONFIG_BYTES + 1]);
        assert!(read_library_config(&oversized).is_err());
        fs::remove_file(&oversized).expect("remove oversized config");

        let target = write_config(&parent, b"MENGXIA_LIBRARY_CONFIG_V1\n");
        let link = parent.join("linked.conf");
        symlink(&target, &link).expect("create config symlink");
        assert!(read_library_config(&link).is_err());
        fs::remove_file(&link).expect("remove config symlink");
        fs::remove_file(&target).expect("remove config target");

        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755))
            .expect("set unsafe config parent mode");
        let path = write_config(&parent, b"MENGXIA_LIBRARY_CONFIG_V1\n");
        assert!(read_library_config(&path).is_err());
        fs::remove_dir_all(parent).expect("remove config matrix fixture");
    }
}
