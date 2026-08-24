#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

run_test_type_001() {
    echo "TEST-TYPE-001: UUIDv7 generation, typed markers and value round trips"
    cargo test -p mengxia-types --lib --locked
    cargo test -p mengxia-types --test value_types --locked
    cargo test -p mengxia-testkit --test task_002_architecture marker_mismatch_fixture_fails_to_compile --locked
}

run_test_parse_001() {
    echo "TEST-PARSE-001: malformed, noncanonical, non-ASCII and boundary input rejection"
    cargo test -p mengxia-types --test value_types rejects --locked
    cargo test -p mengxia-types --test value_types enforces --locked
}

run_test_time_001() {
    echo "TEST-TIME-001: timestamp and revision range/canonical/exhaustion behavior"
    cargo test -p mengxia-types --test value_types timestamp --locked
    cargo test -p mengxia-types --test value_types revision --locked
}

run_test_error_001() {
    echo "TEST-ERROR-001: exact error taxonomy, typed mapping and safe diagnostics"
    cargo test -p mengxia-types --test value_types error --locked
    cargo test -p mengxia-domain --test error_baseline --locked
}

run_test_arch_002() {
    echo "TEST-ARCH-002: exact dependency/public surface and marker compile-fail"
    cargo test -p mengxia-testkit --test task_002_architecture --locked
}

run_test_supply_002() {
    echo "TEST-SUPPLY-002: exact feature/lock/license/advisory graph and dev-only duplicate"
    grep -F 'getrandom = { version = "=0.4.3", default-features = false, features = ["std"] }' Cargo.toml
    grep -F 'proptest = { version = "=1.11.0", default-features = false, features = ["std"] }' Cargo.toml
    grep -F 'time = { version = "=0.3.55", default-features = false, features = ["std", "formatting", "parsing"] }' Cargo.toml
    grep -F 'uuid = { version = "=1.24.1", default-features = false, features = ["std"] }' Cargo.toml

    production_tree=$(cargo tree -p mengxia-types --edges normal,build --locked)
    printf '%s\n' "$production_tree" | grep -F 'getrandom v0.4.3'
    printf '%s\n' "$production_tree" | grep -F 'time v0.3.55'
    printf '%s\n' "$production_tree" | grep -F 'uuid v1.24.1'
    if printf '%s\n' "$production_tree" | grep -E 'getrandom v0\.3\.4|proptest|serde|prost|rusqlite|tokio|reqwest'; then
        echo "production dependency graph contains a forbidden TASK-002 dependency" >&2
        exit 1
    fi

    test_tree=$(cargo tree -p mengxia-types --edges normal,build,dev --locked)
    printf '%s\n' "$test_tree" | grep -F 'proptest v1.11.0'
    printf '%s\n' "$test_tree" | grep -F 'getrandom v0.3.4'
    duplicate_roots=$(cargo tree -p mengxia-types --duplicates \
        --edges normal,build,dev --locked \
        | sed -n '/^[[:alnum:]_-][[:alnum:]_.-]* v/p')
    test "$duplicate_roots" = "getrandom v0.3.4
getrandom v0.4.3"

    set +e
    unavailable_output=$(scripts/check-supply-chain.sh --simulate-advisory-unavailable 2>&1)
    unavailable_status=$?
    set -e
    test "$unavailable_status" -eq 2
    printf '%s\n' "$unavailable_output" | grep '^UNVERIFIABLE:'
    scripts/check-supply-chain.sh
}

run_test_doc_002() {
    echo "TEST-DOC-002: TASK-002 stable registry and synchronized lifecycle state"
    cargo test -p mengxia-testkit --test document_traceability --locked
}

run_one() {
    case "$1" in
        TEST-TYPE-001) run_test_type_001 ;;
        TEST-PARSE-001) run_test_parse_001 ;;
        TEST-TIME-001) run_test_time_001 ;;
        TEST-ERROR-001) run_test_error_001 ;;
        TEST-ARCH-002) run_test_arch_002 ;;
        TEST-SUPPLY-002) run_test_supply_002 ;;
        TEST-DOC-002) run_test_doc_002 ;;
        *) echo "unknown TASK-002 test ID: $1" >&2; exit 64 ;;
    esac
}

if [ "$#" -eq 0 ]; then
    for test_id in \
        TEST-TYPE-001 \
        TEST-PARSE-001 \
        TEST-TIME-001 \
        TEST-ERROR-001 \
        TEST-ARCH-002 \
        TEST-SUPPLY-002 \
        TEST-DOC-002
    do
        run_one "$test_id"
    done
else
    for test_id in "$@"; do
        run_one "$test_id"
    done
fi
