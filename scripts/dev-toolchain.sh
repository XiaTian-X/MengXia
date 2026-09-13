#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
. scripts/toolchain-environment.sh
case "${1-}" in
    prepare)
        [ "$#" -eq 2 ] && [ "$2" = --network ] || tool_error NEEDS_PREPARATION 'prepare requires explicit --network'
        tool_prepare
        ;;
    inspect)
        [ "$#" -eq 1 ] || exit 64
        toolchain_observe
        printf '%s\n' "$tool_observation" "environment_fingerprint=$tool_fingerprint"
        echo 'COMPATIBILITY: NOT_TESTED; SECURITY_COVERAGE: UNKNOWN'
        tool_resolve
        echo 'TOOLS_AVAILABLE: verified; no compatibility claim'
        ;;
    verify)
        [ "$#" -eq 1 ] || exit 64
        exec scripts/verify-toolchain-candidate.sh
        ;;
    fast)
        [ "$#" -eq 1 ] || exit 64
        exec scripts/verify-ci-fast.sh
        ;;
    check|build|test)
        [ "$#" -eq 1 ] || exit 64
        tool_action=$1
        toolchain_environment
        cargo "$tool_action" --locked --offline --workspace --all-targets --all-features
        toolchain_environment_finish
        ;;
    *) echo 'usage: scripts/dev-toolchain.sh inspect|prepare --network|verify|fast|check|build|test' >&2; exit 64 ;;
esac
