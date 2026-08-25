#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

task003_run() {
    test_id=$1
    shift
    test "$1" = "--"
    shift
    test "$#" -gt 0
    "$@"
    echo "$test_id: PASS"
}

./scripts/verify-task-003.sh
task003_run TEST-IPC-MACOS-001 -- ./scripts/run-task-003-second-uid.sh
