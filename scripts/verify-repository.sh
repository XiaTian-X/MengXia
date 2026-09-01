#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
case "$mode" in
    docs)
        cargo test --locked --offline -p mengxia-testkit --test document_traceability
        cargo test --locked --offline -p mengxia-testkit --test naming
        cargo test --locked --offline -p mengxia-testkit --test ci_orchestration
        git diff --check
        exit 0
        ;;
    developer|formal) ;;
    *) echo "usage: scripts/verify-repository.sh docs|developer|formal" >&2; exit 64 ;;
esac

# One repository baseline followed by each task's owned mappings exactly once.
scripts/verify-task-001.sh
scripts/verify-task-002.sh
scripts/verify-task-004.sh --component
scripts/verify-task-003.sh component
scripts/verify-task-005.sh "$mode" component
scripts/verify-task-006.sh "$mode" component
scripts/verify-task-007.sh "$mode" component
