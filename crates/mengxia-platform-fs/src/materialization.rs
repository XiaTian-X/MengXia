//! Descriptor-first destination authority and exact TASK-008 intent codec.

use std::ffi::OsString;
use std::fs::File;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

use rustix::fs::{
    Mode, OFlags, RenameFlags, fcntl_fullfsync, fstat, open, openat, renameat_with, unlinkat,
};
use rustix::io::{pread, pwrite};
use sha2::{Digest as _, Sha256};

use super::blob_storage::{BlobFileError, OpenedBlobRootAuthority, exact_final_component};
use super::{
    AuthorityError, ComponentRole, MacOsObjectSecurity, inspect_directory,
    validate_component_policy,
};

pub const MATERIALIZATION_INTENT_BYTES: usize = 512;
const MAX_DESTINATION_BYTES: usize = 1023;
const MAX_BASENAME_BYTES: usize = 255;
const INTENT_MAGIC: [u8; 16] = *b"MENGXIA_MAT_V1\0\0";
const INTENT_VERSION: u16 = 1;
const INTENT_CHECKSUM_OFFSET: usize = 480;
const SIDECAR_PREFIX: &[u8] = b".mengxia-materialize-";
const INTENT_SUFFIX: &[u8] = b".intent";
const STAGING_SUFFIX: &[u8] = b".staging";
const MAX_INTERRUPTED_SYSCALL_RETRIES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MaterializationDestinationError {
    InvalidPath,
    Conflict,
    UnsafeConfiguration,
    SourceCorruption,
    Io,
}

struct RetainedDestinationComponent {
    name: Option<OsString>,
    fd: OwnedFd,
    security: MacOsObjectSecurity,
    role: ComponentRole,
}

/// Retained authority for one absent final destination under an owner-only parent.
pub struct OpenedMaterializationDestination {
    components: Vec<RetainedDestinationComponent>,
    final_name: OsString,
    owner_uid: u32,
    parent_device: u64,
    parent_inode: u64,
}

/// Exact already-validated semantic values bound into one destination intent.
pub struct MaterializationIntentBinding {
    command_id: [u8; 16],
    asset_id: [u8; 16],
    asset_revision_id: [u8; 16],
    representation_id: [u8; 16],
    resource_id: [u8; 16],
    member_ordinal: u32,
    expected_digest: [u8; 32],
    expected_length: u64,
    request_digest: [u8; 32],
    library_id: [u8; 16],
}

/// Durable, exclusively-created evidence that authorizes only its paired staging name.
pub struct OpenedMaterializationIntent {
    file: File,
    name: String,
    staging_name: String,
    record: [u8; MATERIALIZATION_INTENT_BYTES],
    device: u64,
    inode: u64,
}

/// Exclusively-created empty destination staging inode.
pub struct OpenedMaterializationStaging {
    file: File,
    name: String,
    device: u64,
    inode: u64,
}

/// Staging whose bytes and retained inode match the exact managed source binding.
pub struct VerifiedMaterializationStaging {
    staging: OpenedMaterializationStaging,
    digest: [u8; 32],
    length: u64,
}

/// Durable no-clobber final effect produced at M10.
pub struct OpenedMaterializedFile {
    file: File,
    device: u64,
    inode: u64,
}

pub enum MaterializationCopyOutcome {
    Verified(VerifiedMaterializationStaging),
    Stopped(OpenedMaterializationStaging),
}

pub enum MaterializationResumeOutcome {
    Published(
        Box<(
            OpenedMaterializationDestination,
            OpenedMaterializationIntent,
            OpenedMaterializedFile,
        )>,
    ),
    Stopped,
}

/// Read-only classification of an exact prior materialization prefix.
pub enum OpenedMaterializationRecovery {
    /// No final exists. An optional valid intent and its exact staging inode may be resumed.
    BeforePublish {
        destination: OpenedMaterializationDestination,
        intent: Option<OpenedMaterializationIntent>,
        staging: Option<OpenedMaterializationStaging>,
    },
    /// The exact final and intent both exist and the final bytes have been verified.
    Published {
        destination: OpenedMaterializationDestination,
        intent: OpenedMaterializationIntent,
        published: OpenedMaterializedFile,
    },
}

impl MaterializationIntentBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_id: [u8; 16],
        asset_id: [u8; 16],
        asset_revision_id: [u8; 16],
        representation_id: [u8; 16],
        resource_id: [u8; 16],
        member_ordinal: u32,
        expected_digest: [u8; 32],
        expected_length: u64,
        request_digest: [u8; 32],
        library_id: [u8; 16],
    ) -> Result<Self, MaterializationDestinationError> {
        if [
            command_id,
            asset_id,
            asset_revision_id,
            representation_id,
            resource_id,
            library_id,
        ]
        .contains(&[0; 16])
            || member_ordinal > 4095
            || expected_length > 1_099_511_627_776
        {
            return Err(MaterializationDestinationError::InvalidPath);
        }
        Ok(Self {
            command_id,
            asset_id,
            asset_revision_id,
            representation_id,
            resource_id,
            member_ordinal,
            expected_digest,
            expected_length,
            request_digest,
            library_id,
        })
    }

    #[must_use]
    pub const fn command_id(&self) -> [u8; 16] {
        self.command_id
    }
}

impl OpenedMaterializationDestination {
    pub fn authorize_new(
        path: &Path,
        blob_authority: &OpenedBlobRootAuthority,
    ) -> Result<Self, MaterializationDestinationError> {
        let authority = Self::open_authority(path, blob_authority)?;
        authority.revalidate_absent()?;
        Ok(authority)
    }

    /// Classifies the exact command sidecars and final without mutating any name.
    pub fn authorize_recovery(
        path: &Path,
        blob_authority: &OpenedBlobRootAuthority,
        binding: &MaterializationIntentBinding,
        buffer_bytes: usize,
    ) -> Result<OpenedMaterializationRecovery, MaterializationDestinationError> {
        let destination = Self::open_authority(path, blob_authority)?;
        let (intent_name, staging_name) = sidecar_names(binding.command_id());
        let parent = destination.parent_fd()?;
        let intent_fd = open_optional(parent, intent_name.as_str())?;
        let staging_fd = open_optional(parent, staging_name.as_str())?;
        let final_fd = open_optional(parent, &destination.final_name)?;

        let intent = match intent_fd {
            Some(fd) => {
                let expected = destination.encode_intent(binding)?;
                let security = validate_created_file(
                    fd.as_fd(),
                    destination.owner_uid,
                    destination.parent_device,
                    MATERIALIZATION_INTENT_BYTES as u64,
                )?;
                exact_final_component(fd.as_fd(), intent_name.as_bytes())
                    .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
                validate_exact_record(fd.as_fd(), &expected)?;
                Some(OpenedMaterializationIntent {
                    file: File::from(fd),
                    name: intent_name,
                    staging_name,
                    record: expected,
                    device: security.device,
                    inode: security.inode,
                })
            }
            None => None,
        };

        let staging = match staging_fd {
            Some(fd) => {
                if intent.is_none() || final_fd.is_some() {
                    return Err(MaterializationDestinationError::UnsafeConfiguration);
                }
                let security = validate_owned_file(
                    fd.as_fd(),
                    destination.owner_uid,
                    destination.parent_device,
                    None,
                )?;
                let name = intent
                    .as_ref()
                    .ok_or(MaterializationDestinationError::UnsafeConfiguration)?
                    .staging_name
                    .clone();
                exact_final_component(fd.as_fd(), name.as_bytes())
                    .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
                Some(OpenedMaterializationStaging {
                    file: File::from(fd),
                    name,
                    device: security.device,
                    inode: security.inode,
                })
            }
            None => None,
        };

        if let Some(fd) = final_fd {
            let intent = intent.ok_or(MaterializationDestinationError::UnsafeConfiguration)?;
            let security = validate_created_file(
                fd.as_fd(),
                destination.owner_uid,
                destination.parent_device,
                binding.expected_length,
            )?;
            exact_final_component(fd.as_fd(), destination.final_name.as_bytes())
                .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
            verify_file_content(
                fd.as_fd(),
                binding.expected_digest,
                binding.expected_length,
                buffer_bytes,
            )?;
            destination.validate_open_intent(&intent, binding)?;
            destination.revalidate_parent()?;
            return Ok(OpenedMaterializationRecovery::Published {
                destination,
                intent,
                published: OpenedMaterializedFile {
                    file: File::from(fd),
                    device: security.device,
                    inode: security.inode,
                },
            });
        }
        if intent.is_none() && staging.is_some() {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        destination.revalidate_absent()?;
        Ok(OpenedMaterializationRecovery::BeforePublish {
            destination,
            intent,
            staging,
        })
    }

    /// Cleans only an exact valid intent after the command ledger is already completed.
    /// The final is never created or replaced, and an unprovable prefix is retained.
    pub fn cleanup_completed(
        path: &Path,
        blob_authority: &OpenedBlobRootAuthority,
        binding: &MaterializationIntentBinding,
        buffer_bytes: usize,
    ) -> Result<(), MaterializationDestinationError> {
        let destination = Self::open_authority(path, blob_authority)?;
        let (intent_name, staging_name) = sidecar_names(binding.command_id());
        let parent = destination.parent_fd()?;
        if open_optional(parent, staging_name.as_str())?.is_some() {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        let Some(intent_fd) = open_optional(parent, intent_name.as_str())? else {
            destination.revalidate_parent()?;
            return Ok(());
        };
        let expected = destination.encode_intent(binding)?;
        let security = validate_created_file(
            intent_fd.as_fd(),
            destination.owner_uid,
            destination.parent_device,
            MATERIALIZATION_INTENT_BYTES as u64,
        )?;
        exact_final_component(intent_fd.as_fd(), intent_name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        validate_exact_record(intent_fd.as_fd(), &expected)?;
        let intent = OpenedMaterializationIntent {
            file: File::from(intent_fd),
            name: intent_name,
            staging_name,
            record: expected,
            device: security.device,
            inode: security.inode,
        };
        let final_fd = open_optional(parent, &destination.final_name)?;
        if let Some(final_fd) = final_fd {
            let final_security = validate_created_file(
                final_fd.as_fd(),
                destination.owner_uid,
                destination.parent_device,
                binding.expected_length,
            )?;
            exact_final_component(final_fd.as_fd(), destination.final_name.as_bytes())
                .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
            verify_file_content(
                final_fd.as_fd(),
                binding.expected_digest,
                binding.expected_length,
                buffer_bytes,
            )?;
            let published = OpenedMaterializedFile {
                file: File::from(final_fd),
                device: final_security.device,
                inode: final_security.inode,
            };
            return destination.cleanup_intent_after_publish(
                &published,
                &intent,
                binding,
                buffer_bytes,
            );
        }
        destination.validate_open_intent(&intent, binding)?;
        destination.revalidate_parent()?;
        unlinkat(
            destination.parent_fd()?,
            intent.name.as_str(),
            rustix::fs::AtFlags::empty(),
        )
        .map_err(|_| MaterializationDestinationError::Io)?;
        fcntl_fullfsync(destination.parent_fd()?)
            .map_err(|_| MaterializationDestinationError::Io)?;
        destination.revalidate_parent()
    }

    fn open_authority(
        path: &Path,
        blob_authority: &OpenedBlobRootAuthority,
    ) -> Result<Self, MaterializationDestinationError> {
        validate_destination_path(path)?;
        blob_authority.revalidate().map_err(map_authority_error)?;
        let names: Vec<OsString> = path
            .components()
            .filter_map(|component| match component {
                Component::Normal(name) => Some(name.to_os_string()),
                _ => None,
            })
            .collect();
        let final_name = names
            .last()
            .ok_or(MaterializationDestinationError::InvalidPath)?
            .clone();
        let parent_names = &names[..names.len() - 1];
        let owner_uid = blob_authority.materialization_owner_uid();
        let excluded = blob_authority.materialization_excluded_roots();
        let root = open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        let root_role = if parent_names.is_empty() {
            ComponentRole::FinalParent
        } else {
            ComponentRole::Ancestor
        };
        let root_security = inspect_destination_directory(root.as_fd(), root_role, owner_uid)?;
        reject_excluded(root_security, excluded)?;
        let mut components = vec![RetainedDestinationComponent {
            name: None,
            fd: root,
            security: root_security,
            role: root_role,
        }];
        for (index, name) in parent_names.iter().enumerate() {
            let role = if index + 1 == parent_names.len() {
                ComponentRole::FinalParent
            } else {
                ComponentRole::Ancestor
            };
            let parent = components
                .last()
                .ok_or(MaterializationDestinationError::UnsafeConfiguration)?;
            let fd = openat(
                parent.fd.as_fd(),
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
            let security = inspect_destination_directory(fd.as_fd(), role, owner_uid)?;
            reject_excluded(security, excluded)?;
            exact_final_component(fd.as_fd(), name.as_bytes())
                .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
            components.push(RetainedDestinationComponent {
                name: Some(name.clone()),
                fd,
                security,
                role,
            });
        }
        let parent = components
            .last()
            .ok_or(MaterializationDestinationError::UnsafeConfiguration)?;
        Ok(Self {
            parent_device: parent.security.device,
            parent_inode: parent.security.inode,
            components,
            final_name,
            owner_uid,
        })
    }

    /// Revalidates every retained edge and the NEW-command absence precondition.
    pub fn revalidate_absent(&self) -> Result<(), MaterializationDestinationError> {
        let mut fresh_parent = open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        let root = self
            .components
            .first()
            .ok_or(MaterializationDestinationError::UnsafeConfiguration)?;
        let security =
            inspect_destination_directory(fresh_parent.as_fd(), root.role, self.owner_uid)?;
        if !security.same_object(root.security) {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        for retained in self.components.iter().skip(1) {
            let name = retained
                .name
                .as_ref()
                .ok_or(MaterializationDestinationError::UnsafeConfiguration)?;
            let fresh = openat(
                fresh_parent.as_fd(),
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
            let security =
                inspect_destination_directory(fresh.as_fd(), retained.role, self.owner_uid)?;
            if !security.same_object(retained.security) {
                return Err(MaterializationDestinationError::UnsafeConfiguration);
            }
            exact_final_component(fresh.as_fd(), name.as_bytes())
                .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
            fresh_parent = fresh;
        }
        match openat(
            fresh_parent.as_fd(),
            &self.final_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        ) {
            Err(rustix::io::Errno::NOENT) => Ok(()),
            Ok(_) => Err(MaterializationDestinationError::Conflict),
            Err(_) => Err(MaterializationDestinationError::UnsafeConfiguration),
        }
    }

    pub fn encode_intent(
        &self,
        binding: &MaterializationIntentBinding,
    ) -> Result<[u8; MATERIALIZATION_INTENT_BYTES], MaterializationDestinationError> {
        let name = self.final_name.as_bytes();
        encode_intent_record(binding, name, self.parent_device, self.parent_inode)
    }

    pub fn validates_intent(&self, record: &[u8], binding: &MaterializationIntentBinding) -> bool {
        let Ok(record): Result<&[u8; MATERIALIZATION_INTENT_BYTES], _> = record.try_into() else {
            return false;
        };
        self.encode_intent(binding)
            .is_ok_and(|expected| &expected == record)
    }

    /// Executes M3-M4: exclusively creates, writes and durably re-proves the exact intent.
    pub fn create_intent(
        &self,
        binding: &MaterializationIntentBinding,
    ) -> Result<OpenedMaterializationIntent, MaterializationDestinationError> {
        self.revalidate_absent()?;
        let record = self.encode_intent(binding)?;
        let (name, staging_name) = sidecar_names(binding.command_id());
        let parent = self.parent_fd()?;
        let fd = openat(
            parent,
            name.as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(map_create_error)?;
        let security = validate_created_file(fd.as_fd(), self.owner_uid, self.parent_device, 0)?;
        exact_final_component(fd.as_fd(), name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        write_all_at(fd.as_fd(), &record, 0)?;
        validate_exact_record(fd.as_fd(), &record)?;
        fcntl_fullfsync(fd.as_fd()).map_err(|_| MaterializationDestinationError::Io)?;
        fcntl_fullfsync(parent).map_err(|_| MaterializationDestinationError::Io)?;

        let reopened = openat(
            parent,
            name.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        let reopened_security = validate_created_file(
            reopened.as_fd(),
            self.owner_uid,
            self.parent_device,
            MATERIALIZATION_INTENT_BYTES as u64,
        )?;
        if !reopened_security.same_object(security) {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        exact_final_component(reopened.as_fd(), name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        validate_exact_record(reopened.as_fd(), &record)?;
        self.revalidate_parent()?;
        Ok(OpenedMaterializationIntent {
            file: File::from(fd),
            name,
            staging_name,
            record,
            device: security.device,
            inode: security.inode,
        })
    }

    /// Executes M5 only after re-proving the exact durable intent.
    pub fn create_staging(
        &self,
        intent: &OpenedMaterializationIntent,
        binding: &MaterializationIntentBinding,
    ) -> Result<OpenedMaterializationStaging, MaterializationDestinationError> {
        self.validate_open_intent(intent, binding)?;
        self.revalidate_absent()?;
        let parent = self.parent_fd()?;
        let fd = openat(
            parent,
            intent.staging_name.as_str(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(map_create_error)?;
        let security = validate_created_file(fd.as_fd(), self.owner_uid, self.parent_device, 0)?;
        exact_final_component(fd.as_fd(), intent.staging_name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        fcntl_fullfsync(fd.as_fd()).map_err(|_| MaterializationDestinationError::Io)?;
        fcntl_fullfsync(parent).map_err(|_| MaterializationDestinationError::Io)?;
        self.revalidate_absent()?;
        Ok(OpenedMaterializationStaging {
            file: File::from(fd),
            name: intent.staging_name.clone(),
            device: security.device,
            inode: security.inode,
        })
    }

    /// Re-proves the exact staging inode, name and current length.
    pub fn revalidate_staging(
        &self,
        staging: &OpenedMaterializationStaging,
        expected_length: u64,
    ) -> Result<(), MaterializationDestinationError> {
        self.revalidate_parent()?;
        let security = validate_created_file(
            staging.file.as_fd(),
            self.owner_uid,
            self.parent_device,
            expected_length,
        )?;
        if security.device != staging.device || security.inode != staging.inode {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        exact_final_component(staging.file.as_fd(), staging.name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)
    }

    /// Executes M6-M8 with a fixed digest-derived CAS source and bounded memory.
    pub fn copy_managed_blob(
        &self,
        blob_authority: &OpenedBlobRootAuthority,
        staging: OpenedMaterializationStaging,
        binding: &MaterializationIntentBinding,
        buffer_bytes: usize,
    ) -> Result<VerifiedMaterializationStaging, MaterializationDestinationError> {
        match self.copy_managed_blob_controlled(
            blob_authority,
            staging,
            binding,
            buffer_bytes,
            || false,
        )? {
            MaterializationCopyOutcome::Verified(verified) => Ok(verified),
            MaterializationCopyOutcome::Stopped(_) => {
                Err(MaterializationDestinationError::UnsafeConfiguration)
            }
        }
    }

    pub fn copy_managed_blob_controlled(
        &self,
        blob_authority: &OpenedBlobRootAuthority,
        staging: OpenedMaterializationStaging,
        binding: &MaterializationIntentBinding,
        buffer_bytes: usize,
        mut should_stop: impl FnMut() -> bool,
    ) -> Result<MaterializationCopyOutcome, MaterializationDestinationError> {
        if !(1_048_576..=33_554_432).contains(&buffer_bytes) {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        self.revalidate_staging(&staging, 0)?;
        if should_stop() {
            return Ok(MaterializationCopyOutcome::Stopped(staging));
        }
        let source = blob_authority
            .open_canonical_blob(binding.expected_digest, binding.expected_length)
            .map_err(map_blob_error)?;
        if source.declared_length() != binding.expected_length {
            return Err(MaterializationDestinationError::SourceCorruption);
        }
        source.revalidate().map_err(map_blob_error)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; buffer_bytes];
        let mut offset = 0_u64;
        while offset < binding.expected_length {
            if should_stop() {
                return Ok(MaterializationCopyOutcome::Stopped(staging));
            }
            let remaining =
                usize::try_from((binding.expected_length - offset).min(buffer_bytes as u64))
                    .map_err(|_| MaterializationDestinationError::SourceCorruption)?;
            let read = source
                .read_at(&mut buffer[..remaining], offset)
                .map_err(map_blob_error)?;
            if read == 0 {
                return Err(MaterializationDestinationError::SourceCorruption);
            }
            staging.write_at(&buffer[..read], offset)?;
            hasher.update(&buffer[..read]);
            offset = offset
                .checked_add(read as u64)
                .ok_or(MaterializationDestinationError::SourceCorruption)?;
        }
        if should_stop() {
            return Ok(MaterializationCopyOutcome::Stopped(staging));
        }
        let mut trailing = [0_u8; 1];
        if source
            .read_at(&mut trailing, binding.expected_length)
            .map_err(map_blob_error)?
            != 0
            || <[u8; 32]>::from(hasher.finalize()) != binding.expected_digest
        {
            return Err(MaterializationDestinationError::SourceCorruption);
        }
        source.revalidate().map_err(map_blob_error)?;
        blob_authority.revalidate().map_err(map_authority_error)?;
        fcntl_fullfsync(staging.file.as_fd()).map_err(|_| MaterializationDestinationError::Io)?;
        self.revalidate_staging(&staging, binding.expected_length)?;
        verify_file_content(
            staging.file.as_fd(),
            binding.expected_digest,
            binding.expected_length,
            buffer_bytes,
        )?;
        self.revalidate_parent()?;
        if should_stop() {
            return Ok(MaterializationCopyOutcome::Stopped(staging));
        }
        Ok(MaterializationCopyOutcome::Verified(
            VerifiedMaterializationStaging {
                staging,
                digest: binding.expected_digest,
                length: binding.expected_length,
            },
        ))
    }

    /// Executes M9-M10 using same-parent `RENAME_EXCL`; no overwrite path exists.
    pub fn publish(
        &self,
        intent: &OpenedMaterializationIntent,
        binding: &MaterializationIntentBinding,
        verified: VerifiedMaterializationStaging,
        buffer_bytes: usize,
    ) -> Result<OpenedMaterializedFile, MaterializationDestinationError> {
        self.validate_open_intent(intent, binding)?;
        if verified.digest != binding.expected_digest || verified.length != binding.expected_length
        {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        self.revalidate_staging(&verified.staging, binding.expected_length)?;
        verify_file_content(
            verified.staging.file.as_fd(),
            binding.expected_digest,
            binding.expected_length,
            buffer_bytes,
        )?;
        self.revalidate_absent()?;
        let parent = self.parent_fd()?;
        renameat_with(
            parent,
            verified.staging.name.as_str(),
            parent,
            &self.final_name,
            RenameFlags::NOREPLACE,
        )
        .map_err(|error| match error {
            rustix::io::Errno::EXIST => MaterializationDestinationError::Conflict,
            rustix::io::Errno::XDEV => MaterializationDestinationError::UnsafeConfiguration,
            _ => MaterializationDestinationError::Io,
        })?;
        let final_fd = openat(
            parent,
            &self.final_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        let security = validate_created_file(
            final_fd.as_fd(),
            self.owner_uid,
            self.parent_device,
            binding.expected_length,
        )?;
        if security.device != verified.staging.device || security.inode != verified.staging.inode {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        exact_final_component(final_fd.as_fd(), self.final_name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        verify_file_content(
            final_fd.as_fd(),
            binding.expected_digest,
            binding.expected_length,
            buffer_bytes,
        )?;
        self.revalidate_parent()?;
        fcntl_fullfsync(parent).map_err(|_| MaterializationDestinationError::Io)?;
        self.revalidate_parent()?;
        let reopened = openat(
            parent,
            &self.final_name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        let reopened_security = validate_created_file(
            reopened.as_fd(),
            self.owner_uid,
            self.parent_device,
            binding.expected_length,
        )?;
        if !reopened_security.same_object(security) {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        verify_file_content(
            reopened.as_fd(),
            binding.expected_digest,
            binding.expected_length,
            buffer_bytes,
        )?;
        Ok(OpenedMaterializedFile {
            file: File::from(final_fd),
            device: security.device,
            inode: security.inode,
        })
    }

    /// Removes only the exact owned prefix while the final target remains absent.
    pub fn cleanup_before_publish(
        &self,
        intent: &OpenedMaterializationIntent,
        staging: Option<&OpenedMaterializationStaging>,
        binding: &MaterializationIntentBinding,
    ) -> Result<(), MaterializationDestinationError> {
        self.revalidate_absent()?;
        self.validate_open_intent(intent, binding)?;
        if let Some(staging) = staging {
            self.validate_staging_identity(staging)?;
            unlinkat(
                self.parent_fd()?,
                staging.name.as_str(),
                rustix::fs::AtFlags::empty(),
            )
            .map_err(|_| MaterializationDestinationError::Io)?;
            fcntl_fullfsync(self.parent_fd()?).map_err(|_| MaterializationDestinationError::Io)?;
        }
        self.validate_open_intent(intent, binding)?;
        unlinkat(
            self.parent_fd()?,
            intent.name.as_str(),
            rustix::fs::AtFlags::empty(),
        )
        .map_err(|_| MaterializationDestinationError::Io)?;
        fcntl_fullfsync(self.parent_fd()?).map_err(|_| MaterializationDestinationError::Io)?;
        self.revalidate_absent()
    }

    /// Executes M12 only after the durable published inode is still proven.
    pub fn cleanup_intent_after_publish(
        &self,
        published: &OpenedMaterializedFile,
        intent: &OpenedMaterializationIntent,
        binding: &MaterializationIntentBinding,
        buffer_bytes: usize,
    ) -> Result<(), MaterializationDestinationError> {
        self.revalidate_published(published, binding, buffer_bytes)?;
        self.validate_open_intent(intent, binding)?;
        self.revalidate_parent()?;
        unlinkat(
            self.parent_fd()?,
            intent.name.as_str(),
            rustix::fs::AtFlags::empty(),
        )
        .map_err(|_| MaterializationDestinationError::Io)?;
        fcntl_fullfsync(self.parent_fd()?).map_err(|_| MaterializationDestinationError::Io)?;
        self.revalidate_parent()
    }

    /// Re-proves a published result without exposing its descriptor or destination path.
    pub fn revalidate_published(
        &self,
        published: &OpenedMaterializedFile,
        binding: &MaterializationIntentBinding,
        buffer_bytes: usize,
    ) -> Result<(), MaterializationDestinationError> {
        self.revalidate_parent()?;
        let security = validate_created_file(
            published.file.as_fd(),
            self.owner_uid,
            self.parent_device,
            binding.expected_length,
        )?;
        if security.device != published.device || security.inode != published.inode {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        exact_final_component(published.file.as_fd(), self.final_name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        verify_file_content(
            published.file.as_fd(),
            binding.expected_digest,
            binding.expected_length,
            buffer_bytes,
        )
    }

    fn validate_open_intent(
        &self,
        intent: &OpenedMaterializationIntent,
        binding: &MaterializationIntentBinding,
    ) -> Result<(), MaterializationDestinationError> {
        let expected = self.encode_intent(binding)?;
        if intent.record != expected {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        let security = validate_created_file(
            intent.file.as_fd(),
            self.owner_uid,
            self.parent_device,
            MATERIALIZATION_INTENT_BYTES as u64,
        )?;
        if security.device != intent.device || security.inode != intent.inode {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        exact_final_component(intent.file.as_fd(), intent.name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?;
        validate_exact_record(intent.file.as_fd(), &expected)
    }

    fn validate_staging_identity(
        &self,
        staging: &OpenedMaterializationStaging,
    ) -> Result<(), MaterializationDestinationError> {
        let security = validate_owned_file(
            staging.file.as_fd(),
            self.owner_uid,
            self.parent_device,
            None,
        )?;
        if security.device != staging.device || security.inode != staging.inode {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        exact_final_component(staging.file.as_fd(), staging.name.as_bytes())
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)
    }

    fn remove_recovery_staging(
        &self,
        intent: &OpenedMaterializationIntent,
        staging: &OpenedMaterializationStaging,
        binding: &MaterializationIntentBinding,
    ) -> Result<(), MaterializationDestinationError> {
        self.revalidate_absent()?;
        self.validate_open_intent(intent, binding)?;
        self.validate_staging_identity(staging)?;
        unlinkat(
            self.parent_fd()?,
            staging.name.as_str(),
            rustix::fs::AtFlags::empty(),
        )
        .map_err(|_| MaterializationDestinationError::Io)?;
        fcntl_fullfsync(self.parent_fd()?).map_err(|_| MaterializationDestinationError::Io)?;
        self.validate_open_intent(intent, binding)?;
        self.revalidate_absent()
    }

    fn revalidate_parent(&self) -> Result<(), MaterializationDestinationError> {
        let parent = self
            .components
            .last()
            .ok_or(MaterializationDestinationError::UnsafeConfiguration)?;
        let security =
            inspect_destination_directory(parent.fd.as_fd(), parent.role, self.owner_uid)?;
        if security.device != self.parent_device
            || security.inode != self.parent_inode
            || !security.same_object(parent.security)
        {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        Ok(())
    }

    fn parent_fd(&self) -> Result<BorrowedFd<'_>, MaterializationDestinationError> {
        self.components
            .last()
            .map(|component| component.fd.as_fd())
            .ok_or(MaterializationDestinationError::UnsafeConfiguration)
    }
}

impl OpenedMaterializationRecovery {
    /// Continues an already classified prefix. Call only after the durable CAS reacquire.
    pub fn resume(
        self,
        blob_authority: &OpenedBlobRootAuthority,
        binding: &MaterializationIntentBinding,
        buffer_bytes: usize,
    ) -> Result<
        (
            OpenedMaterializationDestination,
            OpenedMaterializationIntent,
            OpenedMaterializedFile,
        ),
        MaterializationDestinationError,
    > {
        match self.resume_controlled(blob_authority, binding, buffer_bytes, || false)? {
            MaterializationResumeOutcome::Published(published) => {
                let (destination, intent, published) = *published;
                Ok((destination, intent, published))
            }
            MaterializationResumeOutcome::Stopped => {
                Err(MaterializationDestinationError::UnsafeConfiguration)
            }
        }
    }

    pub fn resume_controlled(
        self,
        blob_authority: &OpenedBlobRootAuthority,
        binding: &MaterializationIntentBinding,
        buffer_bytes: usize,
        mut should_stop: impl FnMut() -> bool,
    ) -> Result<MaterializationResumeOutcome, MaterializationDestinationError> {
        match self {
            Self::Published {
                destination,
                intent,
                published,
            } => {
                destination.revalidate_published(&published, binding, buffer_bytes)?;
                destination.validate_open_intent(&intent, binding)?;
                Ok(MaterializationResumeOutcome::Published(Box::new((
                    destination,
                    intent,
                    published,
                ))))
            }
            Self::BeforePublish {
                destination,
                intent,
                staging,
            } => {
                let intent = match intent {
                    Some(intent) => intent,
                    None => destination.create_intent(binding)?,
                };
                if let Some(staging) = staging {
                    destination.remove_recovery_staging(&intent, &staging, binding)?;
                }
                let staging = destination.create_staging(&intent, binding)?;
                let verified = match destination.copy_managed_blob_controlled(
                    blob_authority,
                    staging,
                    binding,
                    buffer_bytes,
                    &mut should_stop,
                )? {
                    MaterializationCopyOutcome::Verified(verified) => verified,
                    MaterializationCopyOutcome::Stopped(staging) => {
                        destination.cleanup_before_publish(&intent, Some(&staging), binding)?;
                        return Ok(MaterializationResumeOutcome::Stopped);
                    }
                };
                let published = destination.publish(&intent, binding, verified, buffer_bytes)?;
                Ok(MaterializationResumeOutcome::Published(Box::new((
                    destination,
                    intent,
                    published,
                ))))
            }
        }
    }
}

impl OpenedMaterializationStaging {
    /// Writes bytes at an explicit offset without exposing the staging descriptor or name.
    pub fn write_at(
        &self,
        bytes: &[u8],
        offset: u64,
    ) -> Result<(), MaterializationDestinationError> {
        write_all_at(self.file.as_fd(), bytes, offset)
    }
}

fn sidecar_names(command_id: [u8; 16]) -> (String, String) {
    use std::fmt::Write as _;

    let mut stem = String::with_capacity(SIDECAR_PREFIX.len() + 32);
    stem.push_str(".mengxia-materialize-");
    for byte in command_id {
        write!(&mut stem, "{byte:02x}").expect("writing to String cannot fail");
    }
    (format!("{stem}.intent"), format!("{stem}.staging"))
}

fn open_optional(
    parent: BorrowedFd<'_>,
    name: impl rustix::path::Arg,
) -> Result<Option<OwnedFd>, MaterializationDestinationError> {
    match openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    ) {
        Ok(fd) => Ok(Some(fd)),
        Err(rustix::io::Errno::NOENT) => Ok(None),
        Err(_) => Err(MaterializationDestinationError::UnsafeConfiguration),
    }
}

fn map_create_error(error: rustix::io::Errno) -> MaterializationDestinationError {
    match error {
        rustix::io::Errno::EXIST => MaterializationDestinationError::Conflict,
        _ => MaterializationDestinationError::Io,
    }
}

fn validate_created_file(
    fd: BorrowedFd<'_>,
    owner_uid: u32,
    parent_device: u64,
    expected_length: u64,
) -> Result<MacOsObjectSecurity, MaterializationDestinationError> {
    validate_owned_file(fd, owner_uid, parent_device, Some(expected_length))
}

fn validate_owned_file(
    fd: BorrowedFd<'_>,
    owner_uid: u32,
    parent_device: u64,
    expected_length: Option<u64>,
) -> Result<MacOsObjectSecurity, MaterializationDestinationError> {
    let security = super::inspect_internal_file_with_size(fd, owner_uid, expected_length)
        .map_err(map_authority_error)?;
    let stat = fstat(fd).map_err(|_| MaterializationDestinationError::Io)?;
    if security.device != parent_device || stat.st_nlink != 1 {
        return Err(MaterializationDestinationError::UnsafeConfiguration);
    }
    Ok(security)
}

fn write_all_at(
    fd: BorrowedFd<'_>,
    mut bytes: &[u8],
    mut offset: u64,
) -> Result<(), MaterializationDestinationError> {
    while !bytes.is_empty() {
        let written = retry_interrupted(|| pwrite(fd, bytes, offset))
            .map_err(|_| MaterializationDestinationError::Io)?;
        if written == 0 {
            return Err(MaterializationDestinationError::Io);
        }
        bytes = &bytes[written..];
        offset = offset
            .checked_add(written as u64)
            .ok_or(MaterializationDestinationError::Io)?;
    }
    Ok(())
}

fn validate_exact_record(
    fd: BorrowedFd<'_>,
    expected: &[u8; MATERIALIZATION_INTENT_BYTES],
) -> Result<(), MaterializationDestinationError> {
    let mut observed = [0_u8; MATERIALIZATION_INTENT_BYTES];
    let mut offset = 0_usize;
    while offset < observed.len() {
        let read = retry_interrupted(|| pread(fd, &mut observed[offset..], offset as u64))
            .map_err(|_| MaterializationDestinationError::Io)?;
        if read == 0 {
            return Err(MaterializationDestinationError::UnsafeConfiguration);
        }
        offset += read;
    }
    let mut trailing = [0_u8; 1];
    let trailing_read =
        retry_interrupted(|| pread(fd, &mut trailing, MATERIALIZATION_INTENT_BYTES as u64))
            .map_err(|_| MaterializationDestinationError::Io)?;
    if trailing_read != 0 || observed != *expected {
        return Err(MaterializationDestinationError::UnsafeConfiguration);
    }
    Ok(())
}

fn retry_interrupted<T>(
    mut operation: impl FnMut() -> rustix::io::Result<T>,
) -> rustix::io::Result<T> {
    let mut interruptions = 0;
    loop {
        match operation() {
            Err(rustix::io::Errno::INTR) if interruptions < MAX_INTERRUPTED_SYSCALL_RETRIES => {
                interruptions += 1;
            }
            result => return result,
        }
    }
}

fn map_blob_error(error: BlobFileError) -> MaterializationDestinationError {
    match error {
        BlobFileError::Io => MaterializationDestinationError::Io,
        BlobFileError::Corruption | BlobFileError::Modified => {
            MaterializationDestinationError::SourceCorruption
        }
        BlobFileError::InvalidPath
        | BlobFileError::UnsupportedType
        | BlobFileError::Configuration
        | BlobFileError::Collision
        | BlobFileError::CleanupFailed => MaterializationDestinationError::UnsafeConfiguration,
    }
}

fn verify_file_content(
    fd: BorrowedFd<'_>,
    expected_digest: [u8; 32],
    expected_length: u64,
    buffer_bytes: usize,
) -> Result<(), MaterializationDestinationError> {
    if !(1_048_576..=33_554_432).contains(&buffer_bytes) {
        return Err(MaterializationDestinationError::UnsafeConfiguration);
    }
    let before = file_snapshot(fd)?;
    if before.2 != expected_length {
        return Err(MaterializationDestinationError::SourceCorruption);
    }
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; buffer_bytes];
    let mut offset = 0_u64;
    while offset < expected_length {
        let remaining = usize::try_from((expected_length - offset).min(buffer_bytes as u64))
            .map_err(|_| MaterializationDestinationError::SourceCorruption)?;
        let read = retry_interrupted(|| pread(fd, &mut buffer[..remaining], offset))
            .map_err(|_| MaterializationDestinationError::Io)?;
        if read == 0 {
            return Err(MaterializationDestinationError::SourceCorruption);
        }
        hasher.update(&buffer[..read]);
        offset = offset
            .checked_add(read as u64)
            .ok_or(MaterializationDestinationError::SourceCorruption)?;
    }
    let mut trailing = [0_u8; 1];
    if retry_interrupted(|| pread(fd, &mut trailing, expected_length))
        .map_err(|_| MaterializationDestinationError::Io)?
        != 0
        || <[u8; 32]>::from(hasher.finalize()) != expected_digest
        || file_snapshot(fd)? != before
    {
        return Err(MaterializationDestinationError::SourceCorruption);
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
fn file_snapshot(
    fd: BorrowedFd<'_>,
) -> Result<(u64, u64, u64, u64, i64, i64, i64, i64), MaterializationDestinationError> {
    let stat = fstat(fd).map_err(|_| MaterializationDestinationError::Io)?;
    Ok((
        u64::try_from(stat.st_dev)
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?,
        stat.st_ino as u64,
        u64::try_from(stat.st_size)
            .map_err(|_| MaterializationDestinationError::UnsafeConfiguration)?,
        u64::from(stat.st_nlink),
        stat.st_mtime,
        stat.st_mtime_nsec,
        stat.st_ctime,
        stat.st_ctime_nsec,
    ))
}

fn encode_intent_record(
    binding: &MaterializationIntentBinding,
    name: &[u8],
    parent_device: u64,
    parent_inode: u64,
) -> Result<[u8; MATERIALIZATION_INTENT_BYTES], MaterializationDestinationError> {
    let name_length =
        u16::try_from(name.len()).map_err(|_| MaterializationDestinationError::InvalidPath)?;
    if name.is_empty() || name.len() > MAX_BASENAME_BYTES || name.contains(&0) {
        return Err(MaterializationDestinationError::InvalidPath);
    }
    let mut record = [0_u8; MATERIALIZATION_INTENT_BYTES];
    record[0..16].copy_from_slice(&INTENT_MAGIC);
    record[16..18].copy_from_slice(&INTENT_VERSION.to_be_bytes());
    record[18..20].copy_from_slice(&(MATERIALIZATION_INTENT_BYTES as u16).to_be_bytes());
    record[24..40].copy_from_slice(&binding.command_id);
    record[40..56].copy_from_slice(&binding.asset_id);
    record[56..72].copy_from_slice(&binding.asset_revision_id);
    record[72..88].copy_from_slice(&binding.representation_id);
    record[88..104].copy_from_slice(&binding.resource_id);
    record[104..108].copy_from_slice(&binding.member_ordinal.to_be_bytes());
    record[112..144].copy_from_slice(&binding.expected_digest);
    record[144..152].copy_from_slice(&binding.expected_length.to_be_bytes());
    record[152..160].copy_from_slice(&parent_device.to_be_bytes());
    record[160..168].copy_from_slice(&parent_inode.to_be_bytes());
    record[168..170].copy_from_slice(&name_length.to_be_bytes());
    record[170..170 + name.len()].copy_from_slice(name);
    record[432..464].copy_from_slice(&binding.request_digest);
    record[464..480].copy_from_slice(&binding.library_id);
    let checksum: [u8; 32] = Sha256::digest(&record[..INTENT_CHECKSUM_OFFSET]).into();
    record[INTENT_CHECKSUM_OFFSET..].copy_from_slice(&checksum);
    Ok(record)
}

fn validate_destination_path(path: &Path) -> Result<(), MaterializationDestinationError> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.is_empty()
        || bytes.len() > MAX_DESTINATION_BYTES
        || bytes.contains(&0)
        || !bytes.starts_with(b"/")
        || bytes == b"/"
        || bytes.ends_with(b"/")
        || bytes.windows(2).any(|window| window == b"//")
        || bytes
            .split(|byte| *byte == b'/')
            .skip(1)
            .any(|component| component == b"." || component == b"..")
    {
        return Err(MaterializationDestinationError::InvalidPath);
    }
    let mut saw_root = false;
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir if !saw_root && names.is_empty() => saw_root = true,
            Component::Normal(name)
                if saw_root && !name.is_empty() && name.as_bytes().len() <= MAX_BASENAME_BYTES =>
            {
                names.push(name)
            }
            _ => return Err(MaterializationDestinationError::InvalidPath),
        }
    }
    let final_name = names
        .last()
        .ok_or(MaterializationDestinationError::InvalidPath)?
        .as_bytes();
    if final_name == b"." || final_name == b".." || reserved_sidecar_name(final_name) {
        return Err(MaterializationDestinationError::InvalidPath);
    }
    Ok(())
}

fn reserved_sidecar_name(name: &[u8]) -> bool {
    if !name.starts_with(SIDECAR_PREFIX) {
        return false;
    }
    let suffix = if name.ends_with(INTENT_SUFFIX) {
        INTENT_SUFFIX
    } else if name.ends_with(STAGING_SUFFIX) {
        STAGING_SUFFIX
    } else {
        return false;
    };
    let middle = &name[SIDECAR_PREFIX.len()..name.len() - suffix.len()];
    middle.len() == 32
        && middle
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn inspect_destination_directory(
    fd: std::os::fd::BorrowedFd<'_>,
    role: ComponentRole,
    owner_uid: u32,
) -> Result<MacOsObjectSecurity, MaterializationDestinationError> {
    let security = inspect_directory(fd).map_err(map_authority_error)?;
    validate_component_policy(security, role, owner_uid).map_err(map_authority_error)?;
    Ok(security)
}

fn reject_excluded(
    security: MacOsObjectSecurity,
    excluded: [(u64, u64); 2],
) -> Result<(), MaterializationDestinationError> {
    if excluded.contains(&(security.device, security.inode)) {
        Err(MaterializationDestinationError::UnsafeConfiguration)
    } else {
        Ok(())
    }
}

fn map_authority_error(error: AuthorityError) -> MaterializationDestinationError {
    match error {
        AuthorityError::Io => MaterializationDestinationError::Io,
        AuthorityError::UnsafeConfiguration
        | AuthorityError::Contended
        | AuthorityError::ConflictingData => MaterializationDestinationError::UnsafeConfiguration,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::{BlobRootRequest, OpenedLibraryAuthority};

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
                .join("target/task-008-materialization")
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
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn binding_for(digest: [u8; 32], length: u64) -> MaterializationIntentBinding {
        MaterializationIntentBinding::new(
            [0x11; 16], [0x22; 16], [0x33; 16], [0x44; 16], [0x55; 16], 4095, digest, length,
            [0x77; 32], [0x88; 16],
        )
        .unwrap()
    }

    fn binding() -> MaterializationIntentBinding {
        binding_for([0x66; 32], 0x0000_0001_0203_0405)
    }

    #[test]
    fn lexical_path_and_sidecar_grammar_is_byte_exact() {
        for invalid in [
            &b""[..],
            &b"relative/file"[..],
            &b"/"[..],
            &b"/a/"[..],
            &b"/a//b"[..],
            &b"/a/./b"[..],
            &b"/a/../b"[..],
            &b"/a/.mengxia-materialize-0123456789abcdef0123456789abcdef.intent"[..],
            &b"/a/.mengxia-materialize-0123456789abcdef0123456789abcdef.staging"[..],
        ] {
            assert_eq!(
                validate_destination_path(Path::new(OsStr::from_bytes(invalid))),
                Err(MaterializationDestinationError::InvalidPath)
            );
        }
        assert!(
            validate_destination_path(Path::new(OsStr::from_bytes(b"/a/non-utf8-\xff"))).is_ok()
        );
    }

    #[test]
    fn intent_layout_checksum_and_every_identity_field_are_exact() {
        let binding = binding();
        let name = b"result-\xff.bin";
        let record =
            encode_intent_record(&binding, name, 0x0102_0304_0506_0708, 0x1112_1314_1516_1718)
                .unwrap();
        assert_eq!(&record[..16], &INTENT_MAGIC);
        assert_eq!(&record[16..20], &[0, 1, 2, 0]);
        assert_eq!(&record[20..24], &[0; 4]);
        assert_eq!(&record[24..40], &[0x11; 16]);
        assert_eq!(&record[40..56], &[0x22; 16]);
        assert_eq!(&record[56..72], &[0x33; 16]);
        assert_eq!(&record[72..88], &[0x44; 16]);
        assert_eq!(&record[88..104], &[0x55; 16]);
        assert_eq!(&record[104..108], &4095_u32.to_be_bytes());
        assert_eq!(&record[108..112], &[0; 4]);
        assert_eq!(&record[112..144], &[0x66; 32]);
        assert_eq!(&record[144..152], &0x0000_0001_0203_0405_u64.to_be_bytes());
        assert_eq!(&record[152..160], &0x0102_0304_0506_0708_u64.to_be_bytes());
        assert_eq!(&record[160..168], &0x1112_1314_1516_1718_u64.to_be_bytes());
        assert_eq!(&record[168..170], &(name.len() as u16).to_be_bytes());
        assert_eq!(&record[170..170 + name.len()], name);
        assert!(record[170 + name.len()..432].iter().all(|byte| *byte == 0));
        assert_eq!(&record[432..464], &[0x77; 32]);
        assert_eq!(&record[464..480], &[0x88; 16]);
        assert_eq!(&record[480..], Sha256::digest(&record[..480]).as_slice());
        for offset in [
            0, 16, 18, 20, 24, 40, 56, 72, 88, 104, 108, 112, 144, 152, 160, 168, 170, 425, 432,
            464, 480,
        ] {
            let mut changed = record;
            changed[offset] ^= 1;
            assert_ne!(changed, record);
            if offset < INTENT_CHECKSUM_OFFSET {
                assert_ne!(
                    Sha256::digest(&changed[..480]),
                    Sha256::digest(&record[..480])
                );
            } else {
                assert_ne!(&changed[480..], Sha256::digest(&changed[..480]).as_slice());
            }
        }
    }

    #[test]
    fn destination_authority_rejects_existing_and_managed_roots_and_revalidates_parent() {
        let fixture = Fixture::new();
        let library = fixture.root.join("Library");
        let destination_parent = fixture.root.join("output");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination_parent)
            .unwrap();
        let library_authority = OpenedLibraryAuthority::acquire_bootstrap(&library).unwrap();
        let blob_path = library.join("storage");
        let blob_request = BlobRootRequest::from_absolute_path(&blob_path).unwrap();
        let blob_authority = library_authority
            .authorize_blob_root(&blob_request, [0x88; 16])
            .unwrap();
        // Darwin rejects some byte sequences at the filesystem boundary even
        // though the lexical validator must remain byte-oriented.
        let final_path = destination_parent.join("result.bin");
        let destination =
            OpenedMaterializationDestination::authorize_new(&final_path, &blob_authority).unwrap();
        let record = destination.encode_intent(&binding()).unwrap();
        assert!(destination.validates_intent(&record, &binding()));
        let mut changed = record;
        changed[112] ^= 1;
        assert!(!destination.validates_intent(&changed, &binding()));
        fs::write(&final_path, b"occupied").unwrap();
        assert_eq!(
            destination.revalidate_absent(),
            Err(MaterializationDestinationError::Conflict)
        );
        assert_eq!(
            OpenedMaterializationDestination::authorize_new(
                &blob_path.join("forbidden"),
                &blob_authority
            )
            .err(),
            Some(MaterializationDestinationError::UnsafeConfiguration)
        );
    }

    #[test]
    fn intent_and_staging_are_exclusive_exact_and_descriptor_revalidated() {
        let fixture = Fixture::new();
        let library = fixture.root.join("Library");
        let destination_parent = fixture.root.join("output");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination_parent)
            .unwrap();
        let library_authority = OpenedLibraryAuthority::acquire_bootstrap(&library).unwrap();
        let blob_path = library.join("storage");
        let blob_request = BlobRootRequest::from_absolute_path(&blob_path).unwrap();
        let blob_authority = library_authority
            .authorize_blob_root(&blob_request, [0x88; 16])
            .unwrap();
        let destination = OpenedMaterializationDestination::authorize_new(
            &destination_parent.join("result.bin"),
            &blob_authority,
        )
        .unwrap();
        let binding = binding();
        let intent = destination.create_intent(&binding).unwrap();
        let intent_path =
            destination_parent.join(".mengxia-materialize-11111111111111111111111111111111.intent");
        assert_eq!(
            fs::read(&intent_path).unwrap(),
            destination.encode_intent(&binding).unwrap()
        );
        assert_eq!(
            fs::metadata(&intent_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            destination.create_intent(&binding).err(),
            Some(MaterializationDestinationError::Conflict)
        );

        let staging = destination.create_staging(&intent, &binding).unwrap();
        assert_eq!(
            destination.create_staging(&intent, &binding).err(),
            Some(MaterializationDestinationError::Conflict)
        );
        staging.write_at(b"bounded-payload", 0).unwrap();
        destination
            .revalidate_staging(&staging, b"bounded-payload".len() as u64)
            .unwrap();
        assert_eq!(
            destination.revalidate_staging(&staging, 1),
            Err(MaterializationDestinationError::UnsafeConfiguration)
        );
        assert_eq!(
            fs::read(
                destination_parent
                    .join(".mengxia-materialize-11111111111111111111111111111111.staging")
            )
            .unwrap(),
            b"bounded-payload"
        );
        destination
            .cleanup_before_publish(&intent, Some(&staging), &binding)
            .unwrap();
        assert!(fs::read_dir(&destination_parent).unwrap().next().is_none());
    }

    #[test]
    fn managed_blob_copy_publishes_no_replace_and_cleans_only_exact_intent() {
        let fixture = Fixture::new();
        let library = fixture.root.join("Library");
        let destination_parent = fixture.root.join("output");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination_parent)
            .unwrap();
        let library_authority = OpenedLibraryAuthority::acquire_bootstrap(&library).unwrap();
        let blob_path = library.join("storage");
        let blob_request = BlobRootRequest::from_absolute_path(&blob_path).unwrap();
        let blob_authority = library_authority
            .authorize_blob_root(&blob_request, [0x88; 16])
            .unwrap();
        let bytes = b"managed materialization payload";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let blob_staging = blob_authority.create_staging([0x99; 16]).unwrap();
        let mut written = 0_usize;
        while written < bytes.len() {
            written += blob_staging
                .write_at(&bytes[written..], written as u64)
                .unwrap();
        }
        blob_authority
            .commit_staging(&blob_staging, digest, bytes.len() as u64, 1_048_576)
            .unwrap();

        let final_path = destination_parent.join("result.bin");
        let destination =
            OpenedMaterializationDestination::authorize_new(&final_path, &blob_authority).unwrap();
        let binding = binding_for(digest, bytes.len() as u64);
        let intent = destination.create_intent(&binding).unwrap();
        let staging = destination.create_staging(&intent, &binding).unwrap();
        let verified = destination
            .copy_managed_blob(&blob_authority, staging, &binding, 1_048_576)
            .unwrap();
        let published = destination
            .publish(&intent, &binding, verified, 1_048_576)
            .unwrap();
        destination
            .revalidate_published(&published, &binding, 1_048_576)
            .unwrap();
        assert_eq!(fs::read(&final_path).unwrap(), bytes);
        assert_eq!(
            fs::metadata(&final_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(
            !destination_parent
                .join(".mengxia-materialize-11111111111111111111111111111111.staging")
                .exists()
        );
        destination
            .cleanup_intent_after_publish(&published, &intent, &binding, 1_048_576)
            .unwrap();
        assert!(
            !destination_parent
                .join(".mengxia-materialize-11111111111111111111111111111111.intent")
                .exists()
        );
    }

    #[test]
    fn publish_race_never_replaces_an_existing_target() {
        let fixture = Fixture::new();
        let library = fixture.root.join("Library");
        let destination_parent = fixture.root.join("output");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination_parent)
            .unwrap();
        let library_authority = OpenedLibraryAuthority::acquire_bootstrap(&library).unwrap();
        let blob_path = library.join("storage");
        let blob_request = BlobRootRequest::from_absolute_path(&blob_path).unwrap();
        let blob_authority = library_authority
            .authorize_blob_root(&blob_request, [0x88; 16])
            .unwrap();
        let digest: [u8; 32] = Sha256::digest([]).into();
        let blob_staging = blob_authority.create_staging([0xaa; 16]).unwrap();
        blob_authority
            .commit_staging(&blob_staging, digest, 0, 1_048_576)
            .unwrap();
        let final_path = destination_parent.join("result.bin");
        let destination =
            OpenedMaterializationDestination::authorize_new(&final_path, &blob_authority).unwrap();
        let binding = binding_for(digest, 0);
        let intent = destination.create_intent(&binding).unwrap();
        let staging = destination.create_staging(&intent, &binding).unwrap();
        let verified = destination
            .copy_managed_blob(&blob_authority, staging, &binding, 1_048_576)
            .unwrap();
        fs::write(&final_path, b"racing user content").unwrap();
        assert_eq!(
            destination
                .publish(&intent, &binding, verified, 1_048_576)
                .err(),
            Some(MaterializationDestinationError::Conflict)
        );
        assert_eq!(fs::read(&final_path).unwrap(), b"racing user content");
    }

    #[test]
    fn recovery_classifies_without_mutation_then_recopies_owned_partial_staging() {
        let fixture = Fixture::new();
        let library = fixture.root.join("Library");
        let destination_parent = fixture.root.join("output");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination_parent)
            .unwrap();
        let library_authority = OpenedLibraryAuthority::acquire_bootstrap(&library).unwrap();
        let blob_path = library.join("storage");
        let blob_request = BlobRootRequest::from_absolute_path(&blob_path).unwrap();
        let blob_authority = library_authority
            .authorize_blob_root(&blob_request, [0x88; 16])
            .unwrap();
        let bytes = b"recovered materialization payload";
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        let blob_staging = blob_authority.create_staging([0xbb; 16]).unwrap();
        blob_staging.write_at(bytes, 0).unwrap();
        blob_authority
            .commit_staging(&blob_staging, digest, bytes.len() as u64, 1_048_576)
            .unwrap();
        let final_path = destination_parent.join("result.bin");
        let binding = binding_for(digest, bytes.len() as u64);
        {
            let destination =
                OpenedMaterializationDestination::authorize_new(&final_path, &blob_authority)
                    .unwrap();
            let intent = destination.create_intent(&binding).unwrap();
            let staging = destination.create_staging(&intent, &binding).unwrap();
            staging.write_at(b"partial", 0).unwrap();
        }
        let intent_path =
            destination_parent.join(".mengxia-materialize-11111111111111111111111111111111.intent");
        let staging_path = destination_parent
            .join(".mengxia-materialize-11111111111111111111111111111111.staging");
        let intent_before = fs::read(&intent_path).unwrap();
        let staging_before = fs::read(&staging_path).unwrap();
        let recovery = OpenedMaterializationDestination::authorize_recovery(
            &final_path,
            &blob_authority,
            &binding,
            1_048_576,
        )
        .unwrap();
        assert_eq!(fs::read(&intent_path).unwrap(), intent_before);
        assert_eq!(fs::read(&staging_path).unwrap(), staging_before);
        let (destination, intent, published) = recovery
            .resume(&blob_authority, &binding, 1_048_576)
            .unwrap();
        assert_eq!(fs::read(&final_path).unwrap(), bytes);
        assert!(!staging_path.exists());
        destination
            .cleanup_intent_after_publish(&published, &intent, &binding, 1_048_576)
            .unwrap();
        assert!(!intent_path.exists());
    }

    #[test]
    fn recovery_rejects_untrusted_intent_without_mutation() {
        let fixture = Fixture::new();
        let library = fixture.root.join("Library");
        let destination_parent = fixture.root.join("output");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination_parent)
            .unwrap();
        let library_authority = OpenedLibraryAuthority::acquire_bootstrap(&library).unwrap();
        let blob_path = library.join("storage");
        let blob_request = BlobRootRequest::from_absolute_path(&blob_path).unwrap();
        let blob_authority = library_authority
            .authorize_blob_root(&blob_request, [0x88; 16])
            .unwrap();
        let intent_path =
            destination_parent.join(".mengxia-materialize-11111111111111111111111111111111.intent");
        fs::write(&intent_path, b"untrusted").unwrap();
        fs::set_permissions(&intent_path, fs::Permissions::from_mode(0o600)).unwrap();
        let before = fs::read(&intent_path).unwrap();
        assert_eq!(
            OpenedMaterializationDestination::authorize_recovery(
                &destination_parent.join("result.bin"),
                &blob_authority,
                &binding_for([0x66; 32], 0),
                1_048_576,
            )
            .err(),
            Some(MaterializationDestinationError::UnsafeConfiguration)
        );
        assert_eq!(fs::read(&intent_path).unwrap(), before);
    }

    #[test]
    fn completed_cleanup_removes_only_valid_intent_and_never_recreates_final() {
        let fixture = Fixture::new();
        let library = fixture.root.join("Library");
        let destination_parent = fixture.root.join("output");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&destination_parent)
            .unwrap();
        let library_authority = OpenedLibraryAuthority::acquire_bootstrap(&library).unwrap();
        let blob_path = library.join("storage");
        let blob_request = BlobRootRequest::from_absolute_path(&blob_path).unwrap();
        let blob_authority = library_authority
            .authorize_blob_root(&blob_request, [0x88; 16])
            .unwrap();
        let final_path = destination_parent.join("removed-by-user.bin");
        let binding = binding_for(Sha256::digest([]).into(), 0);
        let intent_path =
            destination_parent.join(".mengxia-materialize-11111111111111111111111111111111.intent");
        {
            let destination =
                OpenedMaterializationDestination::authorize_new(&final_path, &blob_authority)
                    .unwrap();
            destination.create_intent(&binding).unwrap();
        }
        assert!(intent_path.exists());
        OpenedMaterializationDestination::cleanup_completed(
            &final_path,
            &blob_authority,
            &binding,
            1_048_576,
        )
        .unwrap();
        assert!(!intent_path.exists());
        assert!(!final_path.exists());
        OpenedMaterializationDestination::cleanup_completed(
            &final_path,
            &blob_authority,
            &binding,
            1_048_576,
        )
        .unwrap();
        assert!(!final_path.exists());
    }
}
