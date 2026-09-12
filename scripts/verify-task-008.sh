#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
case "$mode" in
    developer|formal) ;;
    *) echo "usage: scripts/verify-task-008.sh developer|formal [component]" >&2; exit 64 ;;
esac
native=0
component=0
case "${2-}" in
    "") ;;
    component) component=1 ;;
    native-component) component=1; native=1 ;;
    *) echo "usage: scripts/verify-task-008.sh developer|formal [component]" >&2; exit 64 ;;
esac
test "$#" -le 2

. scripts/ci-evidence.sh

run() {
    test_id=$1
    shift
    "$@"
    ci_result "$test_id"
}

protocol_tests() {
    cargo test --locked --offline -p mengxia-core-proto
    cargo test --locked --offline -p mengxia-testkit --test task_008_foundation protocol
}

query_tests() {
    cargo test --locked --offline -p mengxia-app asset_query::tests
    cargo test --locked --offline -p mengxia-store-sqlite --test task_008_queries
}

verification_tests() {
    cargo test --locked --offline -p mengxia-app verification::tests
    cargo test --locked --offline -p mengxia-storage-local --test task_008_read_materialize \
        normal_and_deep_verification_classify_registered_blob_without_mutation
}

materialize_tests() {
    cargo test --locked --offline -p mengxia-app materialize::tests
    cargo test --locked --offline -p mengxia-storage-local --test task_008_read_materialize
    cargo test --locked --offline -p mengxia-store-sqlite --test task_008_queries materialization
}

recovery_tests() {
    cargo test --locked --offline -p mengxia-app \
        materialize::tests::recovery_is_physically_classified_before_cas_reacquire_and_resume -- --exact
    cargo test --locked --offline -p mengxia-storage-local --test task_008_read_materialize \
        exact_recovery_classifies_published_prefix_before_resuming_and_cleanup
    cargo test --locked --offline -p mengxia-store-sqlite --test task_008_queries \
        materialization_reacquire_is_separate_cas_and_clears_recovery_code
}

cancel_tests() {
    cargo test --locked --offline -p mengxia-app materialize::tests::cancellation
    cargo test --locked --offline -p mengxia-app verification::tests::cancelled
    cargo test --locked --offline -p mengxia-store-sqlite lifecycle::tests::interrupted_read
    cargo test --locked --offline -p mengxiad --bin mengxiad sqlite_query_deadline
}

config_tests() {
    cargo test --locked --offline -p mengxia-app config::tests
    cargo test --locked --offline -p mengxia --bin mengxia typed_layers
    cargo test --locked --offline -p mengxiad --bin mengxiad typed_layers
}

observability_tests() {
    cargo test --locked --offline -p mengxia-app observability::tests
}

architecture_tests() {
    cargo test --locked --offline -p mengxia-testkit --test architecture
    cargo test --locked --offline -p mengxia-testkit --test task_008_foundation architecture
}

health_tests() {
    cargo test --locked --offline -p mengxia-app \
        observability::tests::health_constructor_enforces_priority_readiness_and_capabilities -- --exact
    cargo test --locked --offline -p mengxiad --bin mengxiad \
        tests::startup_health_is_status_only_until_classification_publishes_ready -- --exact
}

lifecycle_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite lifecycle::tests
    cargo test --locked --offline -p mengxia-app verification::tests::active_limit
    cargo test --locked --offline -p mengxiad --bin mengxiad leaked_owner
}

e2e_test() {
    cargo build --locked --offline -p mengxia -p mengxiad
    fixture=$(mktemp -d "$repository_root/target/task008-e2e.XXXXXX")
    chmod 700 "$fixture"
    mkdir -m 700 "$fixture/runtime" "$fixture/output"
    dd if=/dev/zero of="$fixture/source.bin" bs=4096 count=1 status=none
    chmod 600 "$fixture/source.bin"
    target/debug/mengxiad serve \
        --library-root "$fixture/Library" \
        --client-endpoint "$fixture/runtime/client.sock" \
        >"$fixture/daemon.out" 2>"$fixture/daemon.err" &
    daemon_pid=$!
    cleanup() {
        kill -INT "$daemon_pid" 2>/dev/null || true
        wait "$daemon_pid" 2>/dev/null || true
        /bin/rm -rf -- "$fixture"
    }
    trap cleanup EXIT HUP INT TERM

    invalid_cursor=$(printf '%0160d' 0)
    set +e
    target/debug/mengxia asset list --cursor "$invalid_cursor" \
        --client-endpoint "$fixture/runtime/not-running.sock" \
        >"$fixture/invalid.out" 2>"$fixture/invalid.err"
    invalid_status=$?
    set -e
    test "$invalid_status" -eq 2
    grep '^MENGXIA_ERROR code=VALIDATION_ERROR retry=NONE$' \
        "$fixture/invalid.err" >/dev/null

    attempts=0
    while :; do
        if [ -S "$fixture/runtime/client.sock" ] && \
            target/debug/mengxia library status \
                --client-endpoint "$fixture/runtime/client.sock" \
                >"$fixture/status.out" 2>"$fixture/status.err" && \
            grep ' readiness=ready ' "$fixture/status.out" >/dev/null; then
            break
        fi
        attempts=$((attempts + 1))
        if [ "$attempts" -ge 500 ]; then
            return 1
        fi
        sleep 0.01
    done

    ingest_command=018d442f-c000-7a11-8022-334455667788
    target/debug/mengxia asset ingest-copy "$fixture/source.bin" \
        --command-id "$ingest_command" --asset-kind file --content-kind binary \
        --representation-purpose original --resource-kind blob \
        --logical-name source.bin --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/ingest.out"

    field() {
        key=$1
        file=$2
        awk -v prefix="$key=" '{ for (i = 1; i <= NF; i++) if (index($i, prefix) == 1) { sub(prefix, "", $i); print $i; exit } }' "$file"
    }
    asset_id=$(field asset_id "$fixture/ingest.out")
    test -n "$asset_id"

    target/debug/mengxia asset list --page-size 1 \
        --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/list.out"
    grep " asset_id=$asset_id " "$fixture/list.out" >/dev/null

    target/debug/mengxia asset inspect --asset-id "$asset_id" --page-size 1 \
        --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/inspect.out"
    asset_revision_id=$(field asset_revision_id "$fixture/inspect.out")
    representation_id=$(field representation_id "$fixture/inspect.out")
    resource_id=$(field resource_id "$fixture/inspect.out")
    member_ordinal=$(field member_ordinal "$fixture/inspect.out")
    blob_sha256=$(field blob_sha256 "$fixture/inspect.out")
    test -n "$asset_revision_id"
    test -n "$representation_id"
    test -n "$resource_id"
    test -n "$member_ordinal"
    test -n "$blob_sha256"

    materialize_command=018d442f-c000-7a11-8022-334455667799
    materialize() {
        target/debug/mengxia asset materialize \
            --command-id "$materialize_command" --asset-id "$asset_id" \
            --asset-revision-id "$asset_revision_id" \
            --representation-id "$representation_id" --resource-id "$resource_id" \
            --member-ordinal "$member_ordinal" --destination "$fixture/output/result.bin" \
            --operation-timeout-ms 10000 \
            --client-endpoint "$fixture/runtime/client.sock"
    }
    materialize >"$fixture/materialize.out"
    cmp "$fixture/source.bin" "$fixture/output/result.bin"
    grep ' replayed=false cleanup_pending=false$' "$fixture/materialize.out" >/dev/null
    grep " blob_sha256=$blob_sha256 " "$fixture/materialize.out" >/dev/null
    materialize >"$fixture/replay.out"
    grep ' replayed=true cleanup_pending=false$' "$fixture/replay.out" >/dev/null

    target/debug/mengxia library verify --mode normal --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/verify-normal.out"
    target/debug/mengxia library verify --mode deep --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/verify-deep.out"
    verification_id=$(field verification_id "$fixture/verify-deep.out")
    test -n "$verification_id"
    target/debug/mengxia library issues --verification-id "$verification_id" --page-size 1 \
        --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/issues.out"
    grep " verification_id=$verification_id " "$fixture/issues.out" >/dev/null
    target/debug/mengxia library status \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/status-final.out"
    grep ' custody_observation=deep_verified ' "$fixture/status-final.out" >/dev/null

    if grep -F "$fixture" "$fixture"/*.out "$fixture"/*.err >/dev/null || \
        grep -F 'source.bin' "$fixture"/*.out "$fixture"/*.err >/dev/null; then
        return 1
    fi
    kill -INT "$daemon_pid"
    wait "$daemon_pid"
    /bin/rm -rf -- "$fixture"
    trap - EXIT HUP INT TERM
}

run TEST-PROTO-008 protocol_tests
run TEST-CLI-008 cargo test --locked --offline -p mengxia --bin mengxia
run TEST-CONFIG-008 config_tests
run TEST-AUTH-008 cargo test --locked --offline -p mengxia-core-proto auth
run TEST-CURSOR-008 cargo test --locked --offline -p mengxia-app cursor
run TEST-QUERY-008 query_tests
run TEST-PAGINATION-008 cargo test --locked --offline -p mengxia-store-sqlite --test task_008_queries
ci_run_group 'TEST-VERIFY-008 TEST-CORRUPTION-008' verification_tests
run TEST-DESTINATION-008 cargo test --locked --offline -p mengxia-platform-fs materialization::tests::destination
run TEST-MATERIALIZE-008 materialize_tests
run TEST-RECOVERY-008 recovery_tests
run TEST-CANCEL-008 cancel_tests
run TEST-OBSERVABILITY-008 observability_tests
run TEST-HEALTH-008 health_tests
run TEST-ERROR-008 cargo test --locked --offline -p mengxia-types -p mengxia-ports error
run TEST-LIFECYCLE-008 lifecycle_tests
run TEST-ARCH-008 architecture_tests
run TEST-SUPPLY-008 cargo check --locked --offline --workspace --all-targets --all-features
run TEST-DOC-008 cargo test --locked --offline -p mengxia-testkit --test document_traceability
run TEST-ENDTOEND-008 e2e_test

if [ "$component" -eq 0 ]; then
    cargo fmt --all -- --check
    /usr/bin/env -u MENGXIA_ACL_BUILD_CLASS \
        cargo clippy --locked --offline --workspace --all-targets --all-features -- -D warnings
    cargo test --locked --offline --workspace --all-targets --all-features
    cargo test --locked --offline --workspace --doc
    cargo test --locked --offline -p mengxia-testkit --test naming
    git diff --check
fi

if [ "$mode" = formal ]; then
    ci_supply
    if [ "$component" -eq 0 ]; then
        scripts/verify-task-007.sh formal
    fi
elif [ "$component" -eq 0 ]; then
    scripts/verify-task-007.sh developer
fi
