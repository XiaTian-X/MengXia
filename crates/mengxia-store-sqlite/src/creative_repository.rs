use mengxia_domain::{
    Project, ProjectSpecRevision, Relationship, Subject, Take, TakeState, TakeTransition, WorkItem,
    WorkRevision,
};
use mengxia_events::{DomainEvent, EventPayload};
use mengxia_ports::{
    AssetPortFuture, AssetStoreError, CommandBinding, CommandResult, CreateProjectCommand,
    CreateSubjectCommand, CreateTakeCommand, CreateWorkCommand, CreativeUnitOfWork,
    MutationOutcome, PureCommandValueSource, ReopenTakeCommand, ReviseProjectSpecCommand,
    ReviseWorkCommand, TransitionTakeCommand, VersionedCommandResult, VersionedResultPayload,
};
use mengxia_types::{ErrorCode, Id, RevisionNo, Timestamp};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest as _, Sha256};

use crate::asset_repository::{
    CommandRow, SqliteAssetStoreHandle, allocate_event_sequences, binding_matches,
    commit_rejection, insert_claim, parse_revision, read_command, replay_pure, revision_bytes,
    sqlite, validate_command_row,
};

impl CreativeUnitOfWork for SqliteAssetStoreHandle {
    fn execute_create_project(
        &self,
        request: CreateProjectCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = self.context();
        self.submit(move |connection| create_project(connection, context, request))
    }

    fn execute_revise_project_spec(
        &self,
        request: ReviseProjectSpecCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = self.context();
        self.submit(move |connection| revise_project_spec(connection, context, request))
    }

    fn execute_create_subject(
        &self,
        request: CreateSubjectCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = self.context();
        self.submit(move |connection| create_subject(connection, context, request))
    }

    fn execute_create_work(
        &self,
        request: CreateWorkCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = self.context();
        self.submit(move |connection| create_work(connection, context, request))
    }

    fn execute_revise_work(
        &self,
        request: ReviseWorkCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = self.context();
        self.submit(move |connection| revise_work(connection, context, request))
    }

    fn execute_create_take(
        &self,
        request: CreateTakeCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = self.context();
        self.submit(move |connection| create_take(connection, context, request))
    }

    fn execute_transition_take(
        &self,
        request: TransitionTakeCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = self.context();
        self.submit(move |connection| transition_take(connection, context, request))
    }

    fn execute_reopen_take(
        &self,
        request: ReopenTakeCommand,
    ) -> AssetPortFuture<'_, MutationOutcome> {
        let context = self.context();
        self.submit(move |connection| reopen_take(connection, context, request))
    }
}

fn generated_id<T>(source: &dyn PureCommandValueSource) -> Result<Id<T>, AssetStoreError> {
    Id::from_bytes(source.next_uuid_v7()?).map_err(|_| AssetStoreError::IdGenerationUnavailable)
}

fn distinct(values: &[[u8; 16]]) -> Result<(), AssetStoreError> {
    if values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[index + 1..].contains(value))
    {
        Ok(())
    } else {
        Err(AssetStoreError::IdGenerationUnavailable)
    }
}

fn replay_existing(
    transaction: &Transaction<'_>,
    context: crate::asset_repository::StoreContext,
    binding: &CommandBinding,
    expected_kind: &str,
) -> Result<Option<MutationOutcome>, AssetStoreError> {
    let Some(row) = read_command(transaction, binding)? else {
        return Ok(None);
    };
    if !binding_matches(&row, binding, context.metadata.owner_uid) {
        return Err(AssetStoreError::Conflict);
    }
    Ok(Some(replay_pure(
        transaction,
        validate_command_row(row)?,
        expected_kind,
    )?))
}

fn insert_project_specification(
    transaction: &Transaction<'_>,
    specification_id: [u8; 16],
    project_id: [u8; 16],
    sequence: u32,
    command_id: [u8; 16],
    at: Timestamp,
    specification: &mengxia_domain::ProjectSpecification,
) -> Result<(), AssetStoreError> {
    let resolution = specification.resolution();
    let frame_rate = specification.frame_rate();
    let aspect_ratio = specification.aspect_ratio();
    transaction
        .execute(
            "INSERT INTO project_spec_revisions (project_spec_revision_id, project_id, sequence, resolution_width, resolution_height, frame_rate_numerator, frame_rate_denominator, aspect_ratio_numerator, aspect_ratio_denominator, policy_schema_version, color_policy_json, audio_policy_json, quality_policy_json, privacy_policy_json, policy_digest, created_by_command_id, created_at_seconds, created_at_nanos) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                specification_id.as_slice(),
                project_id.as_slice(),
                i64::from(sequence),
                resolution.map(|value| i64::from(value.width())),
                resolution.map(|value| i64::from(value.height())),
                frame_rate.map(|value| i64::from(value.numerator())),
                frame_rate.map(|value| i64::from(value.denominator())),
                aspect_ratio.map(|value| i64::from(value.numerator())),
                aspect_ratio.map(|value| i64::from(value.denominator())),
                specification.color_policy().bytes(),
                specification.audio_policy().bytes(),
                specification.quality_policy().bytes(),
                specification.privacy_policy().bytes(),
                specification.policy_digest().to_bytes().as_slice(),
                command_id.as_slice(),
                at.unix_seconds(),
                i64::from(at.subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    Ok(())
}

// Keeping the complete ledger tuple visible at each call site makes omissions in
// the security-critical event write auditable; these are not optional parameters.
#[allow(clippy::too_many_arguments)]
fn insert_domain_event(
    transaction: &Transaction<'_>,
    event_id: [u8; 16],
    sequence: i64,
    binding: &CommandBinding,
    event_type: &str,
    aggregate_kind: &str,
    aggregate_id: [u8; 16],
    aggregate_revision: u64,
    at: Timestamp,
) -> Result<(), AssetStoreError> {
    transaction
        .execute(
            "INSERT INTO domain_events (domain_event_id, commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, event_payload, event_payload_sha256, occurred_at_seconds, occurred_at_nanos) VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, ?7, NULL, NULL, ?8, ?9)",
            params![
                event_id.as_slice(),
                sequence,
                binding.command_id().to_bytes().as_slice(),
                event_type,
                aggregate_kind,
                aggregate_id.as_slice(),
                revision_bytes(aggregate_revision).as_slice(),
                at.unix_seconds(),
                i64::from(at.subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    Ok(())
}

fn complete_versioned(
    transaction: Transaction<'_>,
    binding: &CommandBinding,
    primary_id: [u8; 16],
    payload: VersionedResultPayload,
    at: Timestamp,
) -> Result<MutationOutcome, AssetStoreError> {
    let payload_bytes = payload.encode();
    let payload_sha256: [u8; 32] = Sha256::digest(&payload_bytes).into();
    let changed = transaction
        .execute(
            "UPDATE commands SET state='COMPLETED', result_kind=?2, result_id=?3, result_schema_version=1, result_payload=?4, result_payload_sha256=?5, updated_at_seconds=?6, updated_at_nanos=?7 WHERE command_id=?1 AND state='CLAIMED'",
            params![
                binding.command_id().to_bytes().as_slice(),
                payload.result_kind(),
                primary_id.as_slice(),
                payload_bytes,
                payload_sha256.as_slice(),
                at.unix_seconds(),
                i64::from(at.subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    let row = validate_command_row(
        read_command(&transaction, binding)?.ok_or(AssetStoreError::StorageCorruption)?,
    )?;
    let result = replay_creative_result(&transaction, &row)?;
    transaction.commit().map_err(sqlite)?;
    Ok(MutationOutcome::Applied(result))
}

fn create_project(
    connection: &mut Connection,
    context: crate::asset_repository::StoreContext,
    request: CreateProjectCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(outcome) = replay_existing(&transaction, context, request.binding(), "PROJECT")? {
        return Ok(outcome);
    }
    let project_id = generated_id::<Project>(request.values())?;
    let spec_id = generated_id::<ProjectSpecRevision>(request.values())?;
    let event_id = generated_id::<DomainEvent>(request.values())?;
    distinct(&[
        project_id.to_bytes(),
        spec_id.to_bytes(),
        event_id.to_bytes(),
    ])?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    insert_claim(&transaction, context, request.binding(), at)?;
    let sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    insert_domain_event(
        &transaction,
        event_id.to_bytes(),
        sequence,
        request.binding(),
        "project.created.v1",
        "PROJECT",
        project_id.to_bytes(),
        1,
        at,
    )?;
    transaction
        .execute(
            "INSERT INTO projects (project_id, name, current_spec_revision_id, revision, creation_commit_sequence, created_by_command_id, created_at_seconds, created_at_nanos, updated_at_seconds, updated_at_nanos) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?7, ?8)",
            params![
                project_id.to_bytes().as_slice(),
                request.name().as_str(),
                spec_id.to_bytes().as_slice(),
                revision_bytes(1).as_slice(),
                sequence,
                request.binding().command_id().to_bytes().as_slice(),
                at.unix_seconds(),
                i64::from(at.subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    insert_project_specification(
        &transaction,
        spec_id.to_bytes(),
        project_id.to_bytes(),
        1,
        request.binding().command_id().to_bytes(),
        at,
        request.specification(),
    )?;
    complete_versioned(
        transaction,
        request.binding(),
        project_id.to_bytes(),
        VersionedResultPayload::Project {
            spec_revision_id: spec_id.to_bytes(),
            revision: 1,
            sequence: 1,
        },
        at,
    )
}

fn revise_project_spec(
    connection: &mut Connection,
    context: crate::asset_repository::StoreContext,
    request: ReviseProjectSpecCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(outcome) = replay_existing(
        &transaction,
        context,
        request.binding(),
        "PROJECT_SPEC_REVISION",
    )? {
        return Ok(outcome);
    }
    let spec_id = generated_id::<ProjectSpecRevision>(request.values())?;
    let event_id = generated_id::<DomainEvent>(request.values())?;
    distinct(&[spec_id.to_bytes(), event_id.to_bytes()])?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    insert_claim(&transaction, context, request.binding(), at)?;
    let current: Option<Vec<u8>> = transaction
        .query_row(
            "SELECT revision FROM projects WHERE project_id=?1",
            [request.project_id().as_slice()],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite)?;
    let Some(current) = current else {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    };
    let current = parse_revision(&current)?;
    if current != request.expected_revision() {
        return commit_rejection(transaction, request.binding(), ErrorCode::Conflict, at);
    }
    let Some(next) = current.get().checked_add(1) else {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::RevisionExhausted,
            at,
        );
    };
    let Ok(sequence) = u32::try_from(next) else {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::RevisionExhausted,
            at,
        );
    };
    insert_project_specification(
        &transaction,
        spec_id.to_bytes(),
        request.project_id(),
        sequence,
        request.binding().command_id().to_bytes(),
        at,
        request.specification(),
    )?;
    let event_sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    let changed = transaction
        .execute(
            "UPDATE projects SET current_spec_revision_id=?2, revision=?3, updated_at_seconds=?4, updated_at_nanos=?5 WHERE project_id=?1 AND revision=?6",
            params![
                request.project_id().as_slice(),
                spec_id.to_bytes().as_slice(),
                revision_bytes(next).as_slice(),
                at.unix_seconds(),
                i64::from(at.subsec_nanoseconds()),
                revision_bytes(current.get()).as_slice(),
            ],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    insert_domain_event(
        &transaction,
        event_id.to_bytes(),
        event_sequence,
        request.binding(),
        "project.spec.revised.v1",
        "PROJECT_SPEC_REVISION",
        spec_id.to_bytes(),
        next,
        at,
    )?;
    complete_versioned(
        transaction,
        request.binding(),
        spec_id.to_bytes(),
        VersionedResultPayload::ProjectSpecRevision {
            project_id: request.project_id(),
            revision: next,
            sequence,
        },
        at,
    )
}

fn create_subject(
    connection: &mut Connection,
    context: crate::asset_repository::StoreContext,
    request: CreateSubjectCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(outcome) = replay_existing(&transaction, context, request.binding(), "SUBJECT")? {
        return Ok(outcome);
    }
    let subject_id = generated_id::<Subject>(request.values())?;
    let event_id = generated_id::<DomainEvent>(request.values())?;
    distinct(&[subject_id.to_bytes(), event_id.to_bytes()])?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    insert_claim(&transaction, context, request.binding(), at)?;
    let sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    insert_domain_event(
        &transaction,
        event_id.to_bytes(),
        sequence,
        request.binding(),
        "subject.created.v1",
        "SUBJECT",
        subject_id.to_bytes(),
        1,
        at,
    )?;
    transaction
        .execute(
            "INSERT INTO subjects (subject_id, kind, canonical_name, revision, creation_commit_sequence, created_by_command_id, created_at_seconds, created_at_nanos) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                subject_id.to_bytes().as_slice(),
                request.kind().as_str(),
                request.name().as_str(),
                revision_bytes(1).as_slice(),
                sequence,
                request.binding().command_id().to_bytes().as_slice(),
                at.unix_seconds(),
                i64::from(at.subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    complete_versioned(
        transaction,
        request.binding(),
        subject_id.to_bytes(),
        VersionedResultPayload::Subject { revision: 1 },
        at,
    )
}

fn validate_work_references(
    transaction: &Transaction<'_>,
    specification: &mengxia_domain::WorkSpecification,
) -> Result<bool, AssetStoreError> {
    for subject_id in specification.subject_ids() {
        let exists = transaction
            .query_row(
                "SELECT 1 FROM subjects WHERE subject_id=?1",
                [subject_id.to_bytes().as_slice()],
                |_| Ok(()),
            )
            .optional()
            .map_err(sqlite)?
            .is_some();
        if !exists {
            return Ok(false);
        }
    }
    for asset_id in specification.asset_ids() {
        let exists = transaction
            .query_row(
                "SELECT 1 FROM assets WHERE asset_id=?1",
                [asset_id.to_bytes().as_slice()],
                |_| Ok(()),
            )
            .optional()
            .map_err(sqlite)?
            .is_some();
        if !exists {
            return Ok(false);
        }
    }
    Ok(true)
}

// A Work revision and its bounded relationship set are one atomic persistence
// record, so splitting this helper would obscure that invariant.
#[allow(clippy::too_many_arguments)]
fn insert_work_revision(
    transaction: &Transaction<'_>,
    work_revision_id: [u8; 16],
    work_item_id: [u8; 16],
    sequence: u32,
    binding: &CommandBinding,
    at: Timestamp,
    specification: &mengxia_domain::WorkSpecification,
    relationship_ids: &[[u8; 16]],
) -> Result<(), AssetStoreError> {
    transaction
        .execute(
            "INSERT INTO work_revisions (work_revision_id, work_item_id, sequence, specification_schema_version, specification_json, specification_digest, created_by_command_id, created_at_seconds, created_at_nanos) VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8)",
            params![
                work_revision_id.as_slice(),
                work_item_id.as_slice(),
                i64::from(sequence),
                specification.json().bytes(),
                specification.json().digest().to_bytes().as_slice(),
                binding.command_id().to_bytes().as_slice(),
                at.unix_seconds(),
                i64::from(at.subsec_nanoseconds()),
            ],
        )
        .map_err(sqlite)?;
    let mut relationship_index = 0_usize;
    for subject_id in specification.subject_ids() {
        let relationship_id = relationship_ids
            .get(relationship_index)
            .ok_or(AssetStoreError::Internal)?;
        relationship_index += 1;
        transaction
            .execute(
                "INSERT INTO relationships (relationship_id, relationship_kind, source_kind, source_id, target_kind, target_id, created_by_command_id, created_at_seconds, created_at_nanos) VALUES (?1, 'WORK_SUBJECT', 'WORK_REVISION', ?2, 'SUBJECT', ?3, ?4, ?5, ?6)",
                params![relationship_id.as_slice(), work_revision_id.as_slice(), subject_id.to_bytes().as_slice(), binding.command_id().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())],
            )
            .map_err(sqlite)?;
    }
    for asset_id in specification.asset_ids() {
        let relationship_id = relationship_ids
            .get(relationship_index)
            .ok_or(AssetStoreError::Internal)?;
        relationship_index += 1;
        transaction
            .execute(
                "INSERT INTO relationships (relationship_id, relationship_kind, source_kind, source_id, target_kind, target_id, created_by_command_id, created_at_seconds, created_at_nanos) VALUES (?1, 'WORK_ASSET', 'WORK_REVISION', ?2, 'ASSET', ?3, ?4, ?5, ?6)",
                params![relationship_id.as_slice(), work_revision_id.as_slice(), asset_id.to_bytes().as_slice(), binding.command_id().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())],
            )
            .map_err(sqlite)?;
    }
    if relationship_index != relationship_ids.len() {
        return Err(AssetStoreError::Internal);
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
fn sample_work_ids(
    source: &dyn PureCommandValueSource,
    relationship_count: usize,
) -> Result<
    (
        Id<WorkItem>,
        Id<WorkRevision>,
        Id<DomainEvent>,
        Vec<[u8; 16]>,
    ),
    AssetStoreError,
> {
    let work_item_id = generated_id::<WorkItem>(source)?;
    let work_revision_id = generated_id::<WorkRevision>(source)?;
    let event_id = generated_id::<DomainEvent>(source)?;
    let relationship_ids = (0..relationship_count)
        .map(|_| generated_id::<Relationship>(source).map(Id::to_bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let mut all = vec![
        work_item_id.to_bytes(),
        work_revision_id.to_bytes(),
        event_id.to_bytes(),
    ];
    all.extend_from_slice(&relationship_ids);
    distinct(&all)?;
    Ok((work_item_id, work_revision_id, event_id, relationship_ids))
}

fn create_work(
    connection: &mut Connection,
    context: crate::asset_repository::StoreContext,
    request: CreateWorkCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(outcome) = replay_existing(&transaction, context, request.binding(), "WORK_ITEM")? {
        return Ok(outcome);
    }
    let (work_id, revision_id, event_id, relationship_ids) = sample_work_ids(
        request.values(),
        request.specification().subject_ids().len() + request.specification().asset_ids().len(),
    )?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    insert_claim(&transaction, context, request.binding(), at)?;
    let project_exists = transaction
        .query_row(
            "SELECT 1 FROM projects WHERE project_id=?1",
            [request.project_id().to_bytes().as_slice()],
            |_| Ok(()),
        )
        .optional()
        .map_err(sqlite)?
        .is_some();
    if !project_exists || !validate_work_references(&transaction, request.specification())? {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    }
    let duplicate = transaction
        .query_row(
            "SELECT 1 FROM work_items WHERE project_id=?1 AND kind=?2 AND code=?3",
            params![
                request.project_id().to_bytes().as_slice(),
                request.kind().as_str(),
                request.code().as_str()
            ],
            |_| Ok(()),
        )
        .optional()
        .map_err(sqlite)?
        .is_some();
    if duplicate {
        return commit_rejection(transaction, request.binding(), ErrorCode::Conflict, at);
    }
    let event_sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    insert_domain_event(
        &transaction,
        event_id.to_bytes(),
        event_sequence,
        request.binding(),
        "work.created.v1",
        "WORK_ITEM",
        work_id.to_bytes(),
        1,
        at,
    )?;
    transaction
        .execute(
            "INSERT INTO work_items (work_item_id, project_id, kind, code, current_work_revision_id, revision, creation_commit_sequence, created_by_command_id, created_at_seconds, created_at_nanos, updated_at_seconds, updated_at_nanos) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?9, ?10)",
            params![work_id.to_bytes().as_slice(), request.project_id().to_bytes().as_slice(), request.kind().as_str(), request.code().as_str(), revision_id.to_bytes().as_slice(), revision_bytes(1).as_slice(), event_sequence, request.binding().command_id().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())],
        )
        .map_err(sqlite)?;
    insert_work_revision(
        &transaction,
        revision_id.to_bytes(),
        work_id.to_bytes(),
        1,
        request.binding(),
        at,
        request.specification(),
        &relationship_ids,
    )?;
    complete_versioned(
        transaction,
        request.binding(),
        work_id.to_bytes(),
        VersionedResultPayload::WorkItem {
            work_revision_id: revision_id.to_bytes(),
            revision: 1,
            sequence: 1,
        },
        at,
    )
}

fn revise_work(
    connection: &mut Connection,
    context: crate::asset_repository::StoreContext,
    request: ReviseWorkCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(outcome) =
        replay_existing(&transaction, context, request.binding(), "WORK_REVISION")?
    {
        return Ok(outcome);
    }
    let revision_id = generated_id::<WorkRevision>(request.values())?;
    let event_id = generated_id::<DomainEvent>(request.values())?;
    let relationship_ids = (0..request.specification().subject_ids().len()
        + request.specification().asset_ids().len())
        .map(|_| generated_id::<Relationship>(request.values()).map(Id::to_bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let mut ids = vec![revision_id.to_bytes(), event_id.to_bytes()];
    ids.extend_from_slice(&relationship_ids);
    distinct(&ids)?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    insert_claim(&transaction, context, request.binding(), at)?;
    let current: Option<Vec<u8>> = transaction
        .query_row(
            "SELECT revision FROM work_items WHERE work_item_id=?1 AND project_id=?2",
            params![
                request.work_item_id().to_bytes().as_slice(),
                request.project_id().to_bytes().as_slice()
            ],
            |row| row.get(0),
        )
        .optional()
        .map_err(sqlite)?;
    let Some(current) = current else {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    };
    let current = parse_revision(&current)?;
    if current != request.expected_revision() {
        return commit_rejection(transaction, request.binding(), ErrorCode::Conflict, at);
    }
    if !validate_work_references(&transaction, request.specification())? {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    }
    let Some(next) = current.get().checked_add(1) else {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::RevisionExhausted,
            at,
        );
    };
    let Ok(sequence) = u32::try_from(next) else {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::RevisionExhausted,
            at,
        );
    };
    insert_work_revision(
        &transaction,
        revision_id.to_bytes(),
        request.work_item_id().to_bytes(),
        sequence,
        request.binding(),
        at,
        request.specification(),
        &relationship_ids,
    )?;
    let changed = transaction
        .execute(
            "UPDATE work_items SET current_work_revision_id=?2, revision=?3, updated_at_seconds=?4, updated_at_nanos=?5 WHERE work_item_id=?1 AND project_id=?6 AND revision=?7",
            params![request.work_item_id().to_bytes().as_slice(), revision_id.to_bytes().as_slice(), revision_bytes(next).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds()), request.project_id().to_bytes().as_slice(), revision_bytes(current.get()).as_slice()],
        )
        .map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    let event_sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    insert_domain_event(
        &transaction,
        event_id.to_bytes(),
        event_sequence,
        request.binding(),
        "work.revised.v1",
        "WORK_REVISION",
        revision_id.to_bytes(),
        next,
        at,
    )?;
    complete_versioned(
        transaction,
        request.binding(),
        revision_id.to_bytes(),
        VersionedResultPayload::WorkRevision {
            work_item_id: request.work_item_id().to_bytes(),
            revision: next,
            sequence,
        },
        at,
    )
}

fn take_context_exists(
    transaction: &Transaction<'_>,
    project_id: [u8; 16],
    work_item_id: [u8; 16],
    work_revision_id: [u8; 16],
) -> Result<bool, AssetStoreError> {
    transaction
        .query_row(
            "SELECT 1 FROM work_items w JOIN work_revisions wr ON wr.work_item_id=w.work_item_id WHERE w.project_id=?1 AND w.work_item_id=?2 AND wr.work_revision_id=?3",
            params![project_id.as_slice(), work_item_id.as_slice(), work_revision_id.as_slice()],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(sqlite)
}

fn asset_exists(
    transaction: &Transaction<'_>,
    asset_id: [u8; 16],
) -> Result<bool, AssetStoreError> {
    transaction
        .query_row(
            "SELECT 1 FROM assets WHERE asset_id=?1",
            [asset_id.as_slice()],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(sqlite)
}

fn next_take_ordinal(
    transaction: &Transaction<'_>,
    work_revision_id: [u8; 16],
) -> Result<u32, AssetStoreError> {
    let maximum: Option<i64> = transaction
        .query_row(
            "SELECT max(ordinal) FROM takes WHERE work_revision_id=?1",
            [work_revision_id.as_slice()],
            |row| row.get(0),
        )
        .map_err(sqlite)?;
    u32::try_from(maximum.unwrap_or(0))
        .ok()
        .and_then(|value| value.checked_add(1))
        .filter(|value| *value != 0)
        .ok_or(AssetStoreError::RevisionExhausted)
}

type TakeRow = (u32, TakeState, [u8; 16], mengxia_types::RevisionNo);

fn read_take(
    transaction: &Transaction<'_>,
    take_id: [u8; 16],
    work_revision_id: [u8; 16],
) -> Result<Option<TakeRow>, AssetStoreError> {
    let row: Option<(i64, String, Vec<u8>, Vec<u8>)> = transaction
        .query_row(
            "SELECT ordinal, state, primary_asset_id, revision FROM takes WHERE take_id=?1 AND work_revision_id=?2",
            params![take_id.as_slice(), work_revision_id.as_slice()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(sqlite)?;
    row.map(|(ordinal, state, asset, revision)| {
        Ok((
            u32::try_from(ordinal).map_err(|_| AssetStoreError::StorageCorruption)?,
            TakeState::parse(&state).map_err(|_| AssetStoreError::StorageCorruption)?,
            asset
                .try_into()
                .map_err(|_| AssetStoreError::StorageCorruption)?,
            parse_revision(&revision)?,
        ))
    })
    .transpose()
}

#[allow(clippy::too_many_arguments)]
fn insert_take(
    transaction: &Transaction<'_>,
    take_id: [u8; 16],
    work_revision_id: [u8; 16],
    ordinal: u32,
    state: TakeState,
    primary_asset_id: [u8; 16],
    creation_commit_sequence: i64,
    binding: &CommandBinding,
    at: Timestamp,
) -> Result<(), AssetStoreError> {
    transaction
        .execute(
            "INSERT INTO takes (take_id, work_revision_id, ordinal, state, primary_asset_id, revision, creation_commit_sequence, created_by_command_id, created_at_seconds, created_at_nanos, updated_at_seconds, updated_at_nanos) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?9, ?10)",
            params![take_id.as_slice(), work_revision_id.as_slice(), i64::from(ordinal), state.as_str(), primary_asset_id.as_slice(), revision_bytes(1).as_slice(), creation_commit_sequence, binding.command_id().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())],
        )
        .map_err(sqlite)?;
    Ok(())
}

fn insert_relationship(
    transaction: &Transaction<'_>,
    relationship_id: [u8; 16],
    kind: &str,
    source_id: [u8; 16],
    target_id: [u8; 16],
    binding: &CommandBinding,
    at: Timestamp,
) -> Result<(), AssetStoreError> {
    transaction
        .execute(
            "INSERT INTO relationships (relationship_id, relationship_kind, source_kind, source_id, target_kind, target_id, created_by_command_id, created_at_seconds, created_at_nanos) VALUES (?1, ?2, 'TAKE', ?3, 'TAKE', ?4, ?5, ?6, ?7)",
            params![relationship_id.as_slice(), kind, source_id.as_slice(), target_id.as_slice(), binding.command_id().to_bytes().as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds())],
        )
        .map_err(sqlite)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_payload_event(
    transaction: &Transaction<'_>,
    event_id: [u8; 16],
    sequence: i64,
    binding: &CommandBinding,
    event_type: &str,
    aggregate_id: [u8; 16],
    aggregate_revision: u64,
    payload_value: Option<&[u8]>,
    at: Timestamp,
) -> Result<(), AssetStoreError> {
    let payload = payload_value
        .map(|value| EventPayload::from_fields([(1, value.to_vec())]))
        .transpose()
        .map_err(|_| AssetStoreError::Internal)?
        .flatten();
    let payload_bytes = payload.as_ref().map(EventPayload::as_bytes);
    let payload_hash = payload_bytes.map(|bytes| <[u8; 32]>::from(Sha256::digest(bytes)));
    transaction
        .execute(
            "INSERT INTO domain_events (domain_event_id, commit_sequence, command_id, event_type, schema_version, aggregate_kind, aggregate_id, aggregate_revision, event_payload, event_payload_sha256, occurred_at_seconds, occurred_at_nanos) VALUES (?1, ?2, ?3, ?4, 1, 'TAKE', ?5, ?6, ?7, ?8, ?9, ?10)",
            params![event_id.as_slice(), sequence, binding.command_id().to_bytes().as_slice(), event_type, aggregate_id.as_slice(), revision_bytes(aggregate_revision).as_slice(), payload_bytes, payload_hash.as_ref().map(<[u8; 32]>::as_slice), at.unix_seconds(), i64::from(at.subsec_nanoseconds())],
        )
        .map_err(sqlite)?;
    Ok(())
}

fn create_take(
    connection: &mut Connection,
    context: crate::asset_repository::StoreContext,
    request: CreateTakeCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(outcome) = replay_existing(&transaction, context, request.binding(), "TAKE")? {
        return Ok(outcome);
    }
    let take_id = generated_id::<Take>(request.values())?;
    let event_id = generated_id::<DomainEvent>(request.values())?;
    distinct(&[take_id.to_bytes(), event_id.to_bytes()])?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    insert_claim(&transaction, context, request.binding(), at)?;
    if !take_context_exists(
        &transaction,
        request.project_id().to_bytes(),
        request.work_item_id().to_bytes(),
        request.work_revision_id().to_bytes(),
    )? || !asset_exists(&transaction, request.primary_asset_id().to_bytes())?
    {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    }
    let ordinal = match next_take_ordinal(&transaction, request.work_revision_id().to_bytes()) {
        Ok(ordinal) => ordinal,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    let event_sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    insert_domain_event(
        &transaction,
        event_id.to_bytes(),
        event_sequence,
        request.binding(),
        "take.created.v1",
        "TAKE",
        take_id.to_bytes(),
        1,
        at,
    )?;
    insert_take(
        &transaction,
        take_id.to_bytes(),
        request.work_revision_id().to_bytes(),
        ordinal,
        TakeState::Candidate,
        request.primary_asset_id().to_bytes(),
        event_sequence,
        request.binding(),
        at,
    )?;
    complete_versioned(
        transaction,
        request.binding(),
        take_id.to_bytes(),
        VersionedResultPayload::Take {
            revision: 1,
            ordinal,
            state: TakeState::Candidate,
            primary_asset_id: request.primary_asset_id().to_bytes(),
            related_take_id: None,
        },
        at,
    )
}

fn transition_take(
    connection: &mut Connection,
    context: crate::asset_repository::StoreContext,
    request: TransitionTakeCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(outcome) = replay_existing(&transaction, context, request.binding(), "TAKE")? {
        return Ok(outcome);
    }
    let first_event = generated_id::<DomainEvent>(request.values())?;
    let second_event = generated_id::<DomainEvent>(request.values())?;
    let relationship_id = generated_id::<Relationship>(request.values())?;
    distinct(&[
        first_event.to_bytes(),
        second_event.to_bytes(),
        relationship_id.to_bytes(),
    ])?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    insert_claim(&transaction, context, request.binding(), at)?;
    if !take_context_exists(
        &transaction,
        request.project_id().to_bytes(),
        request.work_item_id().to_bytes(),
        request.work_revision_id().to_bytes(),
    )? {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    }
    let current = read_take(
        &transaction,
        request.take_id().to_bytes(),
        request.work_revision_id().to_bytes(),
    )?;
    let Some((ordinal, state, primary_asset_id, revision)) = current else {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    };
    if revision != request.expected_revision() {
        return commit_rejection(transaction, request.binding(), ErrorCode::Conflict, at);
    }
    let target = match state.transition(request.transition()) {
        Ok(target) => target,
        Err(_) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::InvalidTransition,
                at,
            );
        }
    };
    if request.transition() == TakeTransition::Shortlist
        && !asset_exists(&transaction, primary_asset_id)?
    {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    }
    let next = match revision.get().checked_add(1) {
        Some(next) => next,
        None => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
    };
    let mut related = None;
    let mut selected_replacement: Option<(Id<Take>, RevisionNo, u64)> = None;
    if request.transition() == TakeTransition::Select {
        let selected: Option<(Vec<u8>, Vec<u8>)> = transaction
            .query_row(
                "SELECT take_id, revision FROM takes WHERE work_revision_id=?1 AND state='SELECTED' AND take_id<>?2",
                params![request.work_revision_id().to_bytes().as_slice(), request.take_id().to_bytes().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(sqlite)?;
        match (selected, request.related_take()) {
            (None, None) => {}
            (Some((selected_id, selected_revision)), Some((supplied_id, supplied_revision))) => {
                let selected_id: [u8; 16] = selected_id
                    .try_into()
                    .map_err(|_| AssetStoreError::StorageCorruption)?;
                let selected_revision = parse_revision(&selected_revision)?;
                if selected_id != supplied_id.to_bytes() || selected_revision != supplied_revision {
                    return commit_rejection(
                        transaction,
                        request.binding(),
                        ErrorCode::Conflict,
                        at,
                    );
                }
                let superseded_revision = match selected_revision.get().checked_add(1) {
                    Some(value) => value,
                    None => {
                        return commit_rejection(
                            transaction,
                            request.binding(),
                            ErrorCode::RevisionExhausted,
                            at,
                        );
                    }
                };
                selected_replacement = Some((supplied_id, selected_revision, superseded_revision));
                related = Some(supplied_id.to_bytes());
            }
            _ => {
                return commit_rejection(
                    transaction,
                    request.binding(),
                    ErrorCode::InvalidTransition,
                    at,
                );
            }
        }
    } else if request.transition() == TakeTransition::Supersede {
        let Some((replacement_id, replacement_revision)) = request.related_take() else {
            return Err(AssetStoreError::Internal);
        };
        let replacement = read_take(
            &transaction,
            replacement_id.to_bytes(),
            request.work_revision_id().to_bytes(),
        )?;
        let Some((_ordinal, _state, _asset, stored_revision)) = replacement else {
            return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
        };
        if replacement_id == request.take_id() || stored_revision != replacement_revision {
            return commit_rejection(transaction, request.binding(), ErrorCode::Conflict, at);
        }
        related = Some(replacement_id.to_bytes());
    }

    let event_count = if selected_replacement.is_some() { 2 } else { 1 };
    let first_sequence = match allocate_event_sequences(&transaction, event_count) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    if let Some((selected_id, selected_revision, superseded_revision)) = selected_replacement {
        let changed = transaction.execute(
            "UPDATE takes SET state='SUPERSEDED', revision=?2, updated_at_seconds=?3, updated_at_nanos=?4 WHERE take_id=?1 AND revision=?5 AND state='SELECTED'",
            params![selected_id.to_bytes().as_slice(), revision_bytes(superseded_revision).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds()), revision_bytes(selected_revision.get()).as_slice()],
        ).map_err(sqlite)?;
        if changed != 1 {
            return Err(AssetStoreError::StorageCorruption);
        }
        insert_payload_event(
            &transaction,
            first_event.to_bytes(),
            first_sequence,
            request.binding(),
            "take.superseded.v1",
            selected_id.to_bytes(),
            superseded_revision,
            Some(request.take_id().to_bytes().as_slice()),
            at,
        )?;
        insert_relationship(
            &transaction,
            relationship_id.to_bytes(),
            "TAKE_SUPERSEDES",
            request.take_id().to_bytes(),
            selected_id.to_bytes(),
            request.binding(),
            at,
        )?;
    } else if request.transition() == TakeTransition::Supersede {
        insert_relationship(
            &transaction,
            relationship_id.to_bytes(),
            "TAKE_SUPERSEDES",
            related.ok_or(AssetStoreError::Internal)?,
            request.take_id().to_bytes(),
            request.binding(),
            at,
        )?;
    }
    let changed = transaction.execute(
        "UPDATE takes SET state=?2, revision=?3, updated_at_seconds=?4, updated_at_nanos=?5 WHERE take_id=?1 AND revision=?6",
        params![request.take_id().to_bytes().as_slice(), target.as_str(), revision_bytes(next).as_slice(), at.unix_seconds(), i64::from(at.subsec_nanoseconds()), revision_bytes(revision.get()).as_slice()],
    ).map_err(sqlite)?;
    if changed != 1 {
        return Err(AssetStoreError::StorageCorruption);
    }
    let (event_type, event_payload) = match request.transition() {
        TakeTransition::Shortlist => ("take.shortlisted.v1", None),
        TakeTransition::Select => (
            "take.selected.v1",
            related.as_ref().map(<[u8; 16]>::as_slice),
        ),
        TakeTransition::Approve => ("take.approved.v1", None),
        TakeTransition::Reject => (
            "take.rejected.v1",
            request.reason().map(|reason| reason.as_str().as_bytes()),
        ),
        TakeTransition::Supersede => (
            "take.superseded.v1",
            related.as_ref().map(<[u8; 16]>::as_slice),
        ),
    };
    insert_payload_event(
        &transaction,
        if selected_replacement.is_some() {
            second_event.to_bytes()
        } else {
            first_event.to_bytes()
        },
        if selected_replacement.is_some() {
            first_sequence + 1
        } else {
            first_sequence
        },
        request.binding(),
        event_type,
        request.take_id().to_bytes(),
        next,
        event_payload,
        at,
    )?;
    complete_versioned(
        transaction,
        request.binding(),
        request.take_id().to_bytes(),
        VersionedResultPayload::Take {
            revision: next,
            ordinal,
            state: target,
            primary_asset_id,
            related_take_id: related,
        },
        at,
    )
}

fn reopen_take(
    connection: &mut Connection,
    context: crate::asset_repository::StoreContext,
    request: ReopenTakeCommand,
) -> Result<MutationOutcome, AssetStoreError> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite)?;
    if let Some(outcome) = replay_existing(&transaction, context, request.binding(), "TAKE")? {
        return Ok(outcome);
    }
    let take_id = generated_id::<Take>(request.values())?;
    let event_id = generated_id::<DomainEvent>(request.values())?;
    let relationship_id = generated_id::<Relationship>(request.values())?;
    distinct(&[
        take_id.to_bytes(),
        event_id.to_bytes(),
        relationship_id.to_bytes(),
    ])?;
    let at = request.values().now()?;
    request.values().checkpoint()?;
    insert_claim(&transaction, context, request.binding(), at)?;
    if !take_context_exists(
        &transaction,
        request.project_id().to_bytes(),
        request.work_item_id().to_bytes(),
        request.work_revision_id().to_bytes(),
    )? || !asset_exists(&transaction, request.new_primary_asset_id().to_bytes())?
    {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    }
    let terminal = read_take(
        &transaction,
        request.terminal_take_id().to_bytes(),
        request.work_revision_id().to_bytes(),
    )?;
    let Some((_old_ordinal, old_state, old_asset, old_revision)) = terminal else {
        return commit_rejection(transaction, request.binding(), ErrorCode::NotFound, at);
    };
    if old_revision != request.expected_terminal_revision() {
        return commit_rejection(transaction, request.binding(), ErrorCode::Conflict, at);
    }
    if !old_state.is_terminal() || old_asset == request.new_primary_asset_id().to_bytes() {
        return commit_rejection(
            transaction,
            request.binding(),
            ErrorCode::InvalidTransition,
            at,
        );
    }
    let ordinal = match next_take_ordinal(&transaction, request.work_revision_id().to_bytes()) {
        Ok(ordinal) => ordinal,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    let event_sequence = match allocate_event_sequences(&transaction, 1) {
        Ok(sequence) => sequence,
        Err(AssetStoreError::RevisionExhausted) => {
            return commit_rejection(
                transaction,
                request.binding(),
                ErrorCode::RevisionExhausted,
                at,
            );
        }
        Err(error) => return Err(error),
    };
    insert_payload_event(
        &transaction,
        event_id.to_bytes(),
        event_sequence,
        request.binding(),
        "take.reopened.v1",
        take_id.to_bytes(),
        1,
        Some(request.terminal_take_id().to_bytes().as_slice()),
        at,
    )?;
    insert_take(
        &transaction,
        take_id.to_bytes(),
        request.work_revision_id().to_bytes(),
        ordinal,
        TakeState::Candidate,
        request.new_primary_asset_id().to_bytes(),
        event_sequence,
        request.binding(),
        at,
    )?;
    insert_relationship(
        &transaction,
        relationship_id.to_bytes(),
        "TAKE_REOPENS",
        take_id.to_bytes(),
        request.terminal_take_id().to_bytes(),
        request.binding(),
        at,
    )?;
    complete_versioned(
        transaction,
        request.binding(),
        take_id.to_bytes(),
        VersionedResultPayload::Take {
            revision: 1,
            ordinal,
            state: TakeState::Candidate,
            primary_asset_id: request.new_primary_asset_id().to_bytes(),
            related_take_id: Some(request.terminal_take_id().to_bytes()),
        },
        at,
    )
}

pub(crate) fn replay_creative_result(
    transaction: &Transaction<'_>,
    row: &CommandRow,
) -> Result<CommandResult, AssetStoreError> {
    let primary: [u8; 16] = row
        .result_id
        .as_deref()
        .ok_or(AssetStoreError::StorageCorruption)?
        .try_into()
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    let kind = row
        .result_kind
        .as_deref()
        .ok_or(AssetStoreError::StorageCorruption)?;
    let payload = VersionedResultPayload::decode(
        kind,
        row.result_payload
            .as_deref()
            .ok_or(AssetStoreError::StorageCorruption)?,
    )?;
    let revision = match payload {
        VersionedResultPayload::Project {
            spec_revision_id,
            revision,
            sequence,
        } => {
            let observed: Option<(Vec<u8>, Vec<u8>, i64, i64)> = transaction
                .query_row(
                    "SELECT ps.project_spec_revision_id, p.revision, ps.sequence, (SELECT count(*) FROM domain_events WHERE command_id=?3 AND event_type='project.created.v1' AND aggregate_kind='PROJECT' AND aggregate_id=p.project_id AND aggregate_revision=?4) FROM projects p JOIN project_spec_revisions ps ON ps.project_id=p.project_id WHERE p.project_id=?1 AND ps.project_spec_revision_id=?2",
                    params![primary.as_slice(), spec_revision_id.as_slice(), row.command_id.as_slice(), revision_bytes(revision).as_slice()],
                    |result| Ok((result.get(0)?, result.get(1)?, result.get(2)?, result.get(3)?)),
                )
                .optional()
                .map_err(sqlite)?;
            let Some((stored_spec, stored_revision, stored_sequence, event_count)) = observed
            else {
                return Err(AssetStoreError::StorageCorruption);
            };
            if stored_spec.as_slice() != spec_revision_id
                || parse_revision(&stored_revision)?.get() < revision
                || stored_sequence != i64::from(sequence)
                || event_count != 1
            {
                return Err(AssetStoreError::StorageCorruption);
            }
            revision
        }
        VersionedResultPayload::ProjectSpecRevision {
            project_id,
            revision,
            sequence,
        } => {
            let observed: Option<(Vec<u8>, Vec<u8>, i64, i64)> = transaction
                .query_row(
                    "SELECT ps.project_spec_revision_id, p.revision, ps.sequence, (SELECT count(*) FROM domain_events WHERE command_id=?3 AND event_type='project.spec.revised.v1' AND aggregate_kind='PROJECT_SPEC_REVISION' AND aggregate_id=?1 AND aggregate_revision=?4) FROM project_spec_revisions ps JOIN projects p ON p.project_id=ps.project_id WHERE ps.project_spec_revision_id=?1 AND ps.project_id=?2",
                    params![primary.as_slice(), project_id.as_slice(), row.command_id.as_slice(), revision_bytes(revision).as_slice()],
                    |result| Ok((result.get(0)?, result.get(1)?, result.get(2)?, result.get(3)?)),
                )
                .optional()
                .map_err(sqlite)?;
            let Some((stored_spec, stored_revision, stored_sequence, event_count)) = observed
            else {
                return Err(AssetStoreError::StorageCorruption);
            };
            if stored_spec.as_slice() != primary
                || parse_revision(&stored_revision)?.get() < revision
                || stored_sequence != i64::from(sequence)
                || event_count != 1
            {
                return Err(AssetStoreError::StorageCorruption);
            }
            revision
        }
        VersionedResultPayload::Subject { revision } => {
            let observed: Option<(Vec<u8>, i64)> = transaction
                .query_row(
                    "SELECT s.revision, (SELECT count(*) FROM domain_events WHERE command_id=?2 AND event_type='subject.created.v1' AND aggregate_kind='SUBJECT' AND aggregate_id=s.subject_id AND aggregate_revision=s.revision) FROM subjects s WHERE s.subject_id=?1",
                    params![primary.as_slice(), row.command_id.as_slice()],
                    |result| Ok((result.get(0)?, result.get(1)?)),
                )
                .optional()
                .map_err(sqlite)?;
            let Some((stored_revision, event_count)) = observed else {
                return Err(AssetStoreError::StorageCorruption);
            };
            if parse_revision(&stored_revision)?.get() != revision || event_count != 1 {
                return Err(AssetStoreError::StorageCorruption);
            }
            revision
        }
        VersionedResultPayload::WorkItem {
            work_revision_id,
            revision,
            sequence,
        } => {
            let observed: Option<(Vec<u8>, Vec<u8>, i64, i64)> = transaction
                .query_row(
                    "SELECT wr.work_revision_id, w.revision, wr.sequence, (SELECT count(*) FROM domain_events WHERE command_id=?3 AND event_type='work.created.v1' AND aggregate_kind='WORK_ITEM' AND aggregate_id=w.work_item_id AND aggregate_revision=?4) FROM work_items w JOIN work_revisions wr ON wr.work_item_id=w.work_item_id WHERE w.work_item_id=?1 AND wr.work_revision_id=?2",
                    params![primary.as_slice(), work_revision_id.as_slice(), row.command_id.as_slice(), revision_bytes(revision).as_slice()],
                    |result| Ok((result.get(0)?, result.get(1)?, result.get(2)?, result.get(3)?)),
                )
                .optional()
                .map_err(sqlite)?;
            let Some((stored_revision_id, stored_revision, stored_sequence, event_count)) =
                observed
            else {
                return Err(AssetStoreError::StorageCorruption);
            };
            if stored_revision_id.as_slice() != work_revision_id
                || parse_revision(&stored_revision)?.get() < revision
                || stored_sequence != i64::from(sequence)
                || event_count != 1
            {
                return Err(AssetStoreError::StorageCorruption);
            }
            revision
        }
        VersionedResultPayload::WorkRevision {
            work_item_id,
            revision,
            sequence,
        } => {
            let observed: Option<(Vec<u8>, Vec<u8>, i64, i64)> = transaction
                .query_row(
                    "SELECT wr.work_revision_id, w.revision, wr.sequence, (SELECT count(*) FROM domain_events WHERE command_id=?3 AND event_type='work.revised.v1' AND aggregate_kind='WORK_REVISION' AND aggregate_id=?1 AND aggregate_revision=?4) FROM work_revisions wr JOIN work_items w ON w.work_item_id=wr.work_item_id WHERE wr.work_revision_id=?1 AND wr.work_item_id=?2",
                    params![primary.as_slice(), work_item_id.as_slice(), row.command_id.as_slice(), revision_bytes(revision).as_slice()],
                    |result| Ok((result.get(0)?, result.get(1)?, result.get(2)?, result.get(3)?)),
                )
                .optional()
                .map_err(sqlite)?;
            let Some((stored_revision_id, stored_revision, stored_sequence, event_count)) =
                observed
            else {
                return Err(AssetStoreError::StorageCorruption);
            };
            if stored_revision_id.as_slice() != primary
                || parse_revision(&stored_revision)?.get() < revision
                || stored_sequence != i64::from(sequence)
                || event_count != 1
            {
                return Err(AssetStoreError::StorageCorruption);
            }
            revision
        }
        VersionedResultPayload::Take {
            revision,
            ordinal,
            state,
            primary_asset_id,
            related_take_id,
        } => {
            let expected_event = match row.operation_id.as_str() {
                "take.create.v1" => "take.created.v1",
                "take.reopen.v1" => "take.reopened.v1",
                "take.transition.v1" => match state {
                    TakeState::Shortlisted => "take.shortlisted.v1",
                    TakeState::Selected => "take.selected.v1",
                    TakeState::Approved => "take.approved.v1",
                    TakeState::Rejected => "take.rejected.v1",
                    TakeState::Superseded => "take.superseded.v1",
                    TakeState::Candidate => return Err(AssetStoreError::StorageCorruption),
                },
                _ => return Err(AssetStoreError::StorageCorruption),
            };
            let observed: Option<(i64, Vec<u8>, Vec<u8>, i64)> = transaction
                .query_row(
                    "SELECT t.ordinal, t.primary_asset_id, t.revision, (SELECT count(*) FROM domain_events WHERE command_id=?2 AND event_type=?3 AND aggregate_kind='TAKE' AND aggregate_id=t.take_id AND aggregate_revision=?4) FROM takes t WHERE t.take_id=?1",
                    params![primary.as_slice(), row.command_id.as_slice(), expected_event, revision_bytes(revision).as_slice()],
                    |result| Ok((result.get(0)?, result.get(1)?, result.get(2)?, result.get(3)?)),
                )
                .optional()
                .map_err(sqlite)?;
            let Some((stored_ordinal, stored_asset, stored_revision, event_count)) = observed
            else {
                return Err(AssetStoreError::StorageCorruption);
            };
            if u32::try_from(stored_ordinal).ok() != Some(ordinal)
                || stored_asset.as_slice() != primary_asset_id
                || parse_revision(&stored_revision)?.get() < revision
                || event_count != 1
            {
                return Err(AssetStoreError::StorageCorruption);
            }
            let relationship_count = match (row.operation_id.as_str(), related_take_id) {
                ("take.create.v1", None) => 0,
                ("take.reopen.v1", Some(related)) => transaction
                    .query_row(
                        "SELECT count(*) FROM relationships WHERE created_by_command_id=?1 AND relationship_kind='TAKE_REOPENS' AND source_kind='TAKE' AND source_id=?2 AND target_kind='TAKE' AND target_id=?3",
                        params![row.command_id.as_slice(), primary.as_slice(), related.as_slice()],
                        |result| result.get::<_, i64>(0),
                    )
                    .map_err(sqlite)?,
                ("take.transition.v1", Some(related)) if state == TakeState::Selected => transaction
                    .query_row(
                        "SELECT count(*) FROM relationships WHERE created_by_command_id=?1 AND relationship_kind='TAKE_SUPERSEDES' AND source_kind='TAKE' AND source_id=?2 AND target_kind='TAKE' AND target_id=?3",
                        params![row.command_id.as_slice(), primary.as_slice(), related.as_slice()],
                        |result| result.get::<_, i64>(0),
                    )
                    .map_err(sqlite)?,
                ("take.transition.v1", Some(related)) if state == TakeState::Superseded => transaction
                    .query_row(
                        "SELECT count(*) FROM relationships WHERE created_by_command_id=?1 AND relationship_kind='TAKE_SUPERSEDES' AND source_kind='TAKE' AND source_id=?3 AND target_kind='TAKE' AND target_id=?2",
                        params![row.command_id.as_slice(), primary.as_slice(), related.as_slice()],
                        |result| result.get::<_, i64>(0),
                    )
                    .map_err(sqlite)?,
                ("take.transition.v1", None) => 0,
                _ => return Err(AssetStoreError::StorageCorruption),
            };
            if related_take_id.is_some() && relationship_count != 1 {
                return Err(AssetStoreError::StorageCorruption);
            }
            revision
        }
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    if revision == 0 {
        return Err(AssetStoreError::StorageCorruption);
    }
    let occurred_at = Timestamp::from_unix_seconds_nanos(
        row.updated_at_seconds,
        u32::try_from(row.updated_at_nanos).map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)?;
    Ok(CommandResult::Versioned(VersionedCommandResult::new(
        primary,
        payload,
        occurred_at,
    )))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use mengxia_domain::{
        Asset, CanonicalJson, PositiveRatio, ProjectName, ProjectSpecification, Resolution,
        SubjectKind, SubjectName, TakeReason, WorkCode, WorkKind, WorkSpecification,
    };
    use mengxia_ports::{
        Command, PROJECT_CREATE_V1, PROJECT_SPEC_REVISE_V1, PureCommandValueSource,
        SUBJECT_CREATE_V1, TAKE_CREATE_V1, TAKE_REOPEN_V1, TAKE_TRANSITION_V1, WORK_CREATE_V1,
        WORK_REVISE_V1,
    };
    use mengxia_types::{RevisionNo, Sha256Digest};

    use super::*;
    use crate::asset_repository::StoreContext;
    use crate::migration::{
        LibraryIdentity, bootstrap_schema, prepare_current_library_schema, verify_bootstrap_schema,
        verify_current_library_schema,
    };
    use crate::runtime::verify_and_harden;

    const OWNER_UID: u32 = 501;

    fn fixed_id<T>(tail: u8) -> Id<T> {
        let mut bytes = [
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x00,
        ];
        bytes[15] = tail;
        Id::from_bytes(bytes).expect("fixed UUIDv7")
    }

    fn at() -> Timestamp {
        Timestamp::from_unix_seconds_nanos(1_777_000_200, 234_567_890).expect("fixed timestamp")
    }

    struct FixedValues {
        ids: Mutex<VecDeque<[u8; 16]>>,
        timestamp: Timestamp,
    }

    impl FixedValues {
        fn new(ids: impl IntoIterator<Item = [u8; 16]>) -> Arc<Self> {
            Arc::new(Self {
                ids: Mutex::new(ids.into_iter().collect()),
                timestamp: at(),
            })
        }
    }

    impl PureCommandValueSource for FixedValues {
        fn next_uuid_v7(&self) -> Result<[u8; 16], AssetStoreError> {
            self.ids
                .lock()
                .map_err(|_| AssetStoreError::Internal)?
                .pop_front()
                .ok_or(AssetStoreError::IdGenerationUnavailable)
        }

        fn now(&self) -> Result<Timestamp, AssetStoreError> {
            Ok(self.timestamp)
        }

        fn checkpoint(&self) -> Result<(), AssetStoreError> {
            Ok(())
        }
    }

    fn specification(seed: u8) -> ProjectSpecification {
        fn json(seed: u8) -> CanonicalJson {
            let bytes = format!("{{\"value\":{seed}}}").into_bytes();
            let digest: [u8; 32] = Sha256::digest(&bytes).into();
            CanonicalJson::__from_validated_object(bytes, Sha256Digest::from_bytes(digest), 65_536)
                .expect("canonical policy")
        }
        let policies = [json(seed), json(seed + 1), json(seed + 2), json(seed + 3)];
        let mut digest_input = b"MENGXIA_PROJECT_POLICIES_V1\0".to_vec();
        for policy in &policies {
            digest_input
                .extend_from_slice(&u32::try_from(policy.bytes().len()).unwrap().to_be_bytes());
            digest_input.extend_from_slice(policy.bytes());
        }
        let digest: [u8; 32] = Sha256::digest(digest_input).into();
        ProjectSpecification::new(
            Some(Resolution::new(1920, 1080).unwrap()),
            Some(PositiveRatio::new(24, 1).unwrap()),
            Some(PositiveRatio::new(16, 9).unwrap()),
            policies[0].clone(),
            policies[1].clone(),
            policies[2].clone(),
            policies[3].clone(),
            Sha256Digest::from_bytes(digest),
        )
    }

    fn work_specification(
        seed: u8,
        subject_ids: impl IntoIterator<Item = Id<Subject>>,
        asset_ids: impl IntoIterator<Item = Id<Asset>>,
    ) -> WorkSpecification {
        let bytes = format!("{{\"work\":{seed}}}").into_bytes();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        WorkSpecification::new(
            CanonicalJson::__from_validated_object(
                bytes,
                Sha256Digest::from_bytes(digest),
                262_144,
            )
            .unwrap(),
            subject_ids,
            asset_ids,
        )
        .unwrap()
    }

    fn fixture(case: &str) -> (PathBuf, Connection, StoreContext) {
        let directory = std::env::temp_dir().join(format!(
            "mengxia-task009-creative-{}-{case}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create creative fixture directory");
        let mut connection =
            Connection::open(directory.join("library.sqlite3")).expect("open creative fixture");
        verify_and_harden(&connection, Duration::from_millis(5000)).expect("harden fixture");
        bootstrap_schema(
            &mut connection,
            fixed_id::<LibraryIdentity>(0x10),
            OWNER_UID,
            at(),
        )
        .expect("bootstrap fixture");
        let metadata = verify_bootstrap_schema(&connection).expect("bootstrap metadata");
        prepare_current_library_schema(&mut connection, metadata).expect("upgrade fixture");
        let metadata = verify_current_library_schema(&connection).expect("current schema");
        let context = StoreContext {
            metadata,
            runtime_id: fixed_id::<()>(0x11).to_bytes(),
        };
        (directory, connection, context)
    }

    fn binding(tail: u8, operation: mengxia_ports::OperationId) -> CommandBinding {
        CommandBinding::new(
            fixed_id::<Command>(tail),
            operation,
            Sha256Digest::from_bytes([tail; 32]),
        )
    }

    #[test]
    fn project_subject_mutations_are_atomic_and_exactly_replayable() {
        let (directory, mut connection, context) = fixture("mutations");
        let create_binding = binding(0x20, PROJECT_CREATE_V1);
        let create = CreateProjectCommand::new(
            create_binding,
            ProjectName::new("Project One").unwrap(),
            specification(1),
            FixedValues::new([
                fixed_id::<Project>(0x21).to_bytes(),
                fixed_id::<ProjectSpecRevision>(0x22).to_bytes(),
                fixed_id::<DomainEvent>(0x23).to_bytes(),
            ]),
        )
        .unwrap();
        let created = create_project(&mut connection, context, create);
        assert!(
            matches!(
                created,
                Ok(MutationOutcome::Applied(CommandResult::Versioned(_)))
            ),
            "unexpected project create outcome: {created:?}"
        );

        let replay = CreateProjectCommand::new(
            create_binding,
            ProjectName::new("Project One").unwrap(),
            specification(1),
            FixedValues::new([]),
        )
        .unwrap();
        assert!(matches!(
            create_project(&mut connection, context, replay),
            Ok(MutationOutcome::Replay(CommandResult::Versioned(_)))
        ));

        let revise = ReviseProjectSpecCommand::new(
            binding(0x24, PROJECT_SPEC_REVISE_V1),
            fixed_id::<Project>(0x21).to_bytes(),
            RevisionNo::new(1),
            specification(8),
            FixedValues::new([
                fixed_id::<ProjectSpecRevision>(0x25).to_bytes(),
                fixed_id::<DomainEvent>(0x26).to_bytes(),
            ]),
        )
        .unwrap();
        assert!(matches!(
            revise_project_spec(&mut connection, context, revise),
            Ok(MutationOutcome::Applied(CommandResult::Versioned(_)))
        ));

        let subject = CreateSubjectCommand::new(
            binding(0x27, SUBJECT_CREATE_V1),
            SubjectKind::new("person").unwrap(),
            SubjectName::new("Subject One").unwrap(),
            FixedValues::new([
                fixed_id::<Subject>(0x28).to_bytes(),
                fixed_id::<DomainEvent>(0x29).to_bytes(),
            ]),
        )
        .unwrap();
        assert!(matches!(
            create_subject(&mut connection, context, subject),
            Ok(MutationOutcome::Applied(CommandResult::Versioned(_)))
        ));

        let facts: (i64, i64, i64, Vec<u8>, i64) = connection
            .query_row(
                "SELECT (SELECT count(*) FROM projects), (SELECT count(*) FROM project_spec_revisions), (SELECT count(*) FROM subjects), (SELECT revision FROM projects), (SELECT count(*) FROM domain_events)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(facts, (1, 2, 1, 2_u64.to_be_bytes().to_vec(), 3));
        drop(connection);
        fs::remove_dir_all(directory).expect("remove creative mutation fixture");
    }

    #[test]
    fn duplicate_generated_identity_rolls_back_without_a_command_row() {
        let (directory, mut connection, context) = fixture("duplicate-id");
        let repeated = fixed_id::<Project>(0x31).to_bytes();
        let request = CreateProjectCommand::new(
            binding(0x30, PROJECT_CREATE_V1),
            ProjectName::new("No Mutation").unwrap(),
            specification(1),
            FixedValues::new([repeated, repeated, repeated]),
        )
        .unwrap();
        assert_eq!(
            create_project(&mut connection, context, request),
            Err(AssetStoreError::IdGenerationUnavailable)
        );
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM commands", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(connection);
        fs::remove_dir_all(directory).expect("remove duplicate-ID fixture");
    }

    #[test]
    fn work_and_take_history_survives_later_mutations_and_explicit_selection() {
        let (directory, mut connection, context) = fixture("work-take");
        let project_id = fixed_id::<Project>(0x41);
        create_project(
            &mut connection,
            context,
            CreateProjectCommand::new(
                binding(0x40, PROJECT_CREATE_V1),
                ProjectName::new("Creative Project").unwrap(),
                specification(1),
                FixedValues::new([
                    project_id.to_bytes(),
                    fixed_id::<ProjectSpecRevision>(0x42).to_bytes(),
                    fixed_id::<DomainEvent>(0x43).to_bytes(),
                ]),
            )
            .unwrap(),
        )
        .unwrap();
        let subject_id = fixed_id::<Subject>(0x45);
        create_subject(
            &mut connection,
            context,
            CreateSubjectCommand::new(
                binding(0x44, SUBJECT_CREATE_V1),
                SubjectKind::new("person").unwrap(),
                SubjectName::new("Lead").unwrap(),
                FixedValues::new([
                    subject_id.to_bytes(),
                    fixed_id::<DomainEvent>(0x46).to_bytes(),
                ]),
            )
            .unwrap(),
        )
        .unwrap();
        let asset_one = fixed_id::<Asset>(0x47);
        let asset_two = fixed_id::<Asset>(0x48);
        for asset in [asset_one, asset_two] {
            connection
                .execute(
                    "INSERT INTO assets (asset_id, kind, lifecycle, revision, created_at_seconds, created_at_nanos, created_by_uid) VALUES (?1, 'image', 'ACTIVE', ?2, ?3, ?4, ?5)",
                    params![asset.to_bytes().as_slice(), revision_bytes(1).as_slice(), at().unix_seconds(), i64::from(at().subsec_nanoseconds()), i64::from(OWNER_UID)],
                )
                .unwrap();
        }

        let work_id = fixed_id::<WorkItem>(0x51);
        let work_revision_id = fixed_id::<WorkRevision>(0x52);
        let create_work_binding = binding(0x50, WORK_CREATE_V1);
        let work_spec = work_specification(1, [subject_id], [asset_one]);
        assert!(matches!(
            create_work(
                &mut connection,
                context,
                CreateWorkCommand::new(
                    create_work_binding,
                    project_id,
                    WorkKind::Shot,
                    WorkCode::new("SHOT-001").unwrap(),
                    work_spec.clone(),
                    FixedValues::new([
                        work_id.to_bytes(),
                        work_revision_id.to_bytes(),
                        fixed_id::<DomainEvent>(0x53).to_bytes(),
                        fixed_id::<Relationship>(0x54).to_bytes(),
                        fixed_id::<Relationship>(0x55).to_bytes(),
                    ]),
                )
                .unwrap(),
            ),
            Ok(MutationOutcome::Applied(CommandResult::Versioned(_)))
        ));
        let revised_work_id = fixed_id::<WorkRevision>(0x57);
        assert!(matches!(
            revise_work(
                &mut connection,
                context,
                ReviseWorkCommand::new(
                    binding(0x56, WORK_REVISE_V1),
                    project_id,
                    work_id,
                    RevisionNo::new(1),
                    work_specification(2, [subject_id], [asset_one, asset_two]),
                    FixedValues::new([
                        revised_work_id.to_bytes(),
                        fixed_id::<DomainEvent>(0x58).to_bytes(),
                        fixed_id::<Relationship>(0x59).to_bytes(),
                        fixed_id::<Relationship>(0x5a).to_bytes(),
                        fixed_id::<Relationship>(0x5b).to_bytes(),
                    ]),
                )
                .unwrap(),
            ),
            Ok(MutationOutcome::Applied(CommandResult::Versioned(_)))
        ));
        let old_create_replay = CreateWorkCommand::new(
            create_work_binding,
            project_id,
            WorkKind::Shot,
            WorkCode::new("SHOT-001").unwrap(),
            work_spec,
            FixedValues::new([]),
        )
        .unwrap();
        assert!(matches!(
            create_work(&mut connection, context, old_create_replay),
            Ok(MutationOutcome::Replay(CommandResult::Versioned(_)))
        ));

        let first_take = fixed_id::<Take>(0x61);
        create_take(
            &mut connection,
            context,
            CreateTakeCommand::new(
                binding(0x60, TAKE_CREATE_V1),
                project_id,
                work_id,
                revised_work_id,
                asset_one,
                FixedValues::new([
                    first_take.to_bytes(),
                    fixed_id::<DomainEvent>(0x62).to_bytes(),
                ]),
            )
            .unwrap(),
        )
        .unwrap();
        let second_take = fixed_id::<Take>(0x64);
        create_take(
            &mut connection,
            context,
            CreateTakeCommand::new(
                binding(0x63, TAKE_CREATE_V1),
                project_id,
                work_id,
                revised_work_id,
                asset_two,
                FixedValues::new([
                    second_take.to_bytes(),
                    fixed_id::<DomainEvent>(0x65).to_bytes(),
                ]),
            )
            .unwrap(),
        )
        .unwrap();
        transition_take(
            &mut connection,
            context,
            TransitionTakeCommand::new(
                binding(0x66, TAKE_TRANSITION_V1),
                project_id,
                work_id,
                revised_work_id,
                first_take,
                RevisionNo::new(1),
                TakeTransition::Select,
                None,
                None,
                FixedValues::new([
                    fixed_id::<DomainEvent>(0x67).to_bytes(),
                    fixed_id::<DomainEvent>(0x68).to_bytes(),
                    fixed_id::<Relationship>(0x69).to_bytes(),
                ]),
            )
            .unwrap(),
        )
        .unwrap();
        let replace_binding = binding(0x6a, TAKE_TRANSITION_V1);
        assert!(matches!(
            transition_take(
                &mut connection,
                context,
                TransitionTakeCommand::new(
                    replace_binding,
                    project_id,
                    work_id,
                    revised_work_id,
                    second_take,
                    RevisionNo::new(1),
                    TakeTransition::Select,
                    None,
                    Some((first_take, RevisionNo::new(2))),
                    FixedValues::new([
                        fixed_id::<DomainEvent>(0x6b).to_bytes(),
                        fixed_id::<DomainEvent>(0x6c).to_bytes(),
                        fixed_id::<Relationship>(0x6d).to_bytes(),
                    ]),
                )
                .unwrap(),
            ),
            Ok(MutationOutcome::Applied(CommandResult::Versioned(_)))
        ));
        let ordered_events = connection
            .prepare(
                "SELECT event_type FROM domain_events WHERE command_id=?1 ORDER BY commit_sequence",
            )
            .unwrap()
            .query_map(
                [replace_binding.command_id().to_bytes().as_slice()],
                |row| row.get::<_, String>(0),
            )
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(ordered_events, ["take.superseded.v1", "take.selected.v1"]);

        let replay_replace = TransitionTakeCommand::new(
            replace_binding,
            project_id,
            work_id,
            revised_work_id,
            second_take,
            RevisionNo::new(1),
            TakeTransition::Select,
            None,
            Some((first_take, RevisionNo::new(2))),
            FixedValues::new([]),
        )
        .unwrap();
        assert!(matches!(
            transition_take(&mut connection, context, replay_replace),
            Ok(MutationOutcome::Replay(CommandResult::Versioned(_)))
        ));

        let reopened_take = fixed_id::<Take>(0x71);
        assert!(matches!(
            reopen_take(
                &mut connection,
                context,
                ReopenTakeCommand::new(
                    binding(0x70, TAKE_REOPEN_V1),
                    project_id,
                    work_id,
                    revised_work_id,
                    first_take,
                    RevisionNo::new(3),
                    asset_two,
                    FixedValues::new([
                        reopened_take.to_bytes(),
                        fixed_id::<DomainEvent>(0x72).to_bytes(),
                        fixed_id::<Relationship>(0x73).to_bytes(),
                    ]),
                )
                .unwrap(),
            ),
            Ok(MutationOutcome::Applied(CommandResult::Versioned(_)))
        ));
        let take_facts: (String, Vec<u8>, String, i64, i64) = connection
            .query_row(
                "SELECT (SELECT state FROM takes WHERE take_id=?1), (SELECT revision FROM takes WHERE take_id=?1), (SELECT state FROM takes WHERE take_id=?2), (SELECT count(*) FROM relationships WHERE relationship_kind='TAKE_SUPERSEDES'), (SELECT count(*) FROM relationships WHERE relationship_kind='TAKE_REOPENS')",
                params![first_take.to_bytes().as_slice(), reopened_take.to_bytes().as_slice()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(
            take_facts,
            (
                "SUPERSEDED".to_owned(),
                3_u64.to_be_bytes().to_vec(),
                "CANDIDATE".to_owned(),
                1,
                1
            )
        );

        // The reject constructor itself enforces the reason/related-field shape before claim.
        assert!(
            TransitionTakeCommand::new(
                binding(0x74, TAKE_TRANSITION_V1),
                project_id,
                work_id,
                revised_work_id,
                reopened_take,
                RevisionNo::new(1),
                TakeTransition::Reject,
                Some(TakeReason::new("discard").unwrap()),
                Some((first_take, RevisionNo::new(3))),
                FixedValues::new([]),
            )
            .is_err()
        );

        drop(connection);
        fs::remove_dir_all(directory).expect("remove work/take fixture");
    }
}
