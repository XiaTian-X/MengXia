CREATE TABLE event_commit_sequence (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    last_sequence INTEGER NOT NULL CHECK (last_sequence BETWEEN 0 AND 9223372036854775807)
) STRICT;

INSERT INTO event_commit_sequence (singleton, last_sequence) VALUES (1, 0);

CREATE TABLE commands (
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
        CHECK (result_kind IS NULL OR result_kind IN ('ASSET', 'ASSET_REVISION', 'LOCATION')),
    result_id BLOB CHECK (result_id IS NULL OR length(result_id) = 16),
    result_location_id BLOB
        CHECK (result_location_id IS NULL OR length(result_location_id) = 16),
    safe_error_code TEXT
        CHECK (safe_error_code IS NULL OR length(CAST(safe_error_code AS BLOB)) BETWEEN 1 AND 64),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    updated_at_seconds INTEGER NOT NULL,
    updated_at_nanos INTEGER NOT NULL CHECK (updated_at_nanos BETWEEN 0 AND 999999999),
    CHECK (
        (state = 'CLAIMED' AND result_kind IS NULL AND result_id IS NULL
            AND result_location_id IS NULL AND safe_error_code IS NULL)
        OR (state = 'COMPLETED' AND result_kind IS NOT NULL AND result_id IS NOT NULL
            AND safe_error_code IS NULL
            AND ((result_kind = 'ASSET' AND result_location_id IS NOT NULL)
                 OR (result_kind IN ('ASSET_REVISION', 'LOCATION')
                     AND result_location_id IS NULL)))
        OR (state IN ('TERMINAL_REJECTED', 'RECOVERY_REQUIRED')
            AND result_kind IS NULL AND result_id IS NULL
            AND result_location_id IS NULL AND safe_error_code IS NOT NULL)
    ),
    FOREIGN KEY (result_location_id) REFERENCES locations(location_id)
) STRICT;

CREATE TABLE assets (
    asset_id BLOB PRIMARY KEY NOT NULL CHECK (length(asset_id) = 16),
    kind TEXT NOT NULL
        CHECK (length(CAST(kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(kind, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (kind NOT GLOB '*[^a-z0-9._-]*'),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('ACTIVE', 'RETIRED')),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    created_by_uid INTEGER NOT NULL CHECK (created_by_uid BETWEEN 0 AND 4294967295)
) STRICT;

CREATE TABLE asset_revisions (
    asset_revision_id BLOB PRIMARY KEY NOT NULL CHECK (length(asset_revision_id) = 16),
    asset_id BLOB NOT NULL CHECK (length(asset_id) = 16),
    sequence INTEGER NOT NULL CHECK (sequence BETWEEN 1 AND 4294967295),
    content_kind TEXT NOT NULL
        CHECK (length(CAST(content_kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(content_kind, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (content_kind NOT GLOB '*[^a-z0-9._-]*'),
    custody TEXT NOT NULL CHECK (custody IN ('MANAGED', 'UNMANAGED')),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL CHECK (created_at_nanos BETWEEN 0 AND 999999999),
    created_by_uid INTEGER NOT NULL CHECK (created_by_uid BETWEEN 0 AND 4294967295),
    FOREIGN KEY (asset_id) REFERENCES assets(asset_id),
    UNIQUE (asset_id, sequence),
    UNIQUE (asset_id, asset_revision_id)
) STRICT;

CREATE TABLE asset_revision_parents (
    asset_id BLOB NOT NULL CHECK (length(asset_id) = 16),
    child_revision_id BLOB NOT NULL CHECK (length(child_revision_id) = 16),
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 63),
    parent_revision_id BLOB NOT NULL CHECK (length(parent_revision_id) = 16),
    PRIMARY KEY (child_revision_id, ordinal),
    UNIQUE (child_revision_id, parent_revision_id),
    CHECK (child_revision_id <> parent_revision_id),
    FOREIGN KEY (asset_id, child_revision_id)
        REFERENCES asset_revisions(asset_id, asset_revision_id),
    FOREIGN KEY (asset_id, parent_revision_id)
        REFERENCES asset_revisions(asset_id, asset_revision_id)
) STRICT;

CREATE TABLE representations (
    representation_id BLOB PRIMARY KEY NOT NULL CHECK (length(representation_id) = 16),
    asset_revision_id BLOB NOT NULL CHECK (length(asset_revision_id) = 16),
    purpose TEXT NOT NULL
        CHECK (length(CAST(purpose AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(purpose, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (purpose NOT GLOB '*[^a-z0-9._-]*'),
    FOREIGN KEY (asset_revision_id) REFERENCES asset_revisions(asset_revision_id)
) STRICT;

CREATE TABLE resources (
    resource_id BLOB PRIMARY KEY NOT NULL CHECK (length(resource_id) = 16),
    representation_id BLOB NOT NULL CHECK (length(representation_id) = 16),
    kind TEXT NOT NULL
        CHECK (length(CAST(kind AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(kind, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (kind NOT GLOB '*[^a-z0-9._-]*'),
    FOREIGN KEY (representation_id) REFERENCES representations(representation_id)
) STRICT;

CREATE TABLE blobs (
    digest BLOB PRIMARY KEY NOT NULL CHECK (length(digest) = 32),
    byte_length INTEGER NOT NULL CHECK (byte_length BETWEEN 0 AND 1099511627776),
    media_type TEXT
        CHECK (media_type IS NULL OR (
            length(CAST(media_type AS BLOB)) BETWEEN 3 AND 255
            AND instr(media_type, '/') BETWEEN 2 AND length(media_type) - 1
            AND instr(substr(media_type, instr(media_type, '/') + 1), '/') = 0
            AND media_type NOT GLOB '*[^a-z0-9!#$&^_.+/-]*'
        )),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('AVAILABLE', 'GC_PENDING', 'PURGED')),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    verified_at_seconds INTEGER NOT NULL,
    verified_at_nanos INTEGER NOT NULL CHECK (verified_at_nanos BETWEEN 0 AND 999999999)
) STRICT;

CREATE TABLE resource_members (
    resource_id BLOB NOT NULL CHECK (length(resource_id) = 16),
    ordinal INTEGER NOT NULL CHECK (ordinal BETWEEN 0 AND 4095),
    logical_name TEXT NOT NULL
        CHECK (length(CAST(logical_name AS BLOB)) BETWEEN 0 AND 255)
        CHECK (instr(logical_name, char(0)) = 0),
    blob_digest BLOB NOT NULL CHECK (length(blob_digest) = 32),
    PRIMARY KEY (resource_id, ordinal),
    FOREIGN KEY (resource_id) REFERENCES resources(resource_id),
    FOREIGN KEY (blob_digest) REFERENCES blobs(digest)
) STRICT;

CREATE TABLE locations (
    location_id BLOB PRIMARY KEY NOT NULL CHECK (length(location_id) = 16),
    blob_digest BLOB NOT NULL CHECK (length(blob_digest) = 32),
    backend_id TEXT NOT NULL
        CHECK (length(CAST(backend_id AS BLOB)) BETWEEN 1 AND 255)
        CHECK (instr(backend_id, char(0)) = 0),
    locator TEXT NOT NULL
        CHECK (length(CAST(locator AS BLOB)) BETWEEN 1 AND 1024)
        CHECK (instr(locator, char(0)) = 0),
    custody TEXT NOT NULL CHECK (custody IN ('MANAGED', 'UNMANAGED')),
    durability TEXT NOT NULL CHECK (durability IN ('DURABLE', 'UNKNOWN')),
    lifecycle TEXT NOT NULL CHECK (lifecycle IN ('AVAILABLE', 'CORRUPT', 'MISSING', 'REMOVED')),
    revision BLOB NOT NULL CHECK (length(revision) = 8),
    verified_at_seconds INTEGER NOT NULL,
    verified_at_nanos INTEGER NOT NULL CHECK (verified_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (blob_digest) REFERENCES blobs(digest),
    UNIQUE (backend_id, locator)
) STRICT;

CREATE TABLE provenance_events (
    provenance_event_id BLOB PRIMARY KEY NOT NULL CHECK (length(provenance_event_id) = 16),
    command_id BLOB NOT NULL CHECK (length(command_id) = 16),
    event_type TEXT NOT NULL
        CHECK (length(CAST(event_type AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(event_type, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (event_type NOT GLOB '*[^a-z0-9._-]*'),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    asset_revision_id BLOB NOT NULL CHECK (length(asset_revision_id) = 16),
    blob_digest BLOB CHECK (blob_digest IS NULL OR length(blob_digest) = 32),
    verification TEXT NOT NULL
        CHECK (verification IN ('UNKNOWN', 'VERIFIED', 'CONFLICTED', 'REJECTED')),
    occurred_at_seconds INTEGER NOT NULL,
    occurred_at_nanos INTEGER NOT NULL CHECK (occurred_at_nanos BETWEEN 0 AND 999999999),
    recorded_at_seconds INTEGER NOT NULL,
    recorded_at_nanos INTEGER NOT NULL CHECK (recorded_at_nanos BETWEEN 0 AND 999999999),
    correction_of BLOB CHECK (correction_of IS NULL OR length(correction_of) = 16),
    CHECK (correction_of IS NULL OR correction_of <> provenance_event_id),
    FOREIGN KEY (command_id) REFERENCES commands(command_id),
    FOREIGN KEY (asset_revision_id) REFERENCES asset_revisions(asset_revision_id),
    FOREIGN KEY (blob_digest) REFERENCES blobs(digest),
    FOREIGN KEY (correction_of) REFERENCES provenance_events(provenance_event_id)
) STRICT;

CREATE TABLE domain_events (
    domain_event_id BLOB PRIMARY KEY NOT NULL CHECK (length(domain_event_id) = 16),
    commit_sequence INTEGER NOT NULL UNIQUE
        CHECK (commit_sequence BETWEEN 1 AND 9223372036854775807),
    command_id BLOB NOT NULL CHECK (length(command_id) = 16),
    event_type TEXT NOT NULL
        CHECK (length(CAST(event_type AS BLOB)) BETWEEN 1 AND 64)
        CHECK (substr(event_type, 1, 1) BETWEEN 'a' AND 'z')
        CHECK (event_type NOT GLOB '*[^a-z0-9._-]*'),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    aggregate_kind TEXT NOT NULL
        CHECK (aggregate_kind IN ('ASSET', 'ASSET_REVISION', 'BLOB', 'LOCATION')),
    aggregate_id BLOB NOT NULL CHECK (length(aggregate_id) IN (16, 32)),
    aggregate_revision BLOB CHECK (aggregate_revision IS NULL OR length(aggregate_revision) = 8),
    occurred_at_seconds INTEGER NOT NULL,
    occurred_at_nanos INTEGER NOT NULL CHECK (occurred_at_nanos BETWEEN 0 AND 999999999),
    FOREIGN KEY (command_id) REFERENCES commands(command_id),
    CHECK ((aggregate_kind = 'BLOB' AND length(aggregate_id) = 32)
           OR (aggregate_kind <> 'BLOB' AND length(aggregate_id) = 16))
) STRICT;

CREATE INDEX commands_state_idx
    ON commands(state, command_id);
CREATE INDEX commands_result_location_idx
    ON commands(result_location_id);
CREATE INDEX asset_revision_parents_child_asset_idx
    ON asset_revision_parents(asset_id, child_revision_id, ordinal);
CREATE INDEX asset_revision_parents_parent_asset_idx
    ON asset_revision_parents(asset_id, parent_revision_id, child_revision_id);
CREATE INDEX representations_revision_idx
    ON representations(asset_revision_id, representation_id);
CREATE INDEX resources_representation_idx
    ON resources(representation_id, resource_id);
CREATE INDEX resource_members_blob_idx
    ON resource_members(blob_digest, resource_id, ordinal);
CREATE INDEX locations_blob_backend_idx
    ON locations(blob_digest, backend_id, location_id);
CREATE INDEX locations_lifecycle_idx
    ON locations(lifecycle, location_id);
CREATE INDEX provenance_events_command_idx
    ON provenance_events(command_id, provenance_event_id);
CREATE INDEX provenance_events_revision_idx
    ON provenance_events(asset_revision_id, provenance_event_id);
CREATE INDEX provenance_events_blob_idx
    ON provenance_events(blob_digest, provenance_event_id);
CREATE INDEX provenance_events_correction_idx
    ON provenance_events(correction_of, provenance_event_id);
CREATE INDEX domain_events_command_idx
    ON domain_events(command_id, commit_sequence);
CREATE INDEX domain_events_aggregate_idx
    ON domain_events(aggregate_kind, aggregate_id, commit_sequence);

CREATE TRIGGER provenance_events_no_update
BEFORE UPDATE ON provenance_events
BEGIN
    SELECT RAISE(ABORT, 'provenance events are append-only');
END;

CREATE TRIGGER provenance_events_no_delete
BEFORE DELETE ON provenance_events
BEGIN
    SELECT RAISE(ABORT, 'provenance events are append-only');
END;

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
