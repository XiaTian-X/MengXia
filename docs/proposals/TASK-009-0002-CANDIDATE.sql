CREATE TABLE commands_v2 (
    command_id BLOB PRIMARY KEY NOT NULL CHECK (length(command_id) = 16),
    operation_id TEXT NOT NULL
        CHECK (length(CAST(operation_id AS BLOB)) BETWEEN 1 AND 128)
        CHECK (substr(operation_id, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (operation_id NOT GLOB '*[^a-z0-9._-]*')
        CHECK (operation_id GLOB '*.v1'),
    principal_kind TEXT NOT NULL CHECK (principal_kind = 'LOCAL_OWNER_UID_V1'),
    principal_uid INTEGER NOT NULL CHECK (principal_uid BETWEEN 0 AND 4294967295),
    canonical_request_digest BLOB NOT NULL CHECK (length(canonical_request_digest) = 32),
    store_runtime_id BLOB NOT NULL CHECK (length(store_runtime_id) = 16),
    state TEXT NOT NULL
        CHECK (state IN ('CLAIMED', 'COMPLETED', 'TERMINAL_REJECTED', 'RECOVERY_REQUIRED')),
    result_kind TEXT
        CHECK (result_kind IS NULL OR (
            length(CAST(result_kind AS BLOB)) BETWEEN 1 AND 64
            AND substr(result_kind, 1, 1) BETWEEN 'A' AND 'Z'
            AND result_kind NOT GLOB '*[^A-Z0-9_]*'
        )),
    result_id BLOB CHECK (result_id IS NULL OR length(result_id) = 16),
    result_location_id BLOB
        CHECK (result_location_id IS NULL OR length(result_location_id) = 16),
    result_schema_version INTEGER
        CHECK (result_schema_version IS NULL OR result_schema_version BETWEEN 1 AND 65535),
    result_payload BLOB
        CHECK (result_payload IS NULL OR length(result_payload) BETWEEN 1 AND 512),
    result_payload_sha256 BLOB
        CHECK (result_payload_sha256 IS NULL OR length(result_payload_sha256) = 32),
    safe_error_code TEXT
        CHECK (safe_error_code IS NULL OR length(CAST(safe_error_code AS BLOB)) BETWEEN 1 AND 64),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    updated_at_seconds INTEGER NOT NULL,
    updated_at_nanos INTEGER NOT NULL CHECK (updated_at_nanos BETWEEN 0 AND 999999999),
    CHECK (
        (state = 'CLAIMED' AND result_kind IS NULL AND result_id IS NULL
            AND result_location_id IS NULL AND result_schema_version IS NULL
            AND result_payload IS NULL AND result_payload_sha256 IS NULL
            AND safe_error_code IS NULL)
        OR (state = 'COMPLETED' AND result_kind IS NOT NULL
            AND safe_error_code IS NULL AND (
            (result_kind = 'ASSET' AND result_id IS NOT NULL
                AND result_location_id IS NOT NULL AND result_schema_version IS NULL
                AND result_payload IS NULL AND result_payload_sha256 IS NULL)
            OR (result_kind IN ('ASSET_REVISION', 'LOCATION') AND result_id IS NOT NULL
                AND result_location_id IS NULL AND result_schema_version IS NULL
                AND result_payload IS NULL AND result_payload_sha256 IS NULL)
            OR (result_kind NOT IN ('ASSET', 'ASSET_REVISION', 'LOCATION')
                AND result_location_id IS NULL
                AND result_schema_version IS NOT NULL AND result_payload IS NOT NULL
                AND result_payload_sha256 IS NOT NULL)
        ))
        OR (state IN ('TERMINAL_REJECTED', 'RECOVERY_REQUIRED')
            AND result_kind IS NULL AND result_id IS NULL
            AND result_location_id IS NULL AND result_schema_version IS NULL
            AND result_payload IS NULL AND result_payload_sha256 IS NULL
            AND safe_error_code IS NOT NULL)
    ),
    FOREIGN KEY (result_location_id) REFERENCES locations(location_id)
) STRICT;

DROP INDEX commands_state_idx;
DROP INDEX commands_result_location_idx;
CREATE INDEX commands_state_idx ON commands_v2(state, command_id);
CREATE INDEX commands_result_location_idx ON commands_v2(result_location_id);

INSERT INTO commands_v2 (
    command_id, operation_id, principal_kind, principal_uid,
    canonical_request_digest, store_runtime_id, state, result_kind, result_id,
    result_location_id, result_schema_version, result_payload,
    result_payload_sha256, safe_error_code, created_at_seconds, created_at_nanos,
    updated_at_seconds, updated_at_nanos
)
SELECT command_id, operation_id, principal_kind, principal_uid,
       canonical_request_digest, store_runtime_id, state, result_kind, result_id,
       result_location_id, NULL, NULL, NULL, safe_error_code,
       created_at_seconds, created_at_nanos, updated_at_seconds, updated_at_nanos
FROM commands;

DROP TABLE commands;
ALTER TABLE commands_v2 RENAME TO commands;

CREATE TABLE domain_events_v2 (
    domain_event_id BLOB PRIMARY KEY NOT NULL CHECK (length(domain_event_id) = 16),
    commit_sequence INTEGER NOT NULL UNIQUE
        CHECK (commit_sequence BETWEEN 1 AND 9223372036854775807),
    command_id BLOB NOT NULL CHECK (length(command_id) = 16),
    event_type TEXT NOT NULL
        CHECK (length(CAST(event_type AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(event_type, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (event_type NOT GLOB '*[^a-z0-9._-]*'),
    schema_version INTEGER NOT NULL CHECK (schema_version BETWEEN 1 AND 65535),
    aggregate_kind TEXT NOT NULL
        CHECK (length(CAST(aggregate_kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(aggregate_kind, 1, 1) BETWEEN 'A' AND 'Z')
        CHECK (aggregate_kind NOT GLOB '*[^A-Z0-9_]*'),
    aggregate_id BLOB NOT NULL CHECK (length(aggregate_id) IN (16, 32)),
    aggregate_revision BLOB CHECK (aggregate_revision IS NULL OR length(aggregate_revision) = 8),
    event_payload BLOB CHECK (event_payload IS NULL OR length(event_payload) BETWEEN 1 AND 2048),
    event_payload_sha256 BLOB CHECK (
        event_payload_sha256 IS NULL OR length(event_payload_sha256) = 32
    ),
    occurred_at_seconds INTEGER NOT NULL,
    occurred_at_nanos INTEGER NOT NULL CHECK (occurred_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (command_id) REFERENCES commands(command_id),
    CHECK ((event_payload IS NULL AND event_payload_sha256 IS NULL)
           OR (event_payload IS NOT NULL AND event_payload_sha256 IS NOT NULL))
) STRICT;

DROP INDEX domain_events_command_idx;
DROP INDEX domain_events_aggregate_idx;
CREATE INDEX domain_events_command_idx
    ON domain_events_v2(command_id, commit_sequence);
CREATE INDEX domain_events_aggregate_idx
    ON domain_events_v2(aggregate_kind, aggregate_id, commit_sequence);

INSERT INTO domain_events_v2 (
    domain_event_id, commit_sequence, command_id, event_type, schema_version,
    aggregate_kind, aggregate_id, aggregate_revision, event_payload,
    event_payload_sha256, occurred_at_seconds, occurred_at_nanos
)
SELECT domain_event_id, commit_sequence, command_id, event_type, schema_version,
       aggregate_kind, aggregate_id, aggregate_revision, NULL, NULL,
       occurred_at_seconds, occurred_at_nanos
FROM domain_events;

DROP TABLE domain_events;
ALTER TABLE domain_events_v2 RENAME TO domain_events;

CREATE TRIGGER domain_events_no_update
BEFORE UPDATE ON domain_events
BEGIN
    SELECT RAISE(ABORT, 'domain events are append-only');
END;

CREATE TRIGGER domain_events_no_delete
BEFORE DELETE ON domain_events
BEGIN
    SELECT RAISE(ABORT, 'domain events are append-only');
END;

ALTER TABLE assets ADD COLUMN updated_at_seconds INTEGER;
ALTER TABLE assets ADD COLUMN updated_at_nanos INTEGER
    CHECK (updated_at_nanos IS NULL OR updated_at_nanos BETWEEN 0 AND 999999999)
    CHECK ((updated_at_seconds IS NULL) = (updated_at_nanos IS NULL));

CREATE TABLE projects (
    project_id BLOB PRIMARY KEY NOT NULL CHECK (length(project_id) = 16),
    name TEXT NOT NULL
        CHECK (length(CAST(name AS BLOB)) BETWEEN 1 AND 255)
        CHECK (instr(name, char(0)) = 0),
    current_spec_revision_id BLOB NOT NULL CHECK (length(current_spec_revision_id) = 16),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    creation_commit_sequence INTEGER NOT NULL UNIQUE
        CHECK (creation_commit_sequence BETWEEN 1 AND 9223372036854775807),
    created_by_command_id BLOB NOT NULL CHECK (length(created_by_command_id) = 16),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    updated_at_seconds INTEGER NOT NULL,
    updated_at_nanos INTEGER NOT NULL CHECK (updated_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (project_id, current_spec_revision_id)
        REFERENCES project_spec_revisions(project_id, project_spec_revision_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (creation_commit_sequence) REFERENCES domain_events(commit_sequence),
    FOREIGN KEY (created_by_command_id) REFERENCES commands(command_id)
) STRICT;

CREATE TABLE project_spec_revisions (
    project_spec_revision_id BLOB PRIMARY KEY NOT NULL
        CHECK (length(project_spec_revision_id) = 16),
    project_id BLOB NOT NULL CHECK (length(project_id) = 16),
    sequence INTEGER NOT NULL CHECK (sequence BETWEEN 1 AND 4294967295),
    resolution_width INTEGER CHECK (resolution_width IS NULL OR resolution_width BETWEEN 1 AND 65535),
    resolution_height INTEGER CHECK (resolution_height IS NULL OR resolution_height BETWEEN 1 AND 65535),
    frame_rate_numerator INTEGER
        CHECK (frame_rate_numerator IS NULL OR frame_rate_numerator BETWEEN 1 AND 4294967295),
    frame_rate_denominator INTEGER
        CHECK (frame_rate_denominator IS NULL OR frame_rate_denominator BETWEEN 1 AND 4294967295),
    aspect_ratio_numerator INTEGER
        CHECK (aspect_ratio_numerator IS NULL OR aspect_ratio_numerator BETWEEN 1 AND 4294967295),
    aspect_ratio_denominator INTEGER
        CHECK (aspect_ratio_denominator IS NULL OR aspect_ratio_denominator BETWEEN 1 AND 4294967295),
    policy_schema_version INTEGER NOT NULL CHECK (policy_schema_version = 1),
    color_policy_json BLOB NOT NULL CHECK (length(color_policy_json) BETWEEN 2 AND 65536),
    audio_policy_json BLOB NOT NULL CHECK (length(audio_policy_json) BETWEEN 2 AND 65536),
    quality_policy_json BLOB NOT NULL CHECK (length(quality_policy_json) BETWEEN 2 AND 65536),
    privacy_policy_json BLOB NOT NULL CHECK (length(privacy_policy_json) BETWEEN 2 AND 65536),
    policy_digest BLOB NOT NULL CHECK (length(policy_digest) = 32),
    created_by_command_id BLOB NOT NULL CHECK (length(created_by_command_id) = 16),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    CHECK ((resolution_width IS NULL) = (resolution_height IS NULL)),
    CHECK ((frame_rate_numerator IS NULL) = (frame_rate_denominator IS NULL)),
    CHECK ((aspect_ratio_numerator IS NULL) = (aspect_ratio_denominator IS NULL)),
    FOREIGN KEY (project_id) REFERENCES projects(project_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (created_by_command_id) REFERENCES commands(command_id),
    UNIQUE (project_id, sequence),
    UNIQUE (project_id, project_spec_revision_id)
) STRICT;

CREATE TABLE subjects (
    subject_id BLOB PRIMARY KEY NOT NULL CHECK (length(subject_id) = 16),
    kind TEXT NOT NULL
        CHECK (length(CAST(kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(kind, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (kind NOT GLOB '*[^a-z0-9._-]*'),
    canonical_name TEXT NOT NULL
        CHECK (length(CAST(canonical_name AS BLOB)) BETWEEN 1 AND 255)
        CHECK (instr(canonical_name, char(0)) = 0),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    creation_commit_sequence INTEGER NOT NULL UNIQUE
        CHECK (creation_commit_sequence BETWEEN 1 AND 9223372036854775807),
    created_by_command_id BLOB NOT NULL CHECK (length(created_by_command_id) = 16),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (creation_commit_sequence) REFERENCES domain_events(commit_sequence),
    FOREIGN KEY (created_by_command_id) REFERENCES commands(command_id)
) STRICT;

CREATE TABLE work_items (
    work_item_id BLOB PRIMARY KEY NOT NULL CHECK (length(work_item_id) = 16),
    project_id BLOB NOT NULL CHECK (length(project_id) = 16),
    kind TEXT NOT NULL CHECK (kind IN ('SCENE', 'SHOT')),
    code TEXT NOT NULL
        CHECK (length(CAST(code AS BLOB)) BETWEEN 1 AND 64)
        CHECK (instr(code, char(0)) = 0),
    current_work_revision_id BLOB NOT NULL CHECK (length(current_work_revision_id) = 16),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    creation_commit_sequence INTEGER NOT NULL UNIQUE
        CHECK (creation_commit_sequence BETWEEN 1 AND 9223372036854775807),
    created_by_command_id BLOB NOT NULL CHECK (length(created_by_command_id) = 16),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    updated_at_seconds INTEGER NOT NULL,
    updated_at_nanos INTEGER NOT NULL CHECK (updated_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (project_id) REFERENCES projects(project_id),
    FOREIGN KEY (work_item_id, current_work_revision_id)
        REFERENCES work_revisions(work_item_id, work_revision_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (creation_commit_sequence) REFERENCES domain_events(commit_sequence),
    FOREIGN KEY (created_by_command_id) REFERENCES commands(command_id),
    UNIQUE (project_id, kind, code),
    UNIQUE (project_id, work_item_id)
) STRICT;

CREATE TABLE work_revisions (
    work_revision_id BLOB PRIMARY KEY NOT NULL CHECK (length(work_revision_id) = 16),
    work_item_id BLOB NOT NULL CHECK (length(work_item_id) = 16),
    sequence INTEGER NOT NULL CHECK (sequence BETWEEN 1 AND 4294967295),
    specification_schema_version INTEGER NOT NULL CHECK (specification_schema_version = 1),
    specification_json BLOB NOT NULL CHECK (length(specification_json) BETWEEN 2 AND 262144),
    specification_digest BLOB NOT NULL CHECK (length(specification_digest) = 32),
    created_by_command_id BLOB NOT NULL CHECK (length(created_by_command_id) = 16),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (work_item_id) REFERENCES work_items(work_item_id)
        DEFERRABLE INITIALLY DEFERRED,
    FOREIGN KEY (created_by_command_id) REFERENCES commands(command_id),
    UNIQUE (work_item_id, sequence),
    UNIQUE (work_item_id, work_revision_id)
) STRICT;

CREATE TABLE takes (
    take_id BLOB PRIMARY KEY NOT NULL CHECK (length(take_id) = 16),
    work_revision_id BLOB NOT NULL CHECK (length(work_revision_id) = 16),
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 1 AND 4294967295),
    state TEXT NOT NULL
        CHECK (state IN ('CANDIDATE', 'SHORTLISTED', 'SELECTED', 'APPROVED',
                         'REJECTED', 'SUPERSEDED')),
    primary_asset_id BLOB NOT NULL CHECK (length(primary_asset_id) = 16),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    creation_commit_sequence INTEGER NOT NULL UNIQUE
        CHECK (creation_commit_sequence BETWEEN 1 AND 9223372036854775807),
    created_by_command_id BLOB NOT NULL CHECK (length(created_by_command_id) = 16),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    updated_at_seconds INTEGER NOT NULL,
    updated_at_nanos INTEGER NOT NULL CHECK (updated_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (work_revision_id) REFERENCES work_revisions(work_revision_id),
    FOREIGN KEY (primary_asset_id) REFERENCES assets(asset_id),
    FOREIGN KEY (creation_commit_sequence) REFERENCES domain_events(commit_sequence),
    FOREIGN KEY (created_by_command_id) REFERENCES commands(command_id),
    UNIQUE (work_revision_id, ordinal)
) STRICT;

CREATE TABLE relationships (
    relationship_id BLOB PRIMARY KEY NOT NULL CHECK (length(relationship_id) = 16),
    relationship_kind TEXT NOT NULL
        CHECK (length(CAST(relationship_kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(relationship_kind, 1, 1) BETWEEN 'A' AND 'Z')
        CHECK (relationship_kind NOT GLOB '*[^A-Z0-9_]*'),
    source_kind TEXT NOT NULL
        CHECK (length(CAST(source_kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(source_kind, 1, 1) BETWEEN 'A' AND 'Z')
        CHECK (source_kind NOT GLOB '*[^A-Z0-9_]*'),
    source_id BLOB NOT NULL CHECK (length(source_id) IN (16, 32)),
    target_kind TEXT NOT NULL
        CHECK (length(CAST(target_kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(target_kind, 1, 1) BETWEEN 'A' AND 'Z')
        CHECK (target_kind NOT GLOB '*[^A-Z0-9_]*'),
    target_id BLOB NOT NULL CHECK (length(target_id) IN (16, 32)),
    created_by_command_id BLOB NOT NULL CHECK (length(created_by_command_id) = 16),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    CHECK (source_id <> target_id OR source_kind <> target_kind),
    FOREIGN KEY (created_by_command_id) REFERENCES commands(command_id),
    UNIQUE (relationship_kind, source_kind, source_id, target_kind, target_id)
) STRICT;

CREATE INDEX projects_creation_idx
    ON projects(creation_commit_sequence, project_id);
CREATE INDEX project_spec_revisions_project_idx
    ON project_spec_revisions(project_id, sequence);
CREATE INDEX project_spec_revisions_command_idx
    ON project_spec_revisions(created_by_command_id, project_spec_revision_id);
CREATE INDEX subjects_creation_idx
    ON subjects(creation_commit_sequence, subject_id);
CREATE INDEX work_items_project_creation_idx
    ON work_items(project_id, creation_commit_sequence, work_item_id);
CREATE INDEX work_items_current_revision_idx
    ON work_items(current_work_revision_id);
CREATE INDEX work_revisions_work_idx
    ON work_revisions(work_item_id, sequence);
CREATE INDEX work_revisions_command_idx
    ON work_revisions(created_by_command_id, work_revision_id);
CREATE INDEX takes_work_revision_ordinal_idx
    ON takes(work_revision_id, ordinal, take_id);
CREATE INDEX takes_primary_asset_idx
    ON takes(primary_asset_id, take_id);
CREATE UNIQUE INDEX takes_one_selected_idx
    ON takes(work_revision_id) WHERE state = 'SELECTED';
CREATE INDEX relationships_source_idx
    ON relationships(source_kind, source_id, relationship_kind, target_id);
CREATE INDEX relationships_target_idx
    ON relationships(target_kind, target_id, relationship_kind, source_id);
CREATE INDEX relationships_command_idx
    ON relationships(created_by_command_id, relationship_id);
CREATE UNIQUE INDEX relationships_take_reopens_source_idx
    ON relationships(source_id)
    WHERE relationship_kind = 'TAKE_REOPENS' AND source_kind = 'TAKE';
CREATE UNIQUE INDEX relationships_take_supersedes_source_idx
    ON relationships(source_id)
    WHERE relationship_kind = 'TAKE_SUPERSEDES' AND source_kind = 'TAKE';
