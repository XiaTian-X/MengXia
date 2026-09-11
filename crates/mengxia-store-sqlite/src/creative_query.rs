use std::sync::Arc;

use mengxia_app::{parse_project_policy, parse_work_specification};
use mengxia_domain::{
    Asset, CanonicalJson, PositiveRatio, Project, ProjectName, ProjectSpecRevision,
    ProjectSpecification, Relationship, RelationshipKind, Resolution, Subject, SubjectKind,
    SubjectName, Take, TakeState, WorkCode, WorkItem, WorkKind, WorkRevision, WorkSpecification,
};
use mengxia_ports::{
    AssetPortFuture, AssetStoreError, CreativeListPosition, CreativeListQuery, CreativePage,
    CreativeQueryPort, InterruptibleSqliteControl, ProjectView, SubjectView, TakeRelationshipView,
    TakeView, WorkView,
};
use mengxia_types::{Id, Sha256Digest, Timestamp};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest as _, Sha256};

use crate::asset_query::with_controlled_read_interrupt;
use crate::asset_repository::{SqliteAssetStoreHandle, parse_revision, sqlite};

impl CreativeQueryPort for SqliteAssetStoreHandle {
    fn list_projects(
        &self,
        request: CreativeListQuery,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> AssetPortFuture<'_, CreativePage<ProjectView>> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        self.submit_read(move |connection| {
            with_controlled_read_interrupt(connection, control, |connection| {
                list_projects(connection, library_id, request)
            })
        })
    }

    fn list_subjects(
        &self,
        request: CreativeListQuery,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> AssetPortFuture<'_, CreativePage<SubjectView>> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        self.submit_read(move |connection| {
            with_controlled_read_interrupt(connection, control, |connection| {
                list_subjects(connection, library_id, request)
            })
        })
    }

    fn list_work(
        &self,
        project_id: Id<Project>,
        request: CreativeListQuery,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> AssetPortFuture<'_, CreativePage<WorkView>> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        self.submit_read(move |connection| {
            with_controlled_read_interrupt(connection, control, |connection| {
                list_work(connection, library_id, project_id, request)
            })
        })
    }

    fn list_takes(
        &self,
        project_id: Id<Project>,
        work_item_id: Id<WorkItem>,
        work_revision_id: Id<WorkRevision>,
        request: CreativeListQuery,
        control: Arc<dyn InterruptibleSqliteControl>,
    ) -> AssetPortFuture<'_, CreativePage<TakeView>> {
        let library_id = self.inner.metadata().library_id.to_bytes();
        self.submit_read(move |connection| {
            with_controlled_read_interrupt(connection, control, |connection| {
                list_takes(
                    connection,
                    library_id,
                    project_id,
                    work_item_id,
                    work_revision_id,
                    request,
                )
            })
        })
    }
}

fn timestamp(seconds: i64, nanos: i64) -> Result<Timestamp, AssetStoreError> {
    Timestamp::from_unix_seconds_nanos(
        seconds,
        u32::try_from(nanos).map_err(|_| AssetStoreError::StorageCorruption)?,
    )
    .map_err(|_| AssetStoreError::StorageCorruption)
}

fn id<T>(bytes: Vec<u8>) -> Result<Id<T>, AssetStoreError> {
    let bytes: [u8; 16] = bytes
        .try_into()
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    Id::from_bytes(bytes).map_err(|_| AssetStoreError::StorageCorruption)
}

fn digest(bytes: Vec<u8>) -> Result<Sha256Digest, AssetStoreError> {
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| AssetStoreError::StorageCorruption)?;
    Ok(Sha256Digest::from_bytes(bytes))
}

fn list_bounds(
    connection: &Connection,
    library_id: [u8; 16],
    position: CreativeListPosition,
    endpoint_sql: &str,
    endpoint_params: &[&dyn rusqlite::ToSql],
) -> Result<(u64, u64), AssetStoreError> {
    match position {
        CreativeListPosition::First => {
            let endpoint: i64 = connection
                .query_row(endpoint_sql, endpoint_params, |row| row.get(0))
                .map_err(sqlite)?;
            Ok((
                u64::try_from(endpoint).map_err(|_| AssetStoreError::StorageCorruption)?,
                0,
            ))
        }
        CreativeListPosition::After {
            library_id: cursor_library,
            snapshot_endpoint,
            last_key,
        } => {
            if cursor_library != library_id || last_key >= snapshot_endpoint {
                return Err(AssetStoreError::Validation);
            }
            Ok((snapshot_endpoint, last_key))
        }
    }
}

fn require_snapshot_endpoint(
    connection: &Connection,
    position: CreativeListPosition,
    sql: &str,
    parameters: &[&dyn rusqlite::ToSql],
) -> Result<(), AssetStoreError> {
    if matches!(position, CreativeListPosition::After { .. }) {
        let exists = connection
            .query_row(sql, parameters, |_| Ok(()))
            .optional()
            .map_err(sqlite)?
            .is_some();
        if !exists {
            return Err(AssetStoreError::StorageCorruption);
        }
    }
    Ok(())
}

type ProjectRow = (
    Vec<u8>,
    String,
    Vec<u8>,
    i64,
    i64,
    i64,
    i64,
    i64,
    Vec<u8>,
    i64,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Option<i64>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
    Vec<u8>,
);

fn project_specification(row: &ProjectRow) -> Result<ProjectSpecification, AssetStoreError> {
    let resolution = match (row.10, row.11) {
        (None, None) => None,
        (Some(width), Some(height)) => Some(
            Resolution::new(
                u32::try_from(width).map_err(|_| AssetStoreError::StorageCorruption)?,
                u32::try_from(height).map_err(|_| AssetStoreError::StorageCorruption)?,
            )
            .map_err(|_| AssetStoreError::StorageCorruption)?,
        ),
        _ => return Err(AssetStoreError::StorageCorruption),
    };
    let ratio = |numerator: Option<i64>, denominator: Option<i64>| match (numerator, denominator) {
        (None, None) => Ok(None),
        (Some(numerator), Some(denominator)) => Ok(Some(
            PositiveRatio::new(
                u32::try_from(numerator).map_err(|_| AssetStoreError::StorageCorruption)?,
                u32::try_from(denominator).map_err(|_| AssetStoreError::StorageCorruption)?,
            )
            .map_err(|_| AssetStoreError::StorageCorruption)?,
        )),
        _ => Err(AssetStoreError::StorageCorruption),
    };
    let mut canonical =
        validate_project_policies([&row.16, &row.17, &row.18, &row.19], row.20.as_slice())?;
    Ok(ProjectSpecification::new(
        resolution,
        ratio(row.12, row.13)?,
        ratio(row.14, row.15)?,
        canonical.remove(0),
        canonical.remove(0),
        canonical.remove(0),
        canonical.remove(0),
        digest(row.20.clone())?,
    ))
}

fn validate_project_policies(
    policies: [&Vec<u8>; 4],
    stored_digest: &[u8],
) -> Result<Vec<CanonicalJson>, AssetStoreError> {
    let mut canonical = Vec::with_capacity(4);
    for bytes in policies {
        let parsed = parse_project_policy(bytes).map_err(|_| AssetStoreError::StorageCorruption)?;
        if parsed.bytes() != bytes.as_slice() {
            return Err(AssetStoreError::StorageCorruption);
        }
        canonical.push(parsed);
    }
    let mut policy_input = b"MENGXIA_PROJECT_POLICIES_V1\0".to_vec();
    for policy in &canonical {
        policy_input.extend_from_slice(
            &u32::try_from(policy.bytes().len())
                .map_err(|_| AssetStoreError::StorageCorruption)?
                .to_be_bytes(),
        );
        policy_input.extend_from_slice(policy.bytes());
    }
    let computed: [u8; 32] = Sha256::digest(policy_input).into();
    if computed != digest(stored_digest.to_vec())?.to_bytes() {
        return Err(AssetStoreError::StorageCorruption);
    }
    Ok(canonical)
}

fn list_projects(
    connection: &Connection,
    library_id: [u8; 16],
    request: CreativeListQuery,
) -> Result<CreativePage<ProjectView>, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let (endpoint, after) = list_bounds(
        &transaction,
        library_id,
        request.position(),
        "SELECT coalesce(max(creation_commit_sequence),0) FROM projects",
        &[],
    )?;
    let endpoint_i64 = i64::try_from(endpoint).map_err(|_| AssetStoreError::Validation)?;
    let endpoint_params: [&dyn rusqlite::ToSql; 1] = [&endpoint_i64];
    require_snapshot_endpoint(
        &transaction,
        request.position(),
        "SELECT 1 FROM projects INDEXED BY projects_creation_idx WHERE creation_commit_sequence=?1",
        &endpoint_params,
    )?;
    let mut statement = transaction.prepare("SELECT p.project_id, p.name, p.revision, p.created_at_seconds, p.created_at_nanos, p.updated_at_seconds, p.updated_at_nanos, p.creation_commit_sequence, ps.project_spec_revision_id, ps.sequence, ps.resolution_width, ps.resolution_height, ps.frame_rate_numerator, ps.frame_rate_denominator, ps.aspect_ratio_numerator, ps.aspect_ratio_denominator, ps.color_policy_json, ps.audio_policy_json, ps.quality_policy_json, ps.privacy_policy_json, ps.policy_digest FROM projects p INDEXED BY projects_creation_idx JOIN project_spec_revisions ps ON ps.project_id=p.project_id AND ps.project_spec_revision_id=p.current_spec_revision_id WHERE p.creation_commit_sequence>?1 AND p.creation_commit_sequence<=?2 ORDER BY p.creation_commit_sequence, p.project_id LIMIT ?3").map_err(sqlite)?;
    let rows = statement
        .query_map(
            params![
                i64::try_from(after).map_err(|_| AssetStoreError::Validation)?,
                i64::try_from(endpoint).map_err(|_| AssetStoreError::Validation)?,
                i64::from(request.page_size()) + 1
            ],
            |row| -> rusqlite::Result<ProjectRow> {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                    row.get(15)?,
                    row.get(16)?,
                    row.get(17)?,
                    row.get(18)?,
                    row.get(19)?,
                    row.get(20)?,
                ))
            },
        )
        .map_err(sqlite)?;
    let mut collected = Vec::new();
    for row in rows {
        collected.push(row.map_err(sqlite)?);
    }
    drop(statement);
    let has_more = collected.len() > usize::from(request.page_size());
    if has_more {
        collected.pop();
    }
    let mut items = Vec::with_capacity(collected.len());
    for row in &collected {
        items.push(ProjectView::__from_store(
            id(row.0.clone())?,
            ProjectName::new(row.1.clone()).map_err(|_| AssetStoreError::StorageCorruption)?,
            parse_revision(&row.2)?,
            timestamp(row.3, row.4)?,
            timestamp(row.5, row.6)?,
            u64::try_from(row.7).map_err(|_| AssetStoreError::StorageCorruption)?,
            id(row.8.clone())?,
            u32::try_from(row.9).map_err(|_| AssetStoreError::StorageCorruption)?,
            project_specification(row)?,
        ));
    }
    let next = if has_more {
        Some(CreativeListPosition::after(
            library_id,
            endpoint,
            u64::try_from(collected.last().ok_or(AssetStoreError::Internal)?.7)
                .map_err(|_| AssetStoreError::StorageCorruption)?,
        )?)
    } else {
        None
    };
    transaction.commit().map_err(sqlite)?;
    Ok(CreativePage::__from_store(endpoint, items, next))
}

fn list_subjects(
    connection: &Connection,
    library_id: [u8; 16],
    request: CreativeListQuery,
) -> Result<CreativePage<SubjectView>, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let (endpoint, after) = list_bounds(
        &transaction,
        library_id,
        request.position(),
        "SELECT coalesce(max(creation_commit_sequence),0) FROM subjects",
        &[],
    )?;
    let endpoint_i64 = i64::try_from(endpoint).map_err(|_| AssetStoreError::Validation)?;
    let endpoint_params: [&dyn rusqlite::ToSql; 1] = [&endpoint_i64];
    require_snapshot_endpoint(
        &transaction,
        request.position(),
        "SELECT 1 FROM subjects INDEXED BY subjects_creation_idx WHERE creation_commit_sequence=?1",
        &endpoint_params,
    )?;
    let mut statement = transaction.prepare("SELECT subject_id, kind, canonical_name, revision, created_at_seconds, created_at_nanos, creation_commit_sequence FROM subjects INDEXED BY subjects_creation_idx WHERE creation_commit_sequence>?1 AND creation_commit_sequence<=?2 ORDER BY creation_commit_sequence, subject_id LIMIT ?3").map_err(sqlite)?;
    let rows = statement
        .query_map(
            params![
                i64::try_from(after).map_err(|_| AssetStoreError::Validation)?,
                i64::try_from(endpoint).map_err(|_| AssetStoreError::Validation)?,
                i64::from(request.page_size()) + 1
            ],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .map_err(sqlite)?;
    let mut collected = rows.collect::<Result<Vec<_>, _>>().map_err(sqlite)?;
    drop(statement);
    let has_more = collected.len() > usize::from(request.page_size());
    if has_more {
        collected.pop();
    }
    let items = collected
        .iter()
        .map(|row| {
            Ok(SubjectView::__from_store(
                id(row.0.clone())?,
                SubjectKind::new(row.1.clone()).map_err(|_| AssetStoreError::StorageCorruption)?,
                SubjectName::new(row.2.clone()).map_err(|_| AssetStoreError::StorageCorruption)?,
                parse_revision(&row.3)?,
                timestamp(row.4, row.5)?,
                u64::try_from(row.6).map_err(|_| AssetStoreError::StorageCorruption)?,
            ))
        })
        .collect::<Result<Vec<_>, AssetStoreError>>()?;
    let next = if has_more {
        Some(CreativeListPosition::after(
            library_id,
            endpoint,
            u64::try_from(collected.last().ok_or(AssetStoreError::Internal)?.6)
                .map_err(|_| AssetStoreError::StorageCorruption)?,
        )?)
    } else {
        None
    };
    transaction.commit().map_err(sqlite)?;
    Ok(CreativePage::__from_store(endpoint, items, next))
}

fn list_work(
    connection: &Connection,
    library_id: [u8; 16],
    project_id: Id<Project>,
    request: CreativeListQuery,
) -> Result<CreativePage<WorkView>, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let project = project_id.to_bytes();
    let project_exists = transaction
        .query_row(
            "SELECT 1 FROM projects WHERE project_id=?1",
            [project.as_slice()],
            |_| Ok(()),
        )
        .optional()
        .map_err(sqlite)?
        .is_some();
    if !project_exists {
        return Err(AssetStoreError::NotFound);
    }
    let endpoint_params: [&dyn rusqlite::ToSql; 1] = [&project.as_slice()];
    let (endpoint, after) = list_bounds(
        &transaction,
        library_id,
        request.position(),
        "SELECT coalesce(max(creation_commit_sequence),0) FROM work_items WHERE project_id=?1",
        &endpoint_params,
    )?;
    let endpoint_i64 = i64::try_from(endpoint).map_err(|_| AssetStoreError::Validation)?;
    let endpoint_exists_params: [&dyn rusqlite::ToSql; 2] = [&project.as_slice(), &endpoint_i64];
    require_snapshot_endpoint(
        &transaction,
        request.position(),
        "SELECT 1 FROM work_items INDEXED BY work_items_project_creation_idx WHERE project_id=?1 AND creation_commit_sequence=?2",
        &endpoint_exists_params,
    )?;
    let mut statement = transaction.prepare("SELECT w.work_item_id,w.kind,w.code,w.revision,w.created_at_seconds,w.created_at_nanos,w.updated_at_seconds,w.updated_at_nanos,w.creation_commit_sequence,wr.work_revision_id,wr.sequence,wr.specification_json,wr.specification_digest FROM work_items w INDEXED BY work_items_project_creation_idx JOIN work_revisions wr ON wr.work_item_id=w.work_item_id AND wr.work_revision_id=w.current_work_revision_id WHERE w.project_id=?1 AND w.creation_commit_sequence>?2 AND w.creation_commit_sequence<=?3 ORDER BY w.creation_commit_sequence,w.work_item_id LIMIT ?4").map_err(sqlite)?;
    let rows = statement
        .query_map(
            params![
                project.as_slice(),
                i64::try_from(after).map_err(|_| AssetStoreError::Validation)?,
                i64::try_from(endpoint).map_err(|_| AssetStoreError::Validation)?,
                i64::from(request.page_size()) + 1
            ],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, Vec<u8>>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, Vec<u8>>(11)?,
                    row.get::<_, Vec<u8>>(12)?,
                ))
            },
        )
        .map_err(sqlite)?;
    let mut collected = rows.collect::<Result<Vec<_>, _>>().map_err(sqlite)?;
    drop(statement);
    let has_more = collected.len() > usize::from(request.page_size());
    if has_more {
        collected.pop();
    }
    let mut items = Vec::with_capacity(collected.len());
    for row in &collected {
        let revision_id: Id<WorkRevision> = id(row.9.clone())?;
        let subjects = relationship_targets::<Subject>(
            &transaction,
            revision_id.to_bytes(),
            "WORK_SUBJECT",
            "SUBJECT",
            64,
        )?;
        let assets = relationship_targets::<Asset>(
            &transaction,
            revision_id.to_bytes(),
            "WORK_ASSET",
            "ASSET",
            64,
        )?;
        let stored_digest = digest(row.12.clone())?;
        let computed: [u8; 32] = Sha256::digest(&row.11).into();
        if computed != stored_digest.to_bytes() {
            return Err(AssetStoreError::StorageCorruption);
        }
        let json =
            parse_work_specification(&row.11).map_err(|_| AssetStoreError::StorageCorruption)?;
        if json.bytes() != row.11.as_slice() || json.digest() != stored_digest {
            return Err(AssetStoreError::StorageCorruption);
        }
        let kind = match row.1.as_str() {
            "SCENE" => WorkKind::Scene,
            "SHOT" => WorkKind::Shot,
            _ => return Err(AssetStoreError::StorageCorruption),
        };
        items.push(WorkView::__from_store(
            id(row.0.clone())?,
            project_id,
            kind,
            WorkCode::new(row.2.clone()).map_err(|_| AssetStoreError::StorageCorruption)?,
            parse_revision(&row.3)?,
            timestamp(row.4, row.5)?,
            timestamp(row.6, row.7)?,
            u64::try_from(row.8).map_err(|_| AssetStoreError::StorageCorruption)?,
            revision_id,
            u32::try_from(row.10).map_err(|_| AssetStoreError::StorageCorruption)?,
            WorkSpecification::new(json, subjects, assets)
                .map_err(|_| AssetStoreError::StorageCorruption)?,
        ));
    }
    let next = if has_more {
        Some(CreativeListPosition::after(
            library_id,
            endpoint,
            u64::try_from(collected.last().ok_or(AssetStoreError::Internal)?.8)
                .map_err(|_| AssetStoreError::StorageCorruption)?,
        )?)
    } else {
        None
    };
    transaction.commit().map_err(sqlite)?;
    Ok(CreativePage::__from_store(endpoint, items, next))
}

fn relationship_targets<T>(
    connection: &Connection,
    source: [u8; 16],
    kind: &str,
    target_kind: &str,
    maximum: usize,
) -> Result<Vec<Id<T>>, AssetStoreError> {
    let mut statement=connection.prepare("SELECT target_id FROM relationships INDEXED BY relationships_source_idx WHERE source_kind='WORK_REVISION' AND source_id=?1 AND relationship_kind=?2 AND target_kind=?3 ORDER BY relationship_kind,target_id LIMIT ?4").map_err(sqlite)?;
    let rows = statement
        .query_map(
            params![
                source.as_slice(),
                kind,
                target_kind,
                i64::try_from(maximum + 1).map_err(|_| AssetStoreError::Internal)?
            ],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .map_err(sqlite)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sqlite)?;
    if rows.len() > maximum {
        return Err(AssetStoreError::StorageCorruption);
    }
    rows.into_iter().map(id).collect()
}

fn list_takes(
    connection: &Connection,
    library_id: [u8; 16],
    project_id: Id<Project>,
    work_item_id: Id<WorkItem>,
    work_revision_id: Id<WorkRevision>,
    request: CreativeListQuery,
) -> Result<CreativePage<TakeView>, AssetStoreError> {
    let transaction = connection.unchecked_transaction().map_err(sqlite)?;
    let context_exists=transaction.query_row("SELECT 1 FROM work_items w JOIN work_revisions wr ON wr.work_item_id=w.work_item_id WHERE w.project_id=?1 AND w.work_item_id=?2 AND wr.work_revision_id=?3",params![project_id.to_bytes().as_slice(),work_item_id.to_bytes().as_slice(),work_revision_id.to_bytes().as_slice()],|_|Ok(())).optional().map_err(sqlite)?.is_some();
    if !context_exists {
        return Err(AssetStoreError::NotFound);
    }
    let scope = work_revision_id.to_bytes();
    let endpoint_params: [&dyn rusqlite::ToSql; 1] = [&scope.as_slice()];
    let (endpoint, after) = list_bounds(
        &transaction,
        library_id,
        request.position(),
        "SELECT coalesce(max(ordinal),0) FROM takes WHERE work_revision_id=?1",
        &endpoint_params,
    )?;
    let endpoint_i64 = i64::try_from(endpoint).map_err(|_| AssetStoreError::Validation)?;
    let endpoint_exists_params: [&dyn rusqlite::ToSql; 2] = [&scope.as_slice(), &endpoint_i64];
    require_snapshot_endpoint(
        &transaction,
        request.position(),
        "SELECT 1 FROM takes INDEXED BY takes_work_revision_ordinal_idx WHERE work_revision_id=?1 AND ordinal=?2",
        &endpoint_exists_params,
    )?;
    let mut statement=transaction.prepare("SELECT take_id,ordinal,state,primary_asset_id,revision,created_at_seconds,created_at_nanos,updated_at_seconds,updated_at_nanos FROM takes INDEXED BY takes_work_revision_ordinal_idx WHERE work_revision_id=?1 AND ordinal>?2 AND ordinal<=?3 ORDER BY ordinal,take_id LIMIT ?4").map_err(sqlite)?;
    let rows = statement
        .query_map(
            params![
                scope.as_slice(),
                i64::try_from(after).map_err(|_| AssetStoreError::Validation)?,
                i64::try_from(endpoint).map_err(|_| AssetStoreError::Validation)?,
                i64::from(request.page_size()) + 1
            ],
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            },
        )
        .map_err(sqlite)?;
    let mut collected = rows.collect::<Result<Vec<_>, _>>().map_err(sqlite)?;
    drop(statement);
    let has_more = collected.len() > usize::from(request.page_size());
    if has_more {
        collected.pop();
    }
    let mut items = Vec::with_capacity(collected.len());
    for row in &collected {
        let take_id: Id<Take> = id(row.0.clone())?;
        let mut rel_statement=transaction.prepare("SELECT relationship_id,relationship_kind,target_id FROM relationships INDEXED BY relationships_source_idx WHERE source_kind='TAKE' AND source_id=?1 AND relationship_kind IN ('TAKE_REOPENS','TAKE_SUPERSEDES') ORDER BY relationship_kind,target_id LIMIT 3").map_err(sqlite)?;
        let rels = rel_statement
            .query_map([take_id.to_bytes().as_slice()], |r| {
                Ok((
                    r.get::<_, Vec<u8>>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                ))
            })
            .map_err(sqlite)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sqlite)?;
        if rels.len() > 2 {
            return Err(AssetStoreError::StorageCorruption);
        }
        let relationships = rels
            .into_iter()
            .map(|(rid, kind, target)| {
                Ok(TakeRelationshipView::__from_store(
                    id::<Relationship>(rid)?,
                    match kind.as_str() {
                        "TAKE_REOPENS" => RelationshipKind::TakeReopens,
                        "TAKE_SUPERSEDES" => RelationshipKind::TakeSupersedes,
                        _ => return Err(AssetStoreError::StorageCorruption),
                    },
                    id::<Take>(target)?,
                ))
            })
            .collect::<Result<Vec<_>, AssetStoreError>>()?;
        items.push(TakeView::__from_store(
            take_id,
            work_revision_id,
            u32::try_from(row.1).map_err(|_| AssetStoreError::StorageCorruption)?,
            TakeState::parse(&row.2).map_err(|_| AssetStoreError::StorageCorruption)?,
            id::<Asset>(row.3.clone())?,
            parse_revision(&row.4)?,
            timestamp(row.5, row.6)?,
            timestamp(row.7, row.8)?,
            relationships,
        ));
    }
    let next = if has_more {
        Some(CreativeListPosition::after(
            library_id,
            endpoint,
            u64::try_from(collected.last().ok_or(AssetStoreError::Internal)?.1)
                .map_err(|_| AssetStoreError::StorageCorruption)?,
        )?)
    } else {
        None
    };
    transaction.commit().map_err(sqlite)?;
    Ok(CreativePage::__from_store(endpoint, items, next))
}

pub(crate) fn verify_persisted_creative_state(
    connection: &Connection,
) -> Result<(), AssetStoreError> {
    let mut statement = connection
        .prepare("SELECT project_spec_revision_id,project_id,sequence,resolution_width,resolution_height,frame_rate_numerator,frame_rate_denominator,aspect_ratio_numerator,aspect_ratio_denominator,color_policy_json,audio_policy_json,quality_policy_json,privacy_policy_json,policy_digest,created_by_command_id,created_at_seconds,created_at_nanos FROM project_spec_revisions ORDER BY project_id,sequence")
        .map_err(sqlite)?;
    let mut rows = statement.query([]).map_err(sqlite)?;
    while let Some(row) = rows.next().map_err(sqlite)? {
        id::<ProjectSpecRevision>(row.get(0).map_err(sqlite)?)?;
        id::<Project>(row.get(1).map_err(sqlite)?)?;
        let sequence = u32::try_from(row.get::<_, i64>(2).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        if sequence == 0 {
            return Err(AssetStoreError::StorageCorruption);
        }
        let pair = |left: Option<i64>, right: Option<i64>| match (left, right) {
            (None, None) => Ok(()),
            (Some(left), Some(right)) => PositiveRatio::new(
                u32::try_from(left).map_err(|_| AssetStoreError::StorageCorruption)?,
                u32::try_from(right).map_err(|_| AssetStoreError::StorageCorruption)?,
            )
            .map(|_| ())
            .map_err(|_| AssetStoreError::StorageCorruption),
            _ => Err(AssetStoreError::StorageCorruption),
        };
        match (
            row.get::<_, Option<i64>>(3).map_err(sqlite)?,
            row.get::<_, Option<i64>>(4).map_err(sqlite)?,
        ) {
            (None, None) => {}
            (Some(width), Some(height)) => {
                Resolution::new(
                    u32::try_from(width).map_err(|_| AssetStoreError::StorageCorruption)?,
                    u32::try_from(height).map_err(|_| AssetStoreError::StorageCorruption)?,
                )
                .map_err(|_| AssetStoreError::StorageCorruption)?;
            }
            _ => return Err(AssetStoreError::StorageCorruption),
        }
        pair(row.get(5).map_err(sqlite)?, row.get(6).map_err(sqlite)?)?;
        pair(row.get(7).map_err(sqlite)?, row.get(8).map_err(sqlite)?)?;
        let policies = [
            row.get::<_, Vec<u8>>(9).map_err(sqlite)?,
            row.get::<_, Vec<u8>>(10).map_err(sqlite)?,
            row.get::<_, Vec<u8>>(11).map_err(sqlite)?,
            row.get::<_, Vec<u8>>(12).map_err(sqlite)?,
        ];
        let stored_digest = row.get::<_, Vec<u8>>(13).map_err(sqlite)?;
        validate_project_policies(
            [&policies[0], &policies[1], &policies[2], &policies[3]],
            &stored_digest,
        )?;
        id::<()>(row.get(14).map_err(sqlite)?)?;
        timestamp(row.get(15).map_err(sqlite)?, row.get(16).map_err(sqlite)?)?;
    }
    drop(rows);
    drop(statement);

    let mut statement = connection
        .prepare("SELECT p.project_id,p.name,p.revision,p.current_spec_revision_id,ps.sequence,p.creation_commit_sequence,p.created_at_seconds,p.created_at_nanos,p.updated_at_seconds,p.updated_at_nanos,p.created_by_command_id FROM projects p JOIN project_spec_revisions ps ON ps.project_id=p.project_id AND ps.project_spec_revision_id=p.current_spec_revision_id ORDER BY p.project_id")
        .map_err(sqlite)?;
    let mut rows = statement.query([]).map_err(sqlite)?;
    while let Some(row) = rows.next().map_err(sqlite)? {
        id::<Project>(row.get(0).map_err(sqlite)?)?;
        ProjectName::new(row.get::<_, String>(1).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        let revision = parse_revision(&row.get::<_, Vec<u8>>(2).map_err(sqlite)?)?;
        id::<ProjectSpecRevision>(row.get(3).map_err(sqlite)?)?;
        let sequence = u32::try_from(row.get::<_, i64>(4).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        if revision.get() == 0
            || revision.get() != u64::from(sequence)
            || row.get::<_, i64>(5).map_err(sqlite)? <= 0
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        timestamp(row.get(6).map_err(sqlite)?, row.get(7).map_err(sqlite)?)?;
        timestamp(row.get(8).map_err(sqlite)?, row.get(9).map_err(sqlite)?)?;
        id::<()>(row.get(10).map_err(sqlite)?)?;
    }
    drop(rows);
    drop(statement);

    let mut statement = connection
        .prepare("SELECT work_revision_id,work_item_id,sequence,specification_json,specification_digest,created_by_command_id,created_at_seconds,created_at_nanos FROM work_revisions ORDER BY work_item_id,sequence")
        .map_err(sqlite)?;
    let mut rows = statement.query([]).map_err(sqlite)?;
    while let Some(row) = rows.next().map_err(sqlite)? {
        id::<WorkRevision>(row.get(0).map_err(sqlite)?)?;
        id::<WorkItem>(row.get(1).map_err(sqlite)?)?;
        let sequence = u32::try_from(row.get::<_, i64>(2).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        if sequence == 0 {
            return Err(AssetStoreError::StorageCorruption);
        }
        let bytes = row.get::<_, Vec<u8>>(3).map_err(sqlite)?;
        let stored = digest(row.get(4).map_err(sqlite)?)?;
        let parsed =
            parse_work_specification(&bytes).map_err(|_| AssetStoreError::StorageCorruption)?;
        if parsed.bytes() != bytes || parsed.digest() != stored {
            return Err(AssetStoreError::StorageCorruption);
        }
        id::<()>(row.get(5).map_err(sqlite)?)?;
        timestamp(row.get(6).map_err(sqlite)?, row.get(7).map_err(sqlite)?)?;
    }
    drop(rows);
    drop(statement);

    let mut statement = connection
        .prepare("SELECT w.work_item_id,w.project_id,w.kind,w.code,w.revision,w.current_work_revision_id,wr.sequence,w.creation_commit_sequence,w.created_at_seconds,w.created_at_nanos,w.updated_at_seconds,w.updated_at_nanos,w.created_by_command_id FROM work_items w JOIN work_revisions wr ON wr.work_item_id=w.work_item_id AND wr.work_revision_id=w.current_work_revision_id ORDER BY w.work_item_id")
        .map_err(sqlite)?;
    let mut rows = statement.query([]).map_err(sqlite)?;
    while let Some(row) = rows.next().map_err(sqlite)? {
        id::<WorkItem>(row.get(0).map_err(sqlite)?)?;
        id::<Project>(row.get(1).map_err(sqlite)?)?;
        match row.get::<_, String>(2).map_err(sqlite)?.as_str() {
            "SCENE" | "SHOT" => {}
            _ => return Err(AssetStoreError::StorageCorruption),
        }
        WorkCode::new(row.get::<_, String>(3).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        let revision = parse_revision(&row.get::<_, Vec<u8>>(4).map_err(sqlite)?)?;
        id::<WorkRevision>(row.get(5).map_err(sqlite)?)?;
        let sequence = u32::try_from(row.get::<_, i64>(6).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        if revision.get() == 0
            || revision.get() != u64::from(sequence)
            || row.get::<_, i64>(7).map_err(sqlite)? <= 0
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        timestamp(row.get(8).map_err(sqlite)?, row.get(9).map_err(sqlite)?)?;
        timestamp(row.get(10).map_err(sqlite)?, row.get(11).map_err(sqlite)?)?;
        id::<()>(row.get(12).map_err(sqlite)?)?;
    }
    drop(rows);
    drop(statement);

    let mut statement = connection
        .prepare("SELECT subject_id,kind,canonical_name,revision,creation_commit_sequence,created_by_command_id,created_at_seconds,created_at_nanos FROM subjects ORDER BY subject_id")
        .map_err(sqlite)?;
    let mut rows = statement.query([]).map_err(sqlite)?;
    while let Some(row) = rows.next().map_err(sqlite)? {
        id::<Subject>(row.get(0).map_err(sqlite)?)?;
        SubjectKind::new(row.get::<_, String>(1).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        SubjectName::new(row.get::<_, String>(2).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        if parse_revision(&row.get::<_, Vec<u8>>(3).map_err(sqlite)?)?.get() == 0
            || row.get::<_, i64>(4).map_err(sqlite)? <= 0
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        id::<()>(row.get(5).map_err(sqlite)?)?;
        timestamp(row.get(6).map_err(sqlite)?, row.get(7).map_err(sqlite)?)?;
    }
    drop(rows);
    drop(statement);

    let mut statement = connection
        .prepare("SELECT take_id,work_revision_id,ordinal,state,primary_asset_id,revision,creation_commit_sequence,created_by_command_id,created_at_seconds,created_at_nanos,updated_at_seconds,updated_at_nanos FROM takes ORDER BY work_revision_id,ordinal")
        .map_err(sqlite)?;
    let mut rows = statement.query([]).map_err(sqlite)?;
    while let Some(row) = rows.next().map_err(sqlite)? {
        id::<Take>(row.get(0).map_err(sqlite)?)?;
        id::<WorkRevision>(row.get(1).map_err(sqlite)?)?;
        let ordinal = u32::try_from(row.get::<_, i64>(2).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        if ordinal == 0 {
            return Err(AssetStoreError::StorageCorruption);
        }
        TakeState::parse(&row.get::<_, String>(3).map_err(sqlite)?)
            .map_err(|_| AssetStoreError::StorageCorruption)?;
        id::<Asset>(row.get(4).map_err(sqlite)?)?;
        if parse_revision(&row.get::<_, Vec<u8>>(5).map_err(sqlite)?)?.get() == 0
            || row.get::<_, i64>(6).map_err(sqlite)? <= 0
        {
            return Err(AssetStoreError::StorageCorruption);
        }
        id::<()>(row.get(7).map_err(sqlite)?)?;
        timestamp(row.get(8).map_err(sqlite)?, row.get(9).map_err(sqlite)?)?;
        timestamp(row.get(10).map_err(sqlite)?, row.get(11).map_err(sqlite)?)?;
    }
    drop(rows);
    drop(statement);

    let bad_sequence: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM (SELECT project_id,count(*) AS n,min(sequence) AS lo,max(sequence) AS hi FROM project_spec_revisions GROUP BY project_id HAVING lo<>1 OR n<>hi) UNION ALL SELECT 1 FROM (SELECT work_item_id,count(*) AS n,min(sequence) AS lo,max(sequence) AS hi FROM work_revisions GROUP BY work_item_id HAVING lo<>1 OR n<>hi) UNION ALL SELECT 1 FROM (SELECT work_revision_id,count(*) AS n,min(ordinal) AS lo,max(ordinal) AS hi FROM takes GROUP BY work_revision_id HAVING lo<>1 OR n<>hi) LIMIT 1)",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite)?;
    let bad_relationship: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM relationships r WHERE NOT ((r.relationship_kind='WORK_SUBJECT' AND r.source_kind='WORK_REVISION' AND r.target_kind='SUBJECT' AND EXISTS(SELECT 1 FROM work_revisions wr WHERE wr.work_revision_id=r.source_id) AND EXISTS(SELECT 1 FROM subjects s WHERE s.subject_id=r.target_id)) OR (r.relationship_kind='WORK_ASSET' AND r.source_kind='WORK_REVISION' AND r.target_kind='ASSET' AND EXISTS(SELECT 1 FROM work_revisions wr WHERE wr.work_revision_id=r.source_id) AND EXISTS(SELECT 1 FROM assets a WHERE a.asset_id=r.target_id)) OR (r.relationship_kind IN ('TAKE_REOPENS','TAKE_SUPERSEDES') AND r.source_kind='TAKE' AND r.target_kind='TAKE' AND EXISTS(SELECT 1 FROM takes source JOIN takes target ON target.take_id=r.target_id AND target.work_revision_id=source.work_revision_id WHERE source.take_id=r.source_id AND target.state IN ('APPROVED','REJECTED','SUPERSEDED')))) UNION ALL SELECT 1 FROM relationships GROUP BY source_kind,source_id,relationship_kind HAVING (relationship_kind IN ('WORK_SUBJECT','WORK_ASSET') AND count(*)>64) OR (relationship_kind IN ('TAKE_REOPENS','TAKE_SUPERSEDES') AND count(*)>1) LIMIT 1)",
            [],
            |row| row.get(0),
        )
        .map_err(sqlite)?;
    if bad_sequence != 0 || bad_relationship != 0 {
        return Err(AssetStoreError::StorageCorruption);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use mengxia_ports::CreativeListPosition;

    use super::*;
    use crate::migration::{
        LibraryIdentity, bootstrap_schema, prepare_current_library_schema, verify_bootstrap_schema,
        verify_current_library_schema,
    };
    use crate::runtime::verify_and_harden;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    fn fixed_id<T>(tail: u8) -> Id<T> {
        let mut bytes = [
            0x01, 0x8d, 0x44, 0x2f, 0xc0, 0x00, 0x7a, 0x11, 0x80, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x00,
        ];
        bytes[15] = tail;
        Id::from_bytes(bytes).unwrap()
    }

    fn fixture() -> (std::path::PathBuf, Connection, [u8; 16]) {
        let directory = std::env::temp_dir().join(format!(
            "mengxia-task009-query-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();
        let mut connection = Connection::open(directory.join("library.sqlite3")).unwrap();
        verify_and_harden(&connection, Duration::from_secs(5)).unwrap();
        let library_id = fixed_id::<LibraryIdentity>(1);
        let at = Timestamp::from_unix_seconds_nanos(1_777_000_300, 123).unwrap();
        bootstrap_schema(&mut connection, library_id, 501, at).unwrap();
        let metadata = verify_bootstrap_schema(&connection).unwrap();
        prepare_current_library_schema(&mut connection, metadata).unwrap();
        verify_current_library_schema(&connection).unwrap();
        (directory, connection, library_id.to_bytes())
    }

    fn insert_subject(connection: &Connection, ordinal: u8) {
        connection.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        connection
            .execute(
                "INSERT INTO subjects(subject_id,kind,canonical_name,revision,created_by_command_id,created_at_seconds,created_at_nanos,creation_commit_sequence) VALUES(?1,'person',?2,?3,?4,100,0,?5)",
                params![
                    fixed_id::<Subject>(ordinal).to_bytes().as_slice(),
                    format!("subject-{ordinal}"),
                    1_u64.to_be_bytes().as_slice(),
                    fixed_id::<()>(ordinal.wrapping_add(64)).to_bytes().as_slice(),
                    i64::from(ordinal),
                ],
            )
            .unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    }

    fn insert_project(connection: &Connection, ordinal: u8, policy: &[u8]) -> Id<Project> {
        let project_id = fixed_id::<Project>(ordinal);
        let spec_id = fixed_id::<mengxia_domain::ProjectSpecRevision>(ordinal.wrapping_add(32));
        let mut digest_input = b"MENGXIA_PROJECT_POLICIES_V1\0".to_vec();
        for _ in 0..4 {
            digest_input.extend_from_slice(&u32::try_from(policy.len()).unwrap().to_be_bytes());
            digest_input.extend_from_slice(policy);
        }
        let policy_digest: [u8; 32] = Sha256::digest(digest_input).into();
        connection.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        connection
            .execute(
                "INSERT INTO project_spec_revisions(project_spec_revision_id,project_id,sequence,policy_schema_version,color_policy_json,audio_policy_json,quality_policy_json,privacy_policy_json,policy_digest,created_by_command_id,created_at_seconds,created_at_nanos) VALUES(?1,?2,1,1,?3,?3,?3,?3,?4,?5,100,0)",
                params![
                    spec_id.to_bytes().as_slice(),
                    project_id.to_bytes().as_slice(),
                    policy,
                    policy_digest.as_slice(),
                    fixed_id::<()>(ordinal.wrapping_add(64)).to_bytes().as_slice(),
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO projects(project_id,name,current_spec_revision_id,revision,creation_commit_sequence,created_by_command_id,created_at_seconds,created_at_nanos,updated_at_seconds,updated_at_nanos) VALUES(?1,?2,?3,?4,?5,?6,100,0,100,0)",
                params![
                    project_id.to_bytes().as_slice(),
                    format!("project-{ordinal}"),
                    spec_id.to_bytes().as_slice(),
                    1_u64.to_be_bytes().as_slice(),
                    i64::from(ordinal),
                    fixed_id::<()>(ordinal.wrapping_add(64)).to_bytes().as_slice(),
                ],
            )
            .unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        project_id
    }

    #[test]
    fn creative_keyset_snapshot_is_stable_and_missing_endpoint_fails_closed() {
        let (directory, connection, library_id) = fixture();
        insert_subject(&connection, 1);
        insert_subject(&connection, 2);
        insert_subject(&connection, 3);

        let first = list_subjects(
            &connection,
            library_id,
            CreativeListQuery::new(2, CreativeListPosition::First).unwrap(),
        )
        .unwrap();
        assert_eq!(first.snapshot_endpoint(), 3);
        assert_eq!(first.items().len(), 2);
        let continuation = first.next().unwrap();

        insert_subject(&connection, 4);
        let second = list_subjects(
            &connection,
            library_id,
            CreativeListQuery::new(2, continuation).unwrap(),
        )
        .unwrap();
        assert_eq!(second.snapshot_endpoint(), 3);
        assert_eq!(second.items().len(), 1);
        assert_eq!(second.items()[0].subject_id(), fixed_id::<Subject>(3));
        assert!(second.next().is_none());

        connection.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        connection
            .execute("DELETE FROM subjects WHERE creation_commit_sequence=3", [])
            .unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        assert_eq!(
            list_subjects(
                &connection,
                library_id,
                CreativeListQuery::new(2, continuation).unwrap(),
            ),
            Err(AssetStoreError::StorageCorruption)
        );
        drop(connection);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn every_creative_page_query_uses_the_accepted_bounded_index_without_sorting() {
        let (directory, connection, _) = fixture();
        let plans = [
            (
                "projects_creation_idx",
                "SELECT p.project_id FROM projects p INDEXED BY projects_creation_idx JOIN project_spec_revisions ps ON ps.project_id=p.project_id AND ps.project_spec_revision_id=p.current_spec_revision_id WHERE p.creation_commit_sequence>0 AND p.creation_commit_sequence<=100 ORDER BY p.creation_commit_sequence,p.project_id LIMIT 65",
            ),
            (
                "subjects_creation_idx",
                "SELECT subject_id FROM subjects INDEXED BY subjects_creation_idx WHERE creation_commit_sequence>0 AND creation_commit_sequence<=100 ORDER BY creation_commit_sequence,subject_id LIMIT 65",
            ),
            (
                "work_items_project_creation_idx",
                "SELECT w.work_item_id FROM work_items w INDEXED BY work_items_project_creation_idx JOIN work_revisions wr ON wr.work_item_id=w.work_item_id AND wr.work_revision_id=w.current_work_revision_id WHERE w.project_id=zeroblob(16) AND w.creation_commit_sequence>0 AND w.creation_commit_sequence<=100 ORDER BY w.creation_commit_sequence,w.work_item_id LIMIT 65",
            ),
            (
                "takes_work_revision_ordinal_idx",
                "SELECT take_id FROM takes INDEXED BY takes_work_revision_ordinal_idx WHERE work_revision_id=zeroblob(16) AND ordinal>0 AND ordinal<=100 ORDER BY ordinal,take_id LIMIT 65",
            ),
        ];
        for (expected_index, sql) in plans {
            let mut statement = connection
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap();
            let details = statement
                .query_map([], |row| row.get::<_, String>(3))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            assert!(
                details.iter().any(|detail| detail.contains(expected_index)),
                "missing {expected_index}: {details:?}"
            );
            assert!(
                details.iter().all(|detail| !detail.contains("TEMP B-TREE")),
                "query must not sort outside the accepted index: {details:?}"
            );
        }
        drop(connection);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn list_work_distinguishes_an_empty_project_from_a_missing_scope() {
        let (directory, connection, library_id) = fixture();
        let project_id = insert_project(&connection, 10, b"{}");
        let request = CreativeListQuery::new(64, CreativeListPosition::First).unwrap();

        let empty = list_work(&connection, library_id, project_id, request).unwrap();
        assert!(empty.items().is_empty());
        assert_eq!(
            list_work(&connection, library_id, fixed_id::<Project>(11), request),
            Err(AssetStoreError::NotFound)
        );

        drop(connection);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn creative_queries_reject_digest_consistent_noncanonical_json() {
        let (directory, connection, library_id) = fixture();
        let project_id = insert_project(&connection, 20, br#"{"z":1,"a":2}"#);
        assert_eq!(
            list_projects(
                &connection,
                library_id,
                CreativeListQuery::new(64, CreativeListPosition::First).unwrap(),
            ),
            Err(AssetStoreError::StorageCorruption)
        );

        let work_id = fixed_id::<WorkItem>(21);
        let work_revision_id = fixed_id::<WorkRevision>(22);
        let specification = br#"{"z":1,"a":2}"#;
        let specification_digest: [u8; 32] = Sha256::digest(specification).into();
        connection.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        connection
            .execute(
                "INSERT INTO work_revisions(work_revision_id,work_item_id,sequence,specification_schema_version,specification_json,specification_digest,created_by_command_id,created_at_seconds,created_at_nanos) VALUES(?1,?2,1,1,?3,?4,?5,100,0)",
                params![
                    work_revision_id.to_bytes().as_slice(),
                    work_id.to_bytes().as_slice(),
                    specification,
                    specification_digest.as_slice(),
                    fixed_id::<()>(85).to_bytes().as_slice(),
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO work_items(work_item_id,project_id,kind,code,current_work_revision_id,revision,creation_commit_sequence,created_by_command_id,created_at_seconds,created_at_nanos,updated_at_seconds,updated_at_nanos) VALUES(?1,?2,'SHOT','SHOT-1',?3,?4,?5,?6,100,0,100,0)",
                params![
                    work_id.to_bytes().as_slice(),
                    project_id.to_bytes().as_slice(),
                    work_revision_id.to_bytes().as_slice(),
                    1_u64.to_be_bytes().as_slice(),
                    21_i64,
                    fixed_id::<()>(85).to_bytes().as_slice(),
                ],
            )
            .unwrap();
        connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
        assert_eq!(
            list_work(
                &connection,
                library_id,
                project_id,
                CreativeListQuery::new(64, CreativeListPosition::First).unwrap(),
            ),
            Err(AssetStoreError::StorageCorruption)
        );

        drop(connection);
        fs::remove_dir_all(directory).unwrap();
    }
}
