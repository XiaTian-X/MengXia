#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
. scripts/toolchain-environment.sh
test "$#" -le 1
case "${1-}" in ''|--simulate-advisory-unavailable) ;; *) exit 64 ;; esac
toolchain_rust
tool_resolve
cargo_deny_bin=$CARGO_DENY_BIN
expected_version="cargo-deny $tool_version"

if ! actual_version=$("$cargo_deny_bin" --version 2>/dev/null); then
    echo "UNVERIFIABLE: cargo-deny $tool_version is unavailable" >&2
    exit 2
fi

if [ "$actual_version" != "$expected_version" ]; then
    echo "UNVERIFIABLE: expected '$expected_version', got '$actual_version'" >&2
    exit 2
fi

if [ "${1:-}" = "--simulate-advisory-unavailable" ]; then
    echo "UNVERIFIABLE: advisory database is unavailable (simulated negative check)" >&2
    exit 2
fi

if ! "$cargo_deny_bin" fetch db; then
    echo "UNVERIFIABLE: current advisory database could not be fetched" >&2
    exit 2
fi

"$cargo_deny_bin" --locked check all
scripts/toolchain-maintenance.sh gate
