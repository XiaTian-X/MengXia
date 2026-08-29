#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
case "$mode" in
    developer|formal) ;;
    *) echo "usage: scripts/verify-task-006.sh developer|formal" >&2; exit 64 ;;
esac

run() {
    test_id=$1
    shift
    "$@"
    if [ "$mode" = developer ]; then
        echo "$test_id: FAST_PASS"
    else
        echo "$test_id: PASS"
    fi
}

domain_tests() {
    cargo test --locked --offline -p mengxia-domain --all-targets
    cargo test --locked --offline --doc -p mengxia-domain -p mengxia-events -p mengxia-ports
}

asset_test() {
    cargo test --locked --offline -p mengxia-store-sqlite --test task_006_assets "$1"
}

mapper_tests() {
    asset_test location_and_creative_revision_use_expected_revision_and_atomic_events
    asset_test materialized_row_corruption_is_not_downgraded_to_not_found_or_conflict
    asset_test command_record_typed_mapping_and_operation_matrix_fail_closed
    asset_test registration_replay_rejects_non_exact_materialized_graph
    cargo test --locked --offline -p mengxia-store-sqlite \
        asset_repository::tests::command_row_mapper_rejects_malformed_typed_fields_and_matrices
}

event_tests() {
    asset_test location_and_creative_revision_use_expected_revision_and_atomic_events
    asset_test event_sequence_exhaustion_commits_only_replayable_terminal_rejection
}

concurrency_tests() {
    asset_test concurrent_duplicate_claim_has_one_durable_owner
    asset_test concurrent_pure_duplicate_mutates_once_and_stale_revision_rejects_replayably
}

migration_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite \
        migration::tests::asset_migration_candidate_has_exact_identity_and_parses
    cargo test --locked --offline -p mengxia-store-sqlite \
        migration::tests::asset_migration_fault_boundaries_rollback_to_exact_0000
}

recovery_tests() {
    asset_test stale_claim_requires_recovery_and_terminal_disposition_replays_after_restart
    asset_test pure_statement_failure_rolls_back_command_state_and_events_and_fails_runtime
    cargo test --locked --offline -p mengxia-store-sqlite \
        asset_repository::tests::pure_transaction_fault_boundaries_rollback_every_statement_group
    cargo test --locked --offline -p mengxia-store-sqlite \
        asset_repository::tests::external_and_location_statement_fault_boundaries_rollback_every_group
    if [ "$mode" = formal ]; then
        cargo test --locked --offline -p mengxia-store-sqlite \
            migration::tests::asset_migration_sigkill_before_and_after_commit_recovers_exactly
        cargo test --locked --offline -p mengxia-store-sqlite \
            asset_repository::tests::pure_transaction_sigkill_before_and_after_commit_is_atomic_and_replayable
    fi
}

run TEST-DOMAIN-006 domain_tests
run TEST-MAPPER-006 mapper_tests
run TEST-MIGRATION-006 migration_tests
run TEST-SCHEMA-006 asset_test current_schema_tamper_and_newer_prefixes_fail_closed_by_class
command_tests() {
    asset_test external_registration_is_atomic_shared_and_exactly_replayable
    asset_test stale_claim_requires_recovery_and_terminal_disposition_replays_after_restart
    asset_test command_record_typed_mapping_and_operation_matrix_fail_closed
    asset_test external_terminal_code_outside_operation_allowlist_is_corruption
}

run TEST-COMMAND-006 command_tests
run TEST-CONCURRENCY-006 concurrency_tests
run TEST-EVENT-006 event_tests
custody_tests() {
    asset_test external_registration_is_atomic_shared_and_exactly_replayable
    asset_test different_command_reusing_location_descriptor_is_terminal_conflict_without_domain_change
}

run TEST-CUSTODY-006 custody_tests
run TEST-ERROR-006 cargo test --locked --offline -p mengxia-types -p mengxia-ports error
run TEST-RECOVERY-006 recovery_tests
run TEST-LIFECYCLE-006 cargo test --locked --offline -p mengxia-store-sqlite lifecycle::tests
run TEST-ARCH-006 cargo test --locked --offline -p mengxia-testkit --test task_006_foundation task_006_dependency_and_architecture_boundaries_are_exact
run TEST-SUPPLY-006 cargo test --locked --offline -p mengxia-testkit --test task_006_foundation task_006_migration_bytes_and_sqlite_ownership_are_frozen
run TEST-DOC-006 cargo test --locked --offline -p mengxia-testkit --test document_traceability

cargo fmt --all -- --check
cargo check --locked --offline --workspace --all-targets --all-features
/usr/bin/env -u MENGXIA_ACL_BUILD_CLASS \
    cargo clippy --locked --offline --workspace --all-targets --all-features -- -D warnings
cargo test --locked --offline --workspace --all-targets --all-features
cargo test --locked --offline --workspace --doc
cargo test --locked --offline -p mengxia-testkit --test naming
git diff --check

if [ "$mode" = formal ]; then
    scripts/check-supply-chain.sh
    scripts/verify-task-005.sh formal
else
    scripts/verify-task-005.sh developer
fi
