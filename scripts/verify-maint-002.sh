#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
test "$#" -eq 0
native=0
. scripts/ci-evidence.sh
cargo test --locked --offline -p mengxia-testkit --test ci_evidence
cargo test --locked --offline -p mengxia-testkit --test ci_orchestration
echo 'MAINT-002 LOCAL: PASS; reviewed PR/main evidence required before DONE'
