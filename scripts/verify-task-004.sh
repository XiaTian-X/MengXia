#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

cargo_test() {
    cargo test --locked --offline "$@"
}

run_test_sqlite_004() {
    echo "TEST-SQLITE-004: source-pinned SQLite identity/options and connection hardening"
    cargo_test -p mengxia-store-sqlite runtime::tests
    cargo_test -p mengxia-testkit --test task_004_foundation source_pinned_sqlite_patch_has_exact_bytes_and_no_fallback_tooling
}

run_test_config_004() {
    echo "TEST-CONFIG-004: pure typed configuration validation before mutation"
    cargo_test -p mengxia-store-sqlite config::tests
}

run_test_bootstrap_004() {
    echo "TEST-BOOTSTRAP-004: durable intent, filesystem state and bootstrap authority"
    cargo_test -p mengxia-store-sqlite intent::tests
    cargo_test -p mengxia-store-sqlite bootstrap::tests
    cargo_test -p mengxia-platform-fs --lib
}

run_test_path_004() {
    echo "TEST-PATH-004: whole-prefix APFS path and ACL authority"
    cargo_test -p mengxia-platform-fs --lib
}

run_test_migration_004() {
    echo "TEST-MIGRATION-004: immutable bootstrap migration and complete reopen validation"
    cargo_test -p mengxia-store-sqlite migration::tests
}

run_test_lock_004() {
    echo "TEST-LOCK-004: durable exclusive lock, contention and restart identity"
    cargo_test -p mengxia-platform-fs --lib
    cargo_test -p mengxia-store-sqlite bootstrap::tests
}

run_test_queue_004() {
    echo "TEST-QUEUE-004: bounded writer/read admission and joined shutdown"
    cargo_test -p mengxia-store-sqlite lifecycle::tests
}

run_test_error_004() {
    echo "TEST-ERROR-004: exact SQLite/platform/config/shutdown error mapping and redaction"
    cargo_test -p mengxia-store-sqlite error::tests
    cargo_test -p mengxia-store-sqlite path_authority::tests
    cargo_test -p mengxia-store-sqlite bootstrap::tests::every_clock_or_timestamp_failure_precedes_identity_and_root_mutation
    cargo_test -p mengxia-store-sqlite bootstrap::tests::every_uuid_generation_failure_precedes_root_mutation
}

run_test_recovery_004() {
    echo "TEST-RECOVERY-004: exact SIGKILL matrix and filesystem fault ordering"
    cargo_test -p mengxia-store-sqlite bootstrap::tests::exact_same_os_sigkill_recovery_matrix
    cargo_test -p mengxia-platform-fs --lib
}

run_test_wal_004() {
    echo "TEST-WAL-004: WAL/SHM recovery and multi-connection reset regression"
    cargo_test -p mengxia-store-sqlite bootstrap::tests::killed_wal_writer_recovers_commit_or_cleans_rolled_back_staging
    cargo_test -p mengxia-store-sqlite bootstrap::tests::killed_writer_required_commit_wal_damage_fails_closed
    cargo_test -p mengxia-store-sqlite --test wal_reset
}

run_test_corruption_004() {
    echo "TEST-CORRUPTION-004: complete deterministic database/WAL/metadata/filesystem matrix"
    cargo_test -p mengxia-store-sqlite migration::tests
    cargo_test -p mengxia-store-sqlite intent::tests
    cargo_test -p mengxia-store-sqlite bootstrap::tests::killed_writer_required_commit_wal_damage_fails_closed
    cargo_test -p mengxia-platform-fs --lib
}

run_test_arch_004() {
    echo "TEST-ARCH-004: package, dependency, unsafe, FFI and SQLite-open boundaries"
    cargo_test -p mengxia-testkit --test architecture
    cargo_test -p mengxia-testkit --test task_002_architecture
    cargo_test -p mengxia-testkit --test task_004_foundation
}

run_test_supply_004() {
    echo "TEST-SUPPLY-004: toolchain/source/environment policy and dependency review"
    scripts/verify-macos-acl-toolchain.sh
    cargo_test -p mengxia-testkit --test task_004_foundation
    scripts/check-supply-chain.sh
}

run_test_doc_004() {
    echo "TEST-DOC-004: stable IDs, lifecycle and downstream dependency invariants"
    cargo_test -p mengxia-testkit --test document_traceability
    cargo_test -p mengxia-testkit --test naming
}

run_one() {
    case "$1" in
        TEST-SQLITE-004) run_test_sqlite_004 ;;
        TEST-CONFIG-004) run_test_config_004 ;;
        TEST-BOOTSTRAP-004) run_test_bootstrap_004 ;;
        TEST-PATH-004) run_test_path_004 ;;
        TEST-MIGRATION-004) run_test_migration_004 ;;
        TEST-LOCK-004) run_test_lock_004 ;;
        TEST-QUEUE-004) run_test_queue_004 ;;
        TEST-ERROR-004) run_test_error_004 ;;
        TEST-RECOVERY-004) run_test_recovery_004 ;;
        TEST-WAL-004) run_test_wal_004 ;;
        TEST-CORRUPTION-004) run_test_corruption_004 ;;
        TEST-ARCH-004) run_test_arch_004 ;;
        TEST-SUPPLY-004) run_test_supply_004 ;;
        TEST-DOC-004) run_test_doc_004 ;;
        *) echo "unknown TASK-004 test ID: $1" >&2; exit 64 ;;
    esac
}

if [ "$#" -eq 0 ]; then
    for test_id in \
        TEST-SQLITE-004 \
        TEST-CONFIG-004 \
        TEST-BOOTSTRAP-004 \
        TEST-PATH-004 \
        TEST-MIGRATION-004 \
        TEST-LOCK-004 \
        TEST-QUEUE-004 \
        TEST-ERROR-004 \
        TEST-RECOVERY-004 \
        TEST-WAL-004 \
        TEST-CORRUPTION-004 \
        TEST-ARCH-004 \
        TEST-SUPPLY-004 \
        TEST-DOC-004
    do
        run_one "$test_id"
    done
else
    for test_id in "$@"; do
        run_one "$test_id"
    done
fi

echo "TASK-004 retained baseline: format, check, lint and complete workspace tests"
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features --locked --offline
/usr/bin/env -u MENGXIA_ACL_BUILD_CLASS \
    cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --all-targets --all-features --locked --offline
git diff --check
