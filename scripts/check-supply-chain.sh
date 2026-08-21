#!/bin/sh
set -eu

cargo_deny_bin=${CARGO_DENY_BIN:-cargo-deny}
expected_version="cargo-deny 0.20.2"

if ! actual_version=$("$cargo_deny_bin" --version 2>/dev/null); then
    echo "UNVERIFIABLE: cargo-deny 0.20.2 is unavailable" >&2
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
