#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
case "$mode" in
    developer|formal) ;;
    *) echo "usage: scripts/verify-task-007.sh developer|formal" >&2; exit 64 ;;
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

protocol_tests() {
    cargo test --locked --offline -p mengxia-core-proto
    cargo test --locked --offline -p mengxia-testkit --test task_007_foundation proto
}

application_tests() {
    cargo test --locked --offline -p mengxia-app
    cargo test --locked --offline -p mengxia-store-sqlite --test task_006_assets \
        external_registration_is_atomic_shared_and_exactly_replayable
}

config_tests() {
    cargo test --locked --offline -p mengxia-app config::tests
    cargo test --locked --offline -p mengxia-platform-fs config_file::tests
    cargo test --locked --offline -p mengxia --bin mengxia typed_layers
    cargo test --locked --offline -p mengxiad --bin mengxiad typed_layers
}

root_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite validate_local_managed_backend
    cargo test --locked --offline -p mengxia-storage-local --test task_005_local_cas \
        backend_identity_tracks_root_inode_not_configured_path_text
}

recovery_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite --test task_006_assets \
        stale_claim_requires_recovery_and_terminal_disposition_replays_after_restart
    cargo test --locked --offline -p mengxia-storage-local --test task_005_recovery
}

lifecycle_tests() {
    cargo test --locked --offline -p mengxiad --bin mengxiad \
        tests::disconnect_extra_input_and_deadline_signal_and_join_owned_work -- --exact
    cargo test --locked --offline -p mengxiad --bin mengxiad \
        tests::leaked_owner_and_shutdown_timeout_exit_without_blocking_drop_unwind -- --exact
    cargo test --locked --offline -p mengxia-store-sqlite lifecycle::tests
}

concurrency_tests() {
    cargo test --locked --offline -p mengxia-app \
        ingest::tests::exact_active_duplicate_uses_one_shared_service_and_one_ingest -- --exact
    cargo test --locked --offline -p mengxia-app \
        ingest::tests::binding_and_execution_saturation_are_preclaim_and_leave_no_second_record -- --exact
    cargo test --locked --offline -p mengxia-store-sqlite --test task_006_assets concurrent
}

error_tests() {
    cargo test --locked --offline -p mengxia-app \
        ingest::tests::error_and_retry_mapping_is_total_and_fail_closed_at_the_claim_boundary -- --exact
    cargo test --locked --offline -p mengxia-core-proto \
        operation_encoder_rejects_invalid_code_retry_pairs_and_unknown_codes
    cargo test --locked --offline -p mengxia-types -p mengxia-ports error
}

e2e_test() {
    cargo build --locked --offline -p mengxia -p mengxiad
    fixture=$(mktemp -d "$repository_root/target/task007-e2e.XXXXXX")
    chmod 700 "$fixture"
    mkdir -m 700 "$fixture/runtime"
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
    attempts=0
    while [ ! -S "$fixture/runtime/client.sock" ]; do
        attempts=$((attempts + 1))
        if [ "$attempts" -ge 500 ]; then
            return 1
        fi
        sleep 0.01
    done
    target/debug/mengxia handshake \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/legacy.out"
    command_id=018d442f-c000-7a11-8022-334455667788
    ingest() {
        target/debug/mengxia asset ingest-copy "$fixture/source.bin" \
            --command-id "$command_id" --asset-kind file --content-kind binary \
            --representation-purpose original --resource-kind blob \
            --logical-name source.bin --operation-timeout-ms 10000 \
            --client-endpoint "$fixture/runtime/client.sock"
    }
    ingest >"$fixture/first.out"
    ingest >"$fixture/replay.out"
    cmp "$fixture/first.out" "$fixture/replay.out"
    second_command_id=018d442f-c000-7a11-8022-334455667789
    target/debug/mengxia asset ingest-copy "$fixture/source.bin" \
        --command-id "$second_command_id" --asset-kind file --content-kind binary \
        --representation-purpose original --resource-kind blob \
        --logical-name source.bin --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/second.out"
    first_asset=$(awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^asset_id=/) { sub(/^asset_id=/, "", $i); print $i } }' "$fixture/first.out")
    second_asset=$(awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^asset_id=/) { sub(/^asset_id=/, "", $i); print $i } }' "$fixture/second.out")
    first_blob=$(awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^blob_sha256=/) { sub(/^blob_sha256=/, "", $i); print $i } }' "$fixture/first.out")
    second_blob=$(awk '{ for (i = 1; i <= NF; i++) if ($i ~ /^blob_sha256=/) { sub(/^blob_sha256=/, "", $i); print $i } }' "$fixture/second.out")
    test -n "$first_asset"
    test -n "$first_blob"
    test "$first_asset" != "$second_asset"
    test "$first_blob" = "$second_blob"
    if target/debug/mengxia asset ingest-copy "$fixture/source.bin" \
        --command-id "$command_id" --asset-kind file --content-kind binary \
        --representation-purpose original --resource-kind blob \
        --logical-name changed --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" \
        >"$fixture/conflict.out" 2>"$fixture/conflict.err"; then
        return 1
    fi
    grep '^MENGXIA_ERROR code=CONFLICT retry=NONE$' "$fixture/conflict.err" >/dev/null

    retry_command_id=018d442f-c000-7a11-8022-334455667790
    if target/debug/mengxia asset ingest-copy "$fixture/retry-source.bin" \
        --command-id "$retry_command_id" --asset-kind file --content-kind binary \
        --representation-purpose original --resource-kind blob \
        --logical-name retry-source.bin --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" \
        >"$fixture/preclaim.out" 2>"$fixture/preclaim.err"; then
        return 1
    fi
    grep '^MENGXIA_ERROR code=STORAGE_IO_ERROR retry=SAME_COMMAND$' \
        "$fixture/preclaim.err" >/dev/null
    dd if=/dev/zero of="$fixture/retry-source.bin" bs=1 count=1 status=none
    chmod 600 "$fixture/retry-source.bin"
    target/debug/mengxia asset ingest-copy "$fixture/retry-source.bin" \
        --command-id "$retry_command_id" --asset-kind file --content-kind binary \
        --representation-purpose original --resource-kind blob \
        --logical-name retry-source.bin --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" >"$fixture/retry.out"

    terminal_command_id=018d442f-c000-7a11-8022-334455667791
    wrong_digest=0000000000000000000000000000000000000000000000000000000000000000
    terminal_ingest() {
        target/debug/mengxia asset ingest-copy "$fixture/source.bin" \
            --command-id "$terminal_command_id" --asset-kind file --content-kind binary \
            --representation-purpose original --resource-kind blob \
            --logical-name terminal-source.bin --expected-sha256 "$wrong_digest" \
            --operation-timeout-ms 10000 --client-endpoint "$fixture/runtime/client.sock"
    }
    if terminal_ingest >"$fixture/terminal-1.out" 2>"$fixture/terminal-1.err"; then
        return 1
    fi
    if terminal_ingest >"$fixture/terminal-2.out" 2>"$fixture/terminal-2.err"; then
        return 1
    fi
    cmp "$fixture/terminal-1.err" "$fixture/terminal-2.err"
    grep '^MENGXIA_ERROR code=STORAGE_CORRUPTION retry=OPERATOR_OR_RUNTIME_ACTION$' \
        "$fixture/terminal-1.err" >/dev/null
    if target/debug/mengxia asset ingest-copy "$fixture/source.bin" \
        --command-id "$terminal_command_id" --asset-kind file --content-kind binary \
        --representation-purpose original --resource-kind blob \
        --logical-name terminal-source.bin --operation-timeout-ms 10000 \
        --client-endpoint "$fixture/runtime/client.sock" \
        >"$fixture/terminal-conflict.out" 2>"$fixture/terminal-conflict.err"; then
        return 1
    fi
    grep '^MENGXIA_ERROR code=CONFLICT retry=NONE$' \
        "$fixture/terminal-conflict.err" >/dev/null
    kill -INT "$daemon_pid"
    wait "$daemon_pid"
    /bin/rm -rf -- "$fixture"
    trap - EXIT HUP INT TERM
}

run TEST-PROTO-007 protocol_tests
run TEST-CLI-007 cargo test --locked --offline -p mengxia --bin mengxia
run TEST-CONFIG-007 config_tests
run TEST-AUTH-007 cargo test --locked --offline -p mengxia-core-proto auth
run TEST-DIGEST-007 cargo test --locked --offline -p mengxia-app canonical_request_digest
run TEST-INGEST-007 application_tests
run TEST-SOURCE-007 cargo test --locked --offline -p mengxia-storage-local --test task_005_local_cas
run TEST-CUSTODY-007 application_tests
run TEST-COMMAND-007 application_tests
run TEST-CONCURRENCY-007 concurrency_tests
run TEST-CANCEL-007 cargo test --locked --offline -p mengxia-storage-local cooperative
run TEST-RECOVERY-007 recovery_tests
run TEST-ROOT-007 root_tests
run TEST-ERROR-007 error_tests
run TEST-LIFECYCLE-007 lifecycle_tests
run TEST-ARCH-007 cargo test --locked --offline -p mengxia-testkit --test task_007_foundation architecture
run TEST-SUPPLY-007 cargo check --locked --offline --workspace --all-targets --all-features
run TEST-DOC-007 cargo test --locked --offline -p mengxia-testkit --test document_traceability
run TEST-ENDTOEND-007 e2e_test

cargo fmt --all -- --check
cargo clippy --locked --offline --workspace --all-targets --all-features -- -D warnings
cargo test --locked --offline --workspace --all-targets --all-features
cargo test --locked --offline --workspace --doc
cargo test --locked --offline -p mengxia-testkit --test naming
git diff --check

if [ "$mode" = formal ]; then
    MENGXIA_TASK007_STRESS_ITERATIONS=100 \
        cargo test --locked --offline -p mengxia-app \
        ingest::tests::exact_active_duplicate_uses_one_shared_service_and_one_ingest -- --exact
    MENGXIA_TASK007_STRESS_ITERATIONS=100 \
        cargo test --locked --offline -p mengxia-app \
        ingest::tests::binding_and_execution_saturation_are_preclaim_and_leave_no_second_record -- --exact
    scripts/check-supply-chain.sh
    scripts/verify-task-006.sh formal
else
    scripts/verify-task-006.sh developer
fi
