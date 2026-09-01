#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

component=0
case "$#:${1-}" in
    0:) ;;
    1:component) component=1 ;;
    *) echo "usage: scripts/verify-task-003.sh [component]" >&2; exit 64 ;;
esac

task003_run() {
    test_id=$1
    shift
    test "$1" = "--"
    shift
    test "$#" -gt 0
    "$@"
    echo "$test_id: PASS"
}

task003_run TEST-PROTO-001 -- cargo test --locked --offline -p mengxia-testkit --test task_003_foundation descriptor_and_offline_generator_inputs_are_source_pinned
task003_run TEST-FRAME-001 -- cargo test --locked --offline -p mengxia-framing -p mengxia-core-proto
task003_run TEST-HANDSHAKE-001 -- cargo test --locked --offline -p mengxia-core-proto
task003_run TEST-ENDPOINT-003 -- cargo test --locked --offline -p mengxia-platform-fs --lib runtime_endpoint
task003_run TEST-CONFIG-003 -- ./scripts/run-task-003-cli-tests.sh
task003_run TEST-AUTH-001 -- cargo test --locked --offline -p mengxia-core-proto auth_
task003_run TEST-CLI-001 -- ./scripts/run-task-003-cli-tests.sh
task003_run TEST-ARCH-003 -- cargo test --locked --offline -p mengxia-testkit --test task_003_foundation task_003_dependency_and_authority_boundaries_are_exact
task003_run TEST-SUPPLY-003 -- ./scripts/check-supply-chain.sh
task003_run TEST-DOC-003 -- cargo test --locked --offline -p mengxia-testkit --test document_traceability

if [ "$component" -eq 0 ]; then
    ./scripts/verify-task-001.sh
    ./scripts/verify-task-002.sh
    ./scripts/verify-task-004.sh
fi
