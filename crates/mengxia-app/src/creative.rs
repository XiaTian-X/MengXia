use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use mengxia_domain::{
    Asset, AssetLifecycle, AssetRevision, CanonicalJson, ContentKind, PositiveRatio, Project,
    ProjectName, ProjectSpecification, Resolution, SubjectKind, SubjectName, Take, TakeReason,
    TakeTransition, WorkCode, WorkItem, WorkKind, WorkRevision, WorkSpecification,
};
use mengxia_ports::{
    ASSET_RESTORE_V1, ASSET_RETIRE_V1, ASSET_REVISION_CREATE_V1, AssetRevisionRepresentationInput,
    AssetStoreError, AssetUnitOfWork, Command, CommandBinding, CreateProjectCommand,
    CreateSubjectCommand, CreateTakeCommand, CreateWorkCommand, CreativeListPosition,
    CreativeListQuery, CreativePage, CreativeQueryPort, CreativeUnitOfWork,
    DeferredAssetLifecycleCommand, DeferredCreateAssetRevisionCommand, IngestControl,
    IngestDirective, IngestStop, InterruptibleSqliteControl, MutationOutcome, PROJECT_CREATE_V1,
    PROJECT_LIST_V1, PROJECT_SPEC_REVISE_V1, ProjectView, PureCommandValueSource,
    ReopenTakeCommand, ReviseProjectSpecCommand, ReviseWorkCommand, SUBJECT_CREATE_V1,
    SUBJECT_LIST_V1, SubjectView, TAKE_CREATE_V1, TAKE_LIST_V1, TAKE_REOPEN_V1, TAKE_TRANSITION_V1,
    TakeView, TransitionTakeCommand, WORK_CREATE_V1, WORK_LIST_V1, WORK_REVISE_V1, WorkView,
};
use mengxia_types::{Id, RevisionNo, Sha256Digest, Timestamp};
use serde::de::{DeserializeSeed, Error as _, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use sha2::{Digest as _, Sha256};

pub const PROJECT_POLICY_MAX_BYTES: usize = 65_536;
pub const WORK_SPECIFICATION_MAX_BYTES: usize = 262_144;
const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 4_096;
const MAX_CONTAINER_ITEMS: usize = 1_024;
const MAX_KEY_BYTES: usize = 64;
const MAX_STRING_BYTES: usize = 8_192;
const FLOAT_LOWER_EXCLUSIVE: f64 = -9_223_372_036_854_775_808_f64;
const FLOAT_UPPER_EXCLUSIVE: f64 = 18_446_744_073_709_551_616_f64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreativeInputError {
    InvalidJson,
}

impl fmt::Display for CreativeInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("creative JSON validation failed")
    }
}

impl std::error::Error for CreativeInputError {}

pub fn parse_project_policy(input: &[u8]) -> Result<CanonicalJson, CreativeInputError> {
    parse_canonical_object(input, PROJECT_POLICY_MAX_BYTES)
}

pub fn parse_work_specification(input: &[u8]) -> Result<CanonicalJson, CreativeInputError> {
    parse_canonical_object(input, WORK_SPECIFICATION_MAX_BYTES)
}

#[allow(clippy::too_many_arguments)]
pub fn parse_project_specification(
    resolution: Option<Resolution>,
    frame_rate: Option<PositiveRatio>,
    aspect_ratio: Option<PositiveRatio>,
    color: &[u8],
    audio: &[u8],
    quality: &[u8],
    privacy: &[u8],
) -> Result<ProjectSpecification, CreativeInputError> {
    let total = color
        .len()
        .checked_add(audio.len())
        .and_then(|value| value.checked_add(quality.len()))
        .and_then(|value| value.checked_add(privacy.len()))
        .ok_or(CreativeInputError::InvalidJson)?;
    if total > 262_144 {
        return Err(CreativeInputError::InvalidJson);
    }
    let color = parse_project_policy(color)?;
    let audio = parse_project_policy(audio)?;
    let quality = parse_project_policy(quality)?;
    let privacy = parse_project_policy(privacy)?;
    let mut digest = Sha256::new();
    digest.update(b"MENGXIA_PROJECT_POLICIES_V1\0");
    for value in [&color, &audio, &quality, &privacy] {
        digest.update(
            u32::try_from(value.bytes().len())
                .map_err(|_| CreativeInputError::InvalidJson)?
                .to_be_bytes(),
        );
        digest.update(value.bytes());
    }
    Ok(ProjectSpecification::new(
        resolution,
        frame_rate,
        aspect_ratio,
        color,
        audio,
        quality,
        privacy,
        Sha256Digest::from_bytes(digest.finalize().into()),
    ))
}

enum GeneratedIdentity {}

struct SystemPureCommandValues {
    control: Arc<dyn IngestControl>,
}

impl PureCommandValueSource for SystemPureCommandValues {
    fn next_uuid_v7(&self) -> Result<[u8; 16], AssetStoreError> {
        Id::<GeneratedIdentity>::try_new()
            .map(Id::to_bytes)
            .map_err(|_| AssetStoreError::IdGenerationUnavailable)
    }

    fn now(&self) -> Result<Timestamp, AssetStoreError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| AssetStoreError::IdGenerationUnavailable)?;
        Timestamp::from_unix_seconds_nanos(
            i64::try_from(duration.as_secs())
                .map_err(|_| AssetStoreError::IdGenerationUnavailable)?,
            duration.subsec_nanos(),
        )
        .map_err(|_| AssetStoreError::IdGenerationUnavailable)
    }

    fn checkpoint(&self) -> Result<(), AssetStoreError> {
        match self.control.checkpoint() {
            IngestDirective::Continue => Ok(()),
            IngestDirective::Stop(IngestStop::Cancelled) => {
                Err(AssetStoreError::OperationCancelled)
            }
            IngestDirective::Stop(IngestStop::DeadlineReached) => {
                Err(AssetStoreError::DeadlineExceeded)
            }
        }
    }
}

fn task009_digest(operation: mengxia_ports::OperationId, fields: &[Vec<u8>]) -> Sha256Digest {
    let mut digest = Sha256::new();
    digest.update(b"MENGXIA_TASK009_REQUEST_V1\0");
    let operation_bytes = operation.as_str().as_bytes();
    digest.update(
        u16::try_from(operation_bytes.len())
            .expect("fixed operation ID")
            .to_be_bytes(),
    );
    digest.update(operation_bytes);
    for (index, value) in fields.iter().enumerate() {
        digest.update([u8::try_from(index + 1).expect("bounded field registry")]);
        digest.update(
            u32::try_from(value.len())
                .expect("validated request field")
                .to_be_bytes(),
        );
        digest.update(value);
    }
    Sha256Digest::from_bytes(digest.finalize().into())
}

fn optional_pair(value: Option<(u32, u32)>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(9);
    match value {
        None => bytes.push(0),
        Some((left, right)) => {
            bytes.push(1);
            bytes.extend_from_slice(&left.to_be_bytes());
            bytes.extend_from_slice(&right.to_be_bytes());
        }
    }
    bytes
}

fn project_spec_fields(specification: &ProjectSpecification) -> Vec<Vec<u8>> {
    vec![
        optional_pair(
            specification
                .resolution()
                .map(|value| (u32::from(value.width()), u32::from(value.height()))),
        ),
        optional_pair(
            specification
                .frame_rate()
                .map(|value| (value.numerator(), value.denominator())),
        ),
        optional_pair(
            specification
                .aspect_ratio()
                .map(|value| (value.numerator(), value.denominator())),
        ),
        specification.color_policy().digest().to_bytes().to_vec(),
        specification.audio_policy().digest().to_bytes().to_vec(),
        specification.quality_policy().digest().to_bytes().to_vec(),
        specification.privacy_policy().digest().to_bytes().to_vec(),
    ]
}

fn id_set<T>(values: &[Id<T>]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + values.len() * 16);
    bytes.extend_from_slice(
        &u32::try_from(values.len())
            .expect("bounded set")
            .to_be_bytes(),
    );
    for value in values {
        bytes.extend_from_slice(&value.to_bytes());
    }
    bytes
}

fn values(control: Arc<dyn IngestControl>) -> Arc<dyn PureCommandValueSource> {
    Arc::new(SystemPureCommandValues { control })
}

fn ordered_ids<T>(items: &[Id<T>]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + items.len() * 16);
    bytes.extend_from_slice(
        &u32::try_from(items.len())
            .expect("bounded IDs")
            .to_be_bytes(),
    );
    for item in items {
        bytes.extend_from_slice(&item.to_bytes());
    }
    bytes
}

fn revision_graph_bytes(items: &[AssetRevisionRepresentationInput]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(
        &u32::try_from(items.len())
            .expect("bounded graph")
            .to_be_bytes(),
    );
    for representation in items {
        let purpose = representation.purpose().as_str().as_bytes();
        bytes.extend_from_slice(
            &u16::try_from(purpose.len())
                .expect("bounded token")
                .to_be_bytes(),
        );
        bytes.extend_from_slice(purpose);
        bytes.extend_from_slice(
            &u32::try_from(representation.resources().len())
                .expect("bounded graph")
                .to_be_bytes(),
        );
        for resource in representation.resources() {
            let kind = resource.kind().as_str().as_bytes();
            bytes.extend_from_slice(
                &u16::try_from(kind.len())
                    .expect("bounded token")
                    .to_be_bytes(),
            );
            bytes.extend_from_slice(kind);
            bytes.extend_from_slice(
                &u32::try_from(resource.members().len())
                    .expect("bounded graph")
                    .to_be_bytes(),
            );
            for member in resource.members() {
                let name = member.logical_name().as_str().as_bytes();
                bytes.extend_from_slice(
                    &u16::try_from(name.len())
                        .expect("bounded logical name")
                        .to_be_bytes(),
                );
                bytes.extend_from_slice(name);
                bytes.extend_from_slice(&member.blob_digest().to_bytes());
            }
        }
    }
    bytes
}

pub struct AssetMetadataCommandService {
    store: Arc<dyn AssetUnitOfWork>,
}

impl AssetMetadataCommandService {
    #[must_use]
    pub fn new(store: Arc<dyn AssetUnitOfWork>) -> Self {
        Self { store }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_revision(
        &self,
        command_id: Id<Command>,
        asset_id: Id<Asset>,
        expected_revision: RevisionNo,
        parents: Vec<Id<AssetRevision>>,
        content_kind: ContentKind,
        representations: Vec<AssetRevisionRepresentationInput>,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let fields = vec![
            asset_id.to_bytes().to_vec(),
            expected_revision.get().to_be_bytes().to_vec(),
            ordered_ids(&parents),
            content_kind.as_str().as_bytes().to_vec(),
            revision_graph_bytes(&representations),
        ];
        let binding = CommandBinding::new(
            command_id,
            ASSET_REVISION_CREATE_V1,
            task009_digest(ASSET_REVISION_CREATE_V1, &fields),
        );
        self.store
            .execute_deferred_create_revision(DeferredCreateAssetRevisionCommand::new(
                binding,
                asset_id,
                expected_revision,
                parents,
                content_kind,
                representations,
                values(control),
            )?)
            .await
    }

    pub async fn change_lifecycle(
        &self,
        command_id: Id<Command>,
        asset_id: Id<Asset>,
        expected_revision: RevisionNo,
        target: AssetLifecycle,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let operation = match target {
            AssetLifecycle::Retired => ASSET_RETIRE_V1,
            AssetLifecycle::Active => ASSET_RESTORE_V1,
        };
        let fields = vec![
            asset_id.to_bytes().to_vec(),
            expected_revision.get().to_be_bytes().to_vec(),
        ];
        let binding =
            CommandBinding::new(command_id, operation, task009_digest(operation, &fields));
        self.store
            .execute_deferred_asset_lifecycle(DeferredAssetLifecycleCommand::new(
                binding,
                asset_id,
                expected_revision,
                target,
                values(control),
            )?)
            .await
    }
}

pub struct CreativeCommandService {
    store: Arc<dyn CreativeUnitOfWork>,
}

impl CreativeCommandService {
    #[must_use]
    pub fn new(store: Arc<dyn CreativeUnitOfWork>) -> Self {
        Self { store }
    }

    pub async fn create_project(
        &self,
        command_id: Id<Command>,
        name: ProjectName,
        specification: ProjectSpecification,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let mut fields = vec![name.as_str().as_bytes().to_vec()];
        fields.extend(project_spec_fields(&specification));
        let binding = CommandBinding::new(
            command_id,
            PROJECT_CREATE_V1,
            task009_digest(PROJECT_CREATE_V1, &fields),
        );
        self.store
            .execute_create_project(CreateProjectCommand::new(
                binding,
                name,
                specification,
                values(control),
            )?)
            .await
    }

    pub async fn revise_project_spec(
        &self,
        command_id: Id<Command>,
        project_id: Id<Project>,
        expected_revision: RevisionNo,
        specification: ProjectSpecification,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let mut fields = vec![
            project_id.to_bytes().to_vec(),
            expected_revision.get().to_be_bytes().to_vec(),
        ];
        fields.extend(project_spec_fields(&specification));
        let binding = CommandBinding::new(
            command_id,
            PROJECT_SPEC_REVISE_V1,
            task009_digest(PROJECT_SPEC_REVISE_V1, &fields),
        );
        self.store
            .execute_revise_project_spec(ReviseProjectSpecCommand::new(
                binding,
                project_id.to_bytes(),
                expected_revision,
                specification,
                values(control),
            )?)
            .await
    }

    pub async fn create_subject(
        &self,
        command_id: Id<Command>,
        kind: SubjectKind,
        name: SubjectName,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let fields = vec![
            kind.as_str().as_bytes().to_vec(),
            name.as_str().as_bytes().to_vec(),
        ];
        let binding = CommandBinding::new(
            command_id,
            SUBJECT_CREATE_V1,
            task009_digest(SUBJECT_CREATE_V1, &fields),
        );
        self.store
            .execute_create_subject(CreateSubjectCommand::new(
                binding,
                kind,
                name,
                values(control),
            )?)
            .await
    }

    pub async fn create_work(
        &self,
        command_id: Id<Command>,
        project_id: Id<Project>,
        kind: WorkKind,
        code: WorkCode,
        specification: WorkSpecification,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let fields = vec![
            project_id.to_bytes().to_vec(),
            vec![kind.code()],
            code.as_str().as_bytes().to_vec(),
            specification.json().digest().to_bytes().to_vec(),
            id_set(specification.subject_ids()),
            id_set(specification.asset_ids()),
        ];
        let binding = CommandBinding::new(
            command_id,
            WORK_CREATE_V1,
            task009_digest(WORK_CREATE_V1, &fields),
        );
        self.store
            .execute_create_work(CreateWorkCommand::new(
                binding,
                project_id,
                kind,
                code,
                specification,
                values(control),
            )?)
            .await
    }

    pub async fn revise_work(
        &self,
        command_id: Id<Command>,
        project_id: Id<Project>,
        work_item_id: Id<WorkItem>,
        expected_revision: RevisionNo,
        specification: WorkSpecification,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let fields = vec![
            project_id.to_bytes().to_vec(),
            work_item_id.to_bytes().to_vec(),
            expected_revision.get().to_be_bytes().to_vec(),
            specification.json().digest().to_bytes().to_vec(),
            id_set(specification.subject_ids()),
            id_set(specification.asset_ids()),
        ];
        let binding = CommandBinding::new(
            command_id,
            WORK_REVISE_V1,
            task009_digest(WORK_REVISE_V1, &fields),
        );
        self.store
            .execute_revise_work(ReviseWorkCommand::new(
                binding,
                project_id,
                work_item_id,
                expected_revision,
                specification,
                values(control),
            )?)
            .await
    }

    pub async fn create_take(
        &self,
        command_id: Id<Command>,
        project_id: Id<Project>,
        work_item_id: Id<WorkItem>,
        work_revision_id: Id<WorkRevision>,
        asset_id: Id<Asset>,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let fields = vec![
            project_id.to_bytes().to_vec(),
            work_item_id.to_bytes().to_vec(),
            work_revision_id.to_bytes().to_vec(),
            asset_id.to_bytes().to_vec(),
        ];
        let binding = CommandBinding::new(
            command_id,
            TAKE_CREATE_V1,
            task009_digest(TAKE_CREATE_V1, &fields),
        );
        self.store
            .execute_create_take(CreateTakeCommand::new(
                binding,
                project_id,
                work_item_id,
                work_revision_id,
                asset_id,
                values(control),
            )?)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn transition_take(
        &self,
        command_id: Id<Command>,
        project_id: Id<Project>,
        work_item_id: Id<WorkItem>,
        work_revision_id: Id<WorkRevision>,
        take_id: Id<Take>,
        expected_revision: RevisionNo,
        transition: TakeTransition,
        reason: Option<TakeReason>,
        related: Option<(Id<Take>, RevisionNo)>,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let mut optional_reason = vec![u8::from(reason.is_some())];
        if let Some(reason) = &reason {
            optional_reason.extend_from_slice(reason.as_str().as_bytes());
        }
        let mut optional_related = vec![u8::from(related.is_some())];
        if let Some((id, revision)) = related {
            optional_related.extend_from_slice(&id.to_bytes());
            optional_related.extend_from_slice(&revision.get().to_be_bytes());
        }
        let fields = vec![
            project_id.to_bytes().to_vec(),
            work_item_id.to_bytes().to_vec(),
            work_revision_id.to_bytes().to_vec(),
            take_id.to_bytes().to_vec(),
            expected_revision.get().to_be_bytes().to_vec(),
            vec![transition.code()],
            optional_reason,
            optional_related,
        ];
        let binding = CommandBinding::new(
            command_id,
            TAKE_TRANSITION_V1,
            task009_digest(TAKE_TRANSITION_V1, &fields),
        );
        self.store
            .execute_transition_take(TransitionTakeCommand::new(
                binding,
                project_id,
                work_item_id,
                work_revision_id,
                take_id,
                expected_revision,
                transition,
                reason,
                related,
                values(control),
            )?)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn reopen_take(
        &self,
        command_id: Id<Command>,
        project_id: Id<Project>,
        work_item_id: Id<WorkItem>,
        work_revision_id: Id<WorkRevision>,
        terminal_take_id: Id<Take>,
        expected_revision: RevisionNo,
        new_asset_id: Id<Asset>,
        control: Arc<dyn IngestControl>,
    ) -> Result<MutationOutcome, AssetStoreError> {
        let fields = vec![
            project_id.to_bytes().to_vec(),
            work_item_id.to_bytes().to_vec(),
            work_revision_id.to_bytes().to_vec(),
            terminal_take_id.to_bytes().to_vec(),
            expected_revision.get().to_be_bytes().to_vec(),
            new_asset_id.to_bytes().to_vec(),
        ];
        let binding = CommandBinding::new(
            command_id,
            TAKE_REOPEN_V1,
            task009_digest(TAKE_REOPEN_V1, &fields),
        );
        self.store
            .execute_reopen_take(ReopenTakeCommand::new(
                binding,
                project_id,
                work_item_id,
                work_revision_id,
                terminal_take_id,
                expected_revision,
                new_asset_id,
                values(control),
            )?)
            .await
    }
}

const CREATIVE_CURSOR_LENGTH: usize = 160;
const CREATIVE_CURSOR_PREFIX_LENGTH: usize = 128;

#[derive(Clone, Copy)]
enum CreativeCursorKind {
    Projects,
    Subjects,
    Work(Id<Project>),
    Takes(Id<WorkRevision>),
}

impl CreativeCursorKind {
    const fn magic(self) -> [u8; 16] {
        match self {
            Self::Projects => *b"MX9_PROJECTS_V1\0",
            Self::Subjects => *b"MX9_SUBJECTS_V1\0",
            Self::Work(_) => *b"MX9_WORKLIST_V1\0",
            Self::Takes(_) => *b"MX9_TAKELIST_V1\0",
        }
    }

    const fn operation(self) -> mengxia_ports::OperationId {
        match self {
            Self::Projects => PROJECT_LIST_V1,
            Self::Subjects => SUBJECT_LIST_V1,
            Self::Work(_) => WORK_LIST_V1,
            Self::Takes(_) => TAKE_LIST_V1,
        }
    }

    const fn scope(self) -> (u8, [u8; 16]) {
        match self {
            Self::Projects | Self::Subjects => (0, [0; 16]),
            Self::Work(id) => (1, id.to_bytes()),
            Self::Takes(id) => (2, id.to_bytes()),
        }
    }
}

fn creative_filter_digest(kind: CreativeCursorKind) -> [u8; 32] {
    let (scope, scope_id) = kind.scope();
    let mut hash = Sha256::new();
    hash.update(b"MENGXIA_");
    hash.update(kind.operation().as_str().as_bytes());
    hash.update(b"_FILTER_V1\0");
    hash.update([scope]);
    hash.update(scope_id);
    hash.finalize().into()
}

fn encode_creative_cursor(
    position: CreativeListPosition,
    library_id: [u8; 16],
    kind: CreativeCursorKind,
) -> Result<[u8; CREATIVE_CURSOR_LENGTH], AssetStoreError> {
    let CreativeListPosition::After {
        library_id: stored,
        snapshot_endpoint,
        last_key,
    } = position
    else {
        return Err(AssetStoreError::Internal);
    };
    if stored != library_id {
        return Err(AssetStoreError::Internal);
    }
    let mut bytes = [0_u8; CREATIVE_CURSOR_LENGTH];
    bytes[..16].copy_from_slice(&kind.magic());
    bytes[16..18].copy_from_slice(&1_u16.to_be_bytes());
    bytes[18..20].copy_from_slice(&(CREATIVE_CURSOR_LENGTH as u16).to_be_bytes());
    bytes[24..40].copy_from_slice(&library_id);
    let (scope, scope_id) = kind.scope();
    bytes[40] = scope;
    bytes[48..64].copy_from_slice(&scope_id);
    bytes[64..72].copy_from_slice(&snapshot_endpoint.to_be_bytes());
    bytes[72..80].copy_from_slice(&last_key.to_be_bytes());
    bytes[80..112].copy_from_slice(&creative_filter_digest(kind));
    let checksum: [u8; 32] = Sha256::digest(&bytes[..CREATIVE_CURSOR_PREFIX_LENGTH]).into();
    bytes[128..].copy_from_slice(&checksum);
    Ok(bytes)
}

fn decode_creative_cursor(
    bytes: &[u8],
    library_id: [u8; 16],
    kind: CreativeCursorKind,
) -> Result<CreativeListPosition, AssetStoreError> {
    let bytes: &[u8; CREATIVE_CURSOR_LENGTH] =
        bytes.try_into().map_err(|_| AssetStoreError::Validation)?;
    let (scope, scope_id) = kind.scope();
    if bytes[..16] != kind.magic()
        || u16::from_be_bytes(bytes[16..18].try_into().expect("fixed cursor")) != 1
        || u16::from_be_bytes(bytes[18..20].try_into().expect("fixed cursor"))
            != CREATIVE_CURSOR_LENGTH as u16
        || bytes[20..24].iter().any(|byte| *byte != 0)
        || bytes[24..40] != library_id
        || bytes[40] != scope
        || bytes[41..48].iter().any(|byte| *byte != 0)
        || bytes[48..64] != scope_id
        || bytes[80..112] != creative_filter_digest(kind)
        || bytes[112..128].iter().any(|byte| *byte != 0)
        || Sha256::digest(&bytes[..128]).as_slice() != &bytes[128..]
    {
        return Err(AssetStoreError::Validation);
    }
    CreativeListPosition::after(
        library_id,
        u64::from_be_bytes(bytes[64..72].try_into().expect("fixed cursor")),
        u64::from_be_bytes(bytes[72..80].try_into().expect("fixed cursor")),
    )
}

pub struct CreativePageResponse<T> {
    page: CreativePage<T>,
    next_cursor: Option<[u8; CREATIVE_CURSOR_LENGTH]>,
}

impl<T> CreativePageResponse<T> {
    #[must_use]
    pub const fn page(&self) -> &CreativePage<T> {
        &self.page
    }
    #[must_use]
    pub const fn next_cursor(&self) -> Option<&[u8; CREATIVE_CURSOR_LENGTH]> {
        self.next_cursor.as_ref()
    }
}

pub struct CreativeQueryService<P> {
    port: Arc<P>,
    library_id: [u8; 16],
}

impl<P: CreativeQueryPort> CreativeQueryService<P> {
    pub fn new(port: Arc<P>, library_id: [u8; 16]) -> Result<Self, AssetStoreError> {
        if library_id == [0; 16] {
            return Err(AssetStoreError::Validation);
        }
        Ok(Self { port, library_id })
    }

    fn position(
        &self,
        cursor: Option<&[u8]>,
        kind: CreativeCursorKind,
    ) -> Result<CreativeListPosition, AssetStoreError> {
        match cursor {
            None | Some([]) => Ok(CreativeListPosition::First),
            Some(cursor) => decode_creative_cursor(cursor, self.library_id, kind),
        }
    }

    fn response<T>(
        &self,
        page: CreativePage<T>,
        kind: CreativeCursorKind,
    ) -> Result<CreativePageResponse<T>, AssetStoreError> {
        let next_cursor = page
            .next()
            .map(|next| encode_creative_cursor(next, self.library_id, kind))
            .transpose()?;
        Ok(CreativePageResponse { page, next_cursor })
    }

    pub async fn list_projects(
        &self,
        page_size: u32,
        cursor: Option<&[u8]>,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> Result<CreativePageResponse<ProjectView>, AssetStoreError> {
        let kind = CreativeCursorKind::Projects;
        let page = self
            .port
            .list_projects(
                CreativeListQuery::new(page_size, self.position(cursor, kind)?)?,
                control,
            )
            .await?;
        self.response(page, kind)
    }
    pub async fn list_subjects(
        &self,
        page_size: u32,
        cursor: Option<&[u8]>,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> Result<CreativePageResponse<SubjectView>, AssetStoreError> {
        let kind = CreativeCursorKind::Subjects;
        let page = self
            .port
            .list_subjects(
                CreativeListQuery::new(page_size, self.position(cursor, kind)?)?,
                control,
            )
            .await?;
        self.response(page, kind)
    }
    pub async fn list_work(
        &self,
        project_id: Id<Project>,
        page_size: u32,
        cursor: Option<&[u8]>,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> Result<CreativePageResponse<WorkView>, AssetStoreError> {
        let kind = CreativeCursorKind::Work(project_id);
        let page = self
            .port
            .list_work(
                project_id,
                CreativeListQuery::new(page_size, self.position(cursor, kind)?)?,
                control,
            )
            .await?;
        self.response(page, kind)
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn list_takes(
        &self,
        project_id: Id<Project>,
        work_item_id: Id<WorkItem>,
        work_revision_id: Id<WorkRevision>,
        page_size: u32,
        cursor: Option<&[u8]>,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> Result<CreativePageResponse<TakeView>, AssetStoreError> {
        let kind = CreativeCursorKind::Takes(work_revision_id);
        let page = self
            .port
            .list_takes(
                project_id,
                work_item_id,
                work_revision_id,
                CreativeListQuery::new(page_size, self.position(cursor, kind)?)?,
                control,
            )
            .await?;
        self.response(page, kind)
    }
}

fn parse_canonical_object(
    input: &[u8],
    maximum_bytes: usize,
) -> Result<CanonicalJson, CreativeInputError> {
    if !(2..=maximum_bytes).contains(&input.len()) {
        return Err(CreativeInputError::InvalidJson);
    }
    let mut budget = Budget { nodes: 0 };
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let value = ValueSeed {
        budget: &mut budget,
        depth: 1,
    }
    .deserialize(&mut deserializer)
    .map_err(|_| CreativeInputError::InvalidJson)?;
    deserializer
        .end()
        .map_err(|_| CreativeInputError::InvalidJson)?;
    if !value.is_object() {
        return Err(CreativeInputError::InvalidJson);
    }
    let canonical = serde_json::to_vec(&value).map_err(|_| CreativeInputError::InvalidJson)?;
    if !(2..=maximum_bytes).contains(&canonical.len()) {
        return Err(CreativeInputError::InvalidJson);
    }
    let reparsed: Value =
        serde_json::from_slice(&canonical).map_err(|_| CreativeInputError::InvalidJson)?;
    if reparsed != value {
        return Err(CreativeInputError::InvalidJson);
    }
    let digest = Sha256Digest::from_bytes(Sha256::digest(&canonical).into());
    CanonicalJson::__from_validated_object(canonical, digest, maximum_bytes)
        .map_err(|_| CreativeInputError::InvalidJson)
}

struct Budget {
    nodes: usize,
}

impl Budget {
    fn claim(&mut self) -> Result<(), &'static str> {
        self.nodes = self.nodes.checked_add(1).ok_or("node overflow")?;
        if self.nodes > MAX_NODES {
            Err("too many nodes")
        } else {
            Ok(())
        }
    }
}

struct ValueSeed<'a> {
    budget: &'a mut Budget,
    depth: usize,
}

impl<'de> DeserializeSeed<'de> for ValueSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        self.budget.claim().map_err(D::Error::custom)?;
        if self.depth > MAX_DEPTH {
            return Err(D::Error::custom("JSON depth exceeded"));
        }
        deserializer.deserialize_any(ValueVisitor {
            budget: self.budget,
            depth: self.depth,
        })
    }
}

struct ValueVisitor<'a> {
    budget: &'a mut Budget,
    depth: usize,
}

impl<'de> Visitor<'de> for ValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded canonical JSON")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(Value::Number(Number::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if !value.is_finite() || value <= FLOAT_LOWER_EXCLUSIVE || value >= FLOAT_UPPER_EXCLUSIVE {
            return Err(E::custom("number outside accepted envelope"));
        }
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if value.len() > MAX_STRING_BYTES {
            return Err(E::custom("string too long"));
        }
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        self.visit_str(&value)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(ValueSeed {
            budget: self.budget,
            depth: self.depth + 1,
        })? {
            if values.len() == MAX_CONTAINER_ITEMS {
                return Err(A::Error::custom("array too long"));
            }
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = Map::new();
        let mut seen = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if values.len() == MAX_CONTAINER_ITEMS
                || key.len() > MAX_KEY_BYTES
                || key.chars().any(char::is_control)
                || !seen.insert(key.clone())
            {
                return Err(A::Error::custom("invalid or duplicate object key"));
            }
            let value = map.next_value_seed(ValueSeed {
                budget: self.budget,
                depth: self.depth + 1,
            })?;
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_id<T>(tail: u8) -> Id<T> {
        let mut bytes = [
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x00,
        ];
        bytes[15] = tail;
        Id::from_bytes(bytes).expect("fixed UUIDv7")
    }

    #[test]
    fn canonicalization_orders_keys_and_rejects_duplicate_or_trailing_input() {
        let parsed = parse_project_policy(br#"{"z":1,"a":[true,null]}"#).unwrap();
        assert_eq!(parsed.bytes(), br#"{"a":[true,null],"z":1}"#);
        assert!(parse_project_policy(br#"{"a":1,"a":2}"#).is_err());
        assert!(parse_project_policy(br#"{} {}"#).is_err());
    }

    #[test]
    fn numeric_and_structural_boundaries_fail_closed() {
        assert!(parse_project_policy(br#"{"n":18446744073709551616}"#).is_err());
        assert!(parse_project_policy(br#"{"n":-9223372036854775809}"#).is_err());
        assert!(parse_project_policy(br#"[]"#).is_err());
        assert!(parse_project_policy(b"{\"a\":").is_err());
        let deep = format!("{{\"a\":{} }}", "[".repeat(32) + &"]".repeat(32));
        assert!(parse_project_policy(deep.as_bytes()).is_err());
    }

    #[test]
    fn creative_cursor_is_exactly_bound_to_generation_library_operation_and_scope() {
        let library = fixed_id::<()>(1).to_bytes();
        let project = fixed_id::<Project>(2);
        let position = CreativeListPosition::after(library, 9, 4).unwrap();
        let cursor =
            encode_creative_cursor(position, library, CreativeCursorKind::Work(project)).unwrap();
        assert_eq!(&cursor[..16], b"MX9_WORKLIST_V1\0");
        assert_eq!(&cursor[16..20], &[0, 1, 0, 160]);
        assert_eq!(cursor[40], 1);
        assert_eq!(&cursor[48..64], &project.to_bytes());
        assert_eq!(
            decode_creative_cursor(&cursor, library, CreativeCursorKind::Work(project)),
            Ok(position)
        );

        assert_eq!(
            decode_creative_cursor(
                &cursor,
                fixed_id::<()>(3).to_bytes(),
                CreativeCursorKind::Work(project),
            ),
            Err(AssetStoreError::Validation)
        );
        assert_eq!(
            decode_creative_cursor(&cursor, library, CreativeCursorKind::Projects),
            Err(AssetStoreError::Validation)
        );
        assert_eq!(
            decode_creative_cursor(
                &cursor,
                library,
                CreativeCursorKind::Work(fixed_id::<Project>(4)),
            ),
            Err(AssetStoreError::Validation)
        );

        for offset in [0, 16, 18, 20, 24, 40, 41, 48, 64, 72, 80, 112, 128, 159] {
            let mut damaged = cursor;
            damaged[offset] ^= 1;
            assert_eq!(
                decode_creative_cursor(&damaged, library, CreativeCursorKind::Work(project)),
                Err(AssetStoreError::Validation),
                "cursor byte {offset} must be authenticated or reserved"
            );
        }
    }
}
