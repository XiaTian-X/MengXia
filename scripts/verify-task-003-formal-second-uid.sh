#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

component=0
case "$#:${1-}" in
    0:) ;;
    1:component) component=1 ;;
    *) echo "usage: scripts/verify-task-003-formal-second-uid.sh [component]" >&2; exit 64 ;;
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

if [ "$component" -eq 0 ]; then
./scripts/verify-task-003.sh
fi
task003_run TEST-IPC-MACOS-001 -- ./scripts/run-task-003-second-uid.sh
