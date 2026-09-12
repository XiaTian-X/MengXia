#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
test "$#" -eq 0
test -z "${MENGXIA_ACL_BUILD_CLASS-}"
native=0
. scripts/ci-evidence.sh
printf 'FAST_CHECKOUT_SHA: %s\n' "$(git rev-parse HEAD)"
cargo fmt --all --check
cargo check --locked --offline --workspace --all-targets --all-features
cargo clippy --locked --offline --workspace --all-targets --all-features -- -D warnings
cargo test --locked --offline -p mengxia-testkit --test task_004_foundation macos_acl
cargo test --locked --offline -p mengxia-testkit --test ci_orchestration
cargo test --locked --offline -p mengxia-testkit --test ci_evidence
git diff --check
echo 'FAST_FEEDBACK: PASS (not formal acceptance)'
