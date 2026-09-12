#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
case "$mode" in
    developer|formal) ;;
    *) echo "usage: scripts/verify-task-009.sh developer|formal [component]" >&2; exit 64 ;;
esac
native=0
component=0
case "${2-}" in
    "") ;;
    component) component=1 ;;
    native-component) component=1; native=1 ;;
    *) echo "usage: scripts/verify-task-009.sh developer|formal [component]" >&2; exit 64 ;;
esac
test "$#" -le 2

. scripts/ci-evidence.sh

run() {
    test_id=$1
    shift
    "$@"
    ci_result "$test_id"
}

migration_tests() {
    cargo test --locked --offline -p mengxia-testkit --test task_009_foundation
    cargo test --locked --offline -p mengxia-store-sqlite migration::tests::creative_migration
    cargo test --locked --offline -p mengxia-store-sqlite --test task_009_creative migration_capacity
    cargo test --locked --offline -p mengxia-platform-fs migration_snapshot::tests
}

schema_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite migration::tests
    cargo test --locked --offline -p mengxia-store-sqlite --test task_006_assets current_schema_tamper
}

outcome_tests() {
    cargo test --locked --offline -p mengxia-ports versioned_result_payloads
    cargo test --locked --offline -p mengxia-store-sqlite command_row_mapper
    cargo test --locked --offline -p mengxia-store-sqlite creative_repository::tests
}

replay_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite creative_repository::tests
    cargo test --locked --offline -p mengxia-store-sqlite --test task_009_creative deferred_asset_commands
}

domain_tests() {
    cargo test --locked --offline -p mengxia-domain
}

json_tests() {
    cargo test --locked --offline -p mengxia-app creative::tests::canonicalization
    cargo test --locked --offline -p mengxia-app creative::tests::numeric_and_structural
}

config_tests() {
    cargo test --locked --offline -p mengxia-app config::tests
    cargo test --locked --offline -p mengxia-store-sqlite config::tests
    cargo test --locked --offline -p mengxia-storage-local config::tests
    cargo test --locked --offline -p mengxia --bin mengxia typed_layers
    cargo test --locked --offline -p mengxiad --bin mengxiad typed_layers
}

protocol_tests() {
    cargo test --locked --offline -p mengxia-core-proto
    cargo test --locked --offline -p mengxia-testkit --test task_008_foundation protocol
    cargo test --locked --offline -p mengxia-testkit --test task_009_foundation protocol
}

auth_tests() {
    cargo test --locked --offline -p mengxia-core-proto auth
    cargo test --locked --offline -p mengxia-store-sqlite \
        creative_query::tests::list_work_distinguishes_an_empty_project_from_a_missing_scope
    cargo test --locked --offline -p mengxia-store-sqlite \
        creative_repository::tests::work_and_take_history_survives_later_mutations_and_explicit_selection
}

project_subject_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite creative_repository::tests::project_subject
    cargo test --locked --offline -p mengxia-store-sqlite creative_query::tests
}

work_take_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite creative_repository::tests::work_and_take
    cargo test --locked --offline -p mengxia-domain creative::tests::take_state_machine
}

asset_lifecycle_tests() {
    cargo test --locked --offline -p mengxia-domain creative_revision
    cargo test --locked --offline -p mengxia-store-sqlite asset_repository::tests::asset_lifecycle
    cargo test --locked --offline -p mengxia-store-sqlite --test task_009_creative deferred_asset_commands
}

concurrency_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite --test task_006_assets concurrent_pure_duplicate
    cargo test --locked --offline -p mengxia-store-sqlite lifecycle::tests::concurrent_submission
    cargo test --locked --offline -p mengxia-store-sqlite creative_repository::tests::work_and_take
}

pagination_tests() {
    cargo test --locked --offline -p mengxia-app creative::tests::creative_cursor
    cargo test --locked --offline -p mengxia-store-sqlite creative_query::tests
}

corruption_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite migration::tests
    cargo test --locked --offline -p mengxia-store-sqlite command_row_mapper
    cargo test --locked --offline -p mengxia-ports versioned_result_payloads
    cargo test --locked --offline -p mengxia-store-sqlite \
        creative_query::tests::creative_queries_reject_digest_consistent_noncanonical_json
    cargo test --locked --offline -p mengxia-store-sqlite \
        creative_repository::tests::current_schema_reopen_rejects_creative_semantic_corruption
}

recovery_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite --test task_009_creative migration_capacity
    cargo test --locked --offline -p mengxia-store-sqlite pure_transaction_sigkill
}

error_tests() {
    cargo test --locked --offline -p mengxia-types -p mengxia-ports error
    cargo test --locked --offline -p mengxia-store-sqlite error::tests
    cargo test --locked --offline -p mengxia --bin mengxia operation_retry
}

observability_tests() {
    cargo test --locked --offline -p mengxia-app observability::tests
    cargo test --locked --offline -p mengxia-store-sqlite \
        verification::tests::task_009_command_registry_accepts_valid_operations_and_rejects_unknown
}

lifecycle_tests() {
    cargo test --locked --offline -p mengxia-store-sqlite lifecycle::tests
    cargo test --locked --offline -p mengxiad --bin mengxiad disconnect_extra_input
    cargo test --locked --offline -p mengxiad --bin mengxiad leaked_owner
}

architecture_tests() {
    cargo test --locked --offline -p mengxia-testkit --test architecture
    cargo test --locked --offline -p mengxia-testkit --test task_009_foundation
}

e2e_test() {
    cargo build --locked --offline -p mengxia -p mengxiad
    fixture=$(mktemp -d "$repository_root/target/task009-e2e.XXXXXX")
    chmod 700 "$fixture"
    mkdir -m 700 "$fixture/runtime"
    dd if=/dev/zero of="$fixture/source-one.bin" bs=4096 count=1 status=none
    dd if=/dev/zero of="$fixture/source-two.bin" bs=4096 count=2 status=none
    chmod 600 "$fixture/source-one.bin" "$fixture/source-two.bin"
    daemon_pid=
    cleanup() {
        if [ -n "${daemon_pid-}" ]; then
            kill -INT "$daemon_pid" 2>/dev/null || true
            wait "$daemon_pid" 2>/dev/null || true
        fi
        /bin/rm -rf -- "$fixture"
    }
    trap cleanup EXIT HUP INT TERM

    start_daemon() {
        target/debug/mengxiad serve \
            --library-root "$fixture/Library" \
            --client-endpoint "$fixture/runtime/client.sock" \
            >>"$fixture/daemon.out" 2>>"$fixture/daemon.err" &
        daemon_pid=$!
    }
    wait_ready() {
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
    }
    field() {
        key=$1
        file=$2
        awk -v prefix="$key=" '{ for (i = 1; i <= NF; i++) if (index($i, prefix) == 1) { sub(prefix, "", $i); print $i; exit } }' "$file"
    }
    client() {
        target/debug/mengxia "$@" --operation-timeout-ms 5000 \
            --client-endpoint "$fixture/runtime/client.sock"
    }

    start_daemon
    wait_ready

    client asset ingest-copy "$fixture/source-one.bin" \
        --command-id 018d442f-c100-7a11-8022-334455667701 \
        --asset-kind file --content-kind binary --representation-purpose original \
        --resource-kind blob --logical-name source-one.bin >"$fixture/ingest-one.out"
    client asset ingest-copy "$fixture/source-two.bin" \
        --command-id 018d442f-c100-7a11-8022-334455667702 \
        --asset-kind file --content-kind binary --representation-purpose original \
        --resource-kind blob --logical-name source-two.bin >"$fixture/ingest-two.out"
    asset_one=$(field asset_id "$fixture/ingest-one.out")
    asset_two=$(field asset_id "$fixture/ingest-two.out")
    blob_one=$(field blob_sha256 "$fixture/ingest-one.out")
    test -n "$asset_one"
    test -n "$asset_two"
    test -n "$blob_one"

    client asset inspect --asset-id "$asset_one" --page-size 64 >"$fixture/inspect.out"
    parent_revision=$(field asset_revision_id "$fixture/inspect.out")
    test -n "$parent_revision"
    client asset create-revision \
        --command-id 018d442f-c100-7a11-8022-334455667703 \
        --asset-id "$asset_one" --expected-revision 1 \
        --parent-revision-id "$parent_revision" --content-kind binary \
        --representation original --resource blob \
        --member "736f757263652d6f6e652e62696e:$blob_one" \
        >"$fixture/asset-revision.out"
    grep '^resulting_revision=2$' "$fixture/asset-revision.out" >/dev/null
    client asset retire --command-id 018d442f-c100-7a11-8022-334455667704 \
        --asset-id "$asset_one" --expected-revision 2 >"$fixture/asset-retire.out"
    grep '^lifecycle=RETIRED$' "$fixture/asset-retire.out" >/dev/null
    client asset restore --command-id 018d442f-c100-7a11-8022-334455667705 \
        --asset-id "$asset_one" --expected-revision 3 >"$fixture/asset-restore.out"
    grep '^lifecycle=ACTIVE$' "$fixture/asset-restore.out" >/dev/null

    project_create() {
        client project create --command-id 018d442f-c100-7a11-8022-334455667706 \
            --name-hex 50726f6a65637420416c706861 --resolution 1920x1080 \
            --frame-rate 24/1 --aspect-ratio 16/9 \
            --color-policy-json-hex 7b7d --audio-policy-json-hex 7b7d \
            --quality-policy-json-hex 7b7d --privacy-policy-json-hex 7b7d
    }
    project_create >"$fixture/project-create.out"
    project_id=$(field project_id "$fixture/project-create.out")
    test -n "$project_id"
    project_create >"$fixture/project-replay.out"
    grep ' replayed=1$' "$fixture/project-replay.out" >/dev/null
    client project revise-spec --command-id 018d442f-c100-7a11-8022-334455667707 \
        --project-id "$project_id" --expected-revision 1 \
        --color-policy-json-hex 7b2276223a317d --audio-policy-json-hex 7b7d \
        --quality-policy-json-hex 7b7d --privacy-policy-json-hex 7b7d \
        >"$fixture/project-revise.out"
    grep '^project_revision=2$' "$fixture/project-revise.out" >/dev/null
    client project list --page-size 64 >"$fixture/project-list.out"
    grep "^project_id=$project_id$" "$fixture/project-list.out" >/dev/null

    client subject create --command-id 018d442f-c100-7a11-8022-334455667708 \
        --kind person --canonical-name-hex 5375626a656374204f6e65 \
        >"$fixture/subject-create.out"
    subject_id=$(field subject_id "$fixture/subject-create.out")
    test -n "$subject_id"
    client subject list --page-size 64 >"$fixture/subject-list.out"
    grep "^subject_id=$subject_id$" "$fixture/subject-list.out" >/dev/null

    client work create --command-id 018d442f-c100-7a11-8022-334455667709 \
        --project-id "$project_id" --kind shot --code-hex 53484f542d303031 \
        --specification-json-hex 7b227363656e65223a317d \
        --subject-id "$subject_id" --asset-id "$asset_one" \
        >"$fixture/work-create.out"
    work_item_id=$(field work_item_id "$fixture/work-create.out")
    work_revision_id=$(field work_revision_id "$fixture/work-create.out")
    test -n "$work_item_id"
    test -n "$work_revision_id"
    client work revise --command-id 018d442f-c100-7a11-8022-33445566770a \
        --project-id "$project_id" --work-item-id "$work_item_id" \
        --expected-revision 1 --specification-json-hex 7b227363656e65223a327d \
        --subject-id "$subject_id" --asset-id "$asset_one" --asset-id "$asset_two" \
        >"$fixture/work-revise.out"
    work_revision_id=$(field work_revision_id "$fixture/work-revise.out")
    grep '^work_item_revision=2$' "$fixture/work-revise.out" >/dev/null
    client work list --project-id "$project_id" --page-size 64 >"$fixture/work-list.out"
    grep "^work_item_id=$work_item_id$" "$fixture/work-list.out" >/dev/null

    client take create --command-id 018d442f-c100-7a11-8022-33445566770b \
        --project-id "$project_id" --work-item-id "$work_item_id" \
        --work-revision-id "$work_revision_id" --primary-asset-id "$asset_one" \
        >"$fixture/take-create.out"
    take_id=$(field take_id "$fixture/take-create.out")
    test -n "$take_id"
    client take transition --command-id 018d442f-c100-7a11-8022-33445566770c \
        --project-id "$project_id" --work-item-id "$work_item_id" \
        --work-revision-id "$work_revision_id" --take-id "$take_id" \
        --expected-revision 1 --transition reject --reason-hex 6e6f \
        >"$fixture/take-transition.out"
    grep '^state=REJECTED$' "$fixture/take-transition.out" >/dev/null
    client take reopen --command-id 018d442f-c100-7a11-8022-33445566770d \
        --project-id "$project_id" --work-item-id "$work_item_id" \
        --work-revision-id "$work_revision_id" --terminal-take-id "$take_id" \
        --expected-revision 2 --new-primary-asset-id "$asset_two" \
        >"$fixture/take-reopen.out"
    reopened_take_id=$(field take_id "$fixture/take-reopen.out")
    test -n "$reopened_take_id"
    test "$reopened_take_id" != "$take_id"
    client take list --project-id "$project_id" --work-item-id "$work_item_id" \
        --work-revision-id "$work_revision_id" --page-size 64 >"$fixture/take-list.out"
    grep "^take_id=$take_id$" "$fixture/take-list.out" >/dev/null
    grep "^take_id=$reopened_take_id$" "$fixture/take-list.out" >/dev/null

    kill -INT "$daemon_pid"
    wait "$daemon_pid"
    daemon_pid=
    start_daemon
    wait_ready
    project_create >"$fixture/project-restart-replay.out"
    grep ' replayed=1$' "$fixture/project-restart-replay.out" >/dev/null

    if grep -F "$fixture" "$fixture/daemon.out" "$fixture/daemon.err" >/dev/null || \
        grep -F 'Project Alpha' "$fixture/daemon.out" "$fixture/daemon.err" >/dev/null || \
        grep -F 'Subject One' "$fixture/daemon.out" "$fixture/daemon.err" >/dev/null; then
        return 1
    fi
    kill -INT "$daemon_pid"
    wait "$daemon_pid"
    daemon_pid=
    /bin/rm -rf -- "$fixture"
    trap - EXIT HUP INT TERM
}

run TEST-MIGRATION-009 migration_tests
run TEST-SCHEMA-009 schema_tests
run TEST-OUTCOME-009 outcome_tests
run TEST-REPLAY-009 replay_tests
run TEST-EVENT-009 cargo test --locked --offline -p mengxia-events
run TEST-DOMAIN-009 domain_tests
run TEST-JSON-009 json_tests
run TEST-CONFIG-009 config_tests
run TEST-PROTO-009 protocol_tests
run TEST-CLI-009 cargo test --locked --offline -p mengxia --bin mengxia
run TEST-AUTH-009 auth_tests
ci_run_group 'TEST-PROJECT-009 TEST-SUBJECT-009' project_subject_tests
ci_run_group 'TEST-WORK-009 TEST-TAKE-009' work_take_tests
run TEST-ASSET-LIFECYCLE-009 asset_lifecycle_tests
run TEST-CONCURRENCY-009 concurrency_tests
run TEST-PAGINATION-009 pagination_tests
run TEST-CORRUPTION-009 corruption_tests
run TEST-RECOVERY-009 recovery_tests
run TEST-ERROR-009 error_tests
run TEST-OBSERVABILITY-009 observability_tests
run TEST-LIFECYCLE-009 lifecycle_tests
run TEST-ARCH-009 architecture_tests
run TEST-SUPPLY-009 cargo check --locked --offline --workspace --all-targets --all-features
run TEST-DOC-009 cargo test --locked --offline -p mengxia-testkit --test document_traceability
run TEST-ENDTOEND-009 e2e_test

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
        scripts/verify-task-008.sh formal
    fi
elif [ "$component" -eq 0 ]; then
    scripts/verify-task-008.sh developer
fi

if [ "$native" -eq 1 ]; then
    echo "TASK-009 $mode: COMPONENT_PASS"
else
    echo "TASK-009 $mode GATE: PASS"
fi
