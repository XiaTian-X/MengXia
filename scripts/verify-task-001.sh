#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

run_test_boot_001() {
    echo "TEST-BOOT-001: pinned toolchain and locked metadata"
    test "$(uname -m)" = "arm64"
    rustc --version --verbose
    cargo --version --verbose
    rustc --version | grep '^rustc 1\.98\.0 '
    cargo metadata --format-version 1 --no-deps --locked >/dev/null
}

run_test_boot_002() {
    echo "TEST-BOOT-002: format, build, check, Clippy and test gates"
    cargo fmt --all --check
    cargo build --workspace --all-targets --all-features --locked
    cargo check --workspace --all-targets --all-features --locked
    # Cargo implements Clippy through RUSTC_WORKSPACE_WRAPPER=clippy-driver.
    # Keep that tool-managed wrapper outside TASK-004's attested FFI build class;
    # the following test command returns to the caller's attested environment.
    /usr/bin/env -u MENGXIA_ACL_BUILD_CLASS \
        cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    cargo test --workspace --all-targets --all-features --locked
}

run_test_arch_001() {
    echo "TEST-ARCH-001: dependency direction and forbidden fixture"
    cargo test -p mengxia-testkit --test architecture --locked
}

run_test_name_001() {
    echo "TEST-NAME-001: canonical package/path inventory and repository hygiene"
    cargo test -p mengxia-testkit --test naming --locked
}

run_test_supply_001() {
    echo "TEST-SUPPLY-001: pinned source/license/advisory policy"
    set +e
    unavailable_output=$(scripts/check-supply-chain.sh --simulate-advisory-unavailable 2>&1)
    unavailable_status=$?
    set -e
    test "$unavailable_status" -eq 2
    printf '%s\n' "$unavailable_output" | grep '^UNVERIFIABLE:'
    scripts/check-supply-chain.sh
}

run_test_doc_001() {
    echo "TEST-DOC-001: stable-ID and task lifecycle traceability"
    cargo test -p mengxia-testkit --test document_traceability --locked
}

run_one() {
    case "$1" in
        TEST-BOOT-001) run_test_boot_001 ;;
        TEST-BOOT-002) run_test_boot_002 ;;
        TEST-ARCH-001) run_test_arch_001 ;;
        TEST-NAME-001) run_test_name_001 ;;
        TEST-SUPPLY-001) run_test_supply_001 ;;
        TEST-DOC-001) run_test_doc_001 ;;
        *) echo "unknown TASK-001 test ID: $1" >&2; exit 64 ;;
    esac
}

if [ "$#" -eq 0 ]; then
    for test_id in \
        TEST-BOOT-001 \
        TEST-BOOT-002 \
        TEST-ARCH-001 \
        TEST-NAME-001 \
        TEST-SUPPLY-001 \
        TEST-DOC-001
    do
        run_one "$test_id"
    done
else
    for test_id in "$@"; do
        run_one "$test_id"
    done
fi
