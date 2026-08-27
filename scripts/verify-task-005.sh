#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
case "$mode" in
    developer|formal) ;;
    *) echo "usage: scripts/verify-task-005.sh developer|formal" >&2; exit 64 ;;
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

cas_test() {
    cargo test --locked --offline -p mengxia-storage-local --test task_005_local_cas "$1"
}

location_tests() {
    cargo test --locked --offline -p mengxia-ports verified_local_result_builds_exact_bounded_opaque_location
    cas_test backend_identity_tracks_root_inode_not_configured_path_text
}

run TEST-CONFIG-005 cargo test --locked --offline -p mengxia-storage-local config::tests
run TEST-NAMESPACE-005 cas_test task_004_reopen_rejects_an_unsafe_storage_directory_without_mutation
run TEST-PATH-005 cas_test source_symlinks_and_invalid_orphan_names_fail_closed
run TEST-SOURCE-005 cas_test source_mutation_and_expected_digest_mismatch_fail_without_publish
run TEST-STREAM-005 cas_test streams_hashes_promotes_and_deduplicates_without_exposing_a_root
run TEST-CONTROL-005 cas_test cooperative_stop_and_control_panic_cleanup_without_poisoning_runtime
run TEST-RESOURCE-005 cas_test atomic_admission_returns_backpressure_without_a_second_staging_file
run TEST-PROMOTE-005 cas_test hostile_existing_canonical_is_preserved_with_staging_evidence
run TEST-LOCATION-005 location_tests
run TEST-RECOVERY-005 cas_test restart_reports_valid_staging_orphan_without_deleting_it
run TEST-ORPHAN-005 cas_test restart_reports_valid_staging_orphan_without_deleting_it
run TEST-CONCURRENCY-005 cas_test atomic_admission_returns_backpressure_without_a_second_staging_file
run TEST-ERROR-005 cargo test --locked --offline -p mengxia-ports blob_error_codes_retry_classes_and_static_messages_are_exact
run TEST-LIFECYCLE-005 cas_test blob_authority_keeps_library_lock_after_sqlite_shutdown
run TEST-ARCH-005 cargo test --locked --offline -p mengxia-testkit --test task_005_foundation
run TEST-SUPPLY-005 cargo test --locked --offline -p mengxia-testkit --test task_005_foundation task_005_dependency_and_architecture_boundaries_are_exact
run TEST-DOC-005 cargo test --locked --offline -p mengxia-testkit --test document_traceability

cargo fmt --all -- --check
cargo check --locked --offline -p mengxia-platform-fs -p mengxia-ports -p mengxia-storage-local -p mengxia-store-sqlite -p mengxia-testkit --all-targets --all-features
cargo clippy --locked --offline -p mengxia-platform-fs -p mengxia-ports -p mengxia-storage-local -p mengxia-store-sqlite -p mengxia-testkit --all-targets --all-features -- -D warnings
cargo test --locked --offline -p mengxia-platform-fs -p mengxia-ports -p mengxia-storage-local -p mengxia-store-sqlite -p mengxia-testkit --all-targets --all-features
cargo test --locked --offline -p mengxia-testkit --test naming
git diff --check

if [ "$mode" = formal ]; then
    if ! grep -q '^// TASK005_FORMAL_MATRIX_COMPLETE: YES$' crates/mengxia-storage-local/tests/task_005_recovery.rs; then
        echo "TASK-005 formal KILL/FAULT matrix is not complete yet" >&2
        exit 1
    fi
    cargo test --locked --offline -p mengxia-platform-fs blob_storage::tests::task_005_blob_root_sigkill_matrix_has_exact_same_os_restart_states -- --exact
    cargo test --locked --offline -p mengxia-platform-fs blob_storage::tests::task_005_blob_file_sigkill_matrix_has_exact_same_os_restart_states -- --exact
    cargo test --locked --offline -p mengxia-platform-fs blob_storage::tests::task_005_before_success_reply_sigkill_is_durable_and_retry_deduplicates -- --exact
    cargo test --locked --offline -p mengxia-platform-fs blob_storage::tests::task_005_blob_root_initialization_fault_matrix_is_ordered_and_recoverable -- --exact
    cargo test --locked --offline -p mengxia-platform-fs blob_storage::tests::task_005_publish_fault_matrix_preserves_exact_no_clobber_prefixes -- --exact
    cargo test --locked --offline -p mengxia-platform-fs blob_storage::tests::task_005_dedup_and_cleanup_fault_matrix_never_removes_foreign_canonical_data -- --exact
    cargo test --locked --offline -p mengxia-storage-local --test task_005_recovery
    cargo test --release --locked --offline -p mengxia-storage-local --lib -- --ignored task_005_generated_scaling_evidence
    scripts/check-supply-chain.sh
    scripts/verify-task-003.sh
    cargo test --workspace --all-targets --all-features --locked --offline
fi
