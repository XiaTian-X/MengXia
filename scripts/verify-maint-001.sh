#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-developer}
case "$mode" in
    developer|formal) ;;
    *) echo "usage: scripts/verify-maint-001.sh developer|formal" >&2; exit 64 ;;
esac

native=0
case "${2-}" in
    "") ;;
    native-component) native=1 ;;
    *) exit 64 ;;
esac
test "$#" -le 2
. scripts/ci-evidence.sh

maint_run() {
    test_id=$1
    shift
    test "$#" -gt 0
    echo "$test_id: $*"
    "$@"
}

maint_run TEST-MAINT-CI-001 cargo test --locked --offline -p mengxia-testkit --test ci_orchestration
maint_run TEST-MAINT-TOOLCHAIN-001 cargo test --locked --offline -p mengxia-testkit --test task_004_foundation macos_acl
if [ "$mode" = formal ]; then
    maint_run TEST-MAINT-PROTO-001 ./scripts/verify-proto-artifacts.sh
else
    maint_run TEST-MAINT-PROTO-001 cargo test --locked --offline -p mengxia-testkit --test task_003_foundation descriptor_and_offline_generator_inputs_are_source_pinned
fi
maint_run TEST-MAINT-SUPPLY-001 ci_supply
maint_run TEST-MAINT-DOC-001 cargo test --locked --offline -p mengxia-testkit --test document_traceability
