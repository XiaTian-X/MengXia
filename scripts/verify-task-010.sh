#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
case "$mode" in
    developer|formal) ;;
    *) echo "usage: scripts/verify-task-010.sh developer|formal [component]" >&2; exit 64 ;;
esac
component=0
case "${2-}" in
    "") ;;
    component) component=1 ;;
    *) echo "usage: scripts/verify-task-010.sh developer|formal [component]" >&2; exit 64 ;;
esac
test "$#" -le 2

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

manifest_check() {
    cargo test --locked --offline -p mengxia-plugin-package manifest::tests
    cargo test --locked --offline -p mengxia-testkit --test task_010_foundation manifest_schema_and_golden_bytes_are_exact_and_offline
    cargo test --locked --offline -p mengxia-testkit --test task_010_foundation unknown_noncanonical_and_invalid_security_fields_never_disappear
}

diff_check() {
    cargo test --locked --offline -p mengxia-plugin-security
    cargo test --locked --offline -p mengxia-testkit --test task_010_foundation semantic_diff_is_total_sorted_and_reaches_the_exact_193_bound
}

supply_check() {
    test "$(shasum -a 256 Cargo.lock | awk '{print $1}')" = \
        302df8141acee77aa58ecb796a53ecbb4faf9f6cd55dc384667bb08e725c0b2e
    cargo tree --locked --offline -e features -p mengxia-plugin-package >target/task010-features.txt
    ! grep -E 'resolve-http|resolve-file|reqwest|rustls|native-tls|idna' target/task010-features.txt
    cargo deny --locked check advisories bans licenses sources
    cargo test --locked --offline -p mengxia-testkit --test task_010_foundation dependency_and_supply
}

run TEST-MANIFEST-010 manifest_check
run TEST-PACKAGE-010 cargo test --locked --offline -p mengxia-testkit --test task_010_foundation package_digest
run TEST-DEPENDENCY-010 cargo test --locked --offline -p mengxia-testkit --test task_010_foundation dependency_declarations
run TEST-DIFF-010 diff_check
run TEST-PUBLISHER-010 cargo test --locked --offline -p mengxia-testkit --test task_010_foundation publisher_text
run TEST-BOUNDS-010 cargo test --locked --offline -p mengxia-testkit --test task_010_foundation parser_and_collection
run TEST-ARCH-010 cargo test --locked --offline -p mengxia-testkit --test architecture
run TEST-SUPPLY-010 supply_check
run TEST-DOC-010 cargo test --locked --offline -p mengxia-testkit --test document_traceability

if [ "$component" -eq 0 ]; then
    cargo fmt --all -- --check
    cargo clippy --locked --offline --workspace --all-targets --all-features -- -D warnings
    cargo test --locked --offline --workspace --all-targets --all-features
    git diff --check
fi
