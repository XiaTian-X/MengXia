CREATE TABLE schema_migrations (
    migration_sequence INTEGER PRIMARY KEY NOT NULL
        CHECK (migration_sequence BETWEEN 0 AND 9999),
    migration_name TEXT NOT NULL UNIQUE,
    sha256 BLOB NOT NULL CHECK (length(sha256) = 32),
    applied_at_seconds INTEGER NOT NULL,
    applied_at_nanos INTEGER NOT NULL
        CHECK (applied_at_nanos BETWEEN 0 AND 999999999)
) STRICT;

CREATE TABLE library_meta (
    singleton INTEGER PRIMARY KEY NOT NULL CHECK (singleton = 1),
    library_id BLOB NOT NULL UNIQUE CHECK (length(library_id) = 16),
    owner_uid INTEGER NOT NULL CHECK (owner_uid BETWEEN 0 AND 4294967295),
    created_at_seconds INTEGER NOT NULL,
    created_at_nanos INTEGER NOT NULL
        CHECK (created_at_nanos BETWEEN 0 AND 999999999)
) STRICT;
