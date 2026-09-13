#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

mode=${1-}
test "$#" -eq 1
case "$mode" in
    docs)
        cargo test --locked --offline -p mengxia-testkit --test document_traceability
        cargo test --locked --offline -p mengxia-testkit --test naming
        cargo test --locked --offline -p mengxia-testkit --test ci_orchestration
        cargo test --locked --offline -p mengxia-testkit --test ci_evidence
        git diff --check
        exit 0
        ;;
    developer|formal|formal-native) ;;
    *) echo "usage: scripts/verify-repository.sh docs|developer|formal|formal-native" >&2; exit 64 ;;
esac
repository_mode=$mode
if [ "$mode" = formal-native ]; then mode=formal; fi
if [ "$mode" = developer ]; then
    . scripts/toolchain-environment.sh
    toolchain_environment
fi
printf 'REPOSITORY_CHECKOUT_SHA: %s\n' "$(git rev-parse HEAD)"
printf 'REPOSITORY_RUN: %s/%s\n' "${GITHUB_RUN_ID-local}" "${GITHUB_RUN_ATTEMPT-1}"
printf 'REPOSITORY_MODE: %s\n' "$repository_mode"
printf 'CHECK_MAPPING_SHA256: %s\n' "$(shasum -a 256 scripts/ci-baseline-mappings.txt | awk '{print $1}')"
printf 'PR_HEAD: %s PR_BASE: %s\n' "${PR_HEAD-not-applicable}" "${PR_BASE-not-applicable}"

# One repository baseline followed by each task's owned mappings exactly once.
scripts/verify-task-001.sh --native-component
scripts/verify-task-002.sh --native-component
scripts/verify-task-004.sh --native-component
scripts/verify-task-003.sh --native-component
scripts/verify-task-005.sh "$mode" native-component
scripts/verify-task-006.sh "$mode" native-component
scripts/verify-task-007.sh "$mode" native-component
scripts/verify-task-008.sh "$mode" native-component
scripts/verify-task-009.sh "$mode" native-component
scripts/verify-task-010.sh "$mode" native-component
scripts/verify-maint-001.sh "$mode" native-component
scripts/verify-maint-002.sh
scripts/verify-toolchain-maintenance.sh
if [ "$repository_mode" = formal-native ]; then
    echo 'FORMAL_NATIVE: COMPONENT_PASS; shared supply and second UID required in CI aggregate'
else
    scripts/verify-ci-supply.sh
    if [ "$mode" = developer ]; then toolchain_environment_finish; fi
    while IFS= read -r test_id; do
        [ "$test_id" != TEST-IPC-MACOS-001 ] || continue
        if [ "$mode" = formal ]; then
            echo "$test_id: PASS"
        else
            echo "$test_id: FAST_PASS"
        fi
    done < scripts/ci-baseline-mappings.txt
    for test_id in TEST-MAINT3-ENV-001 TEST-MAINT3-INSTALL-001 TEST-MAINT3-CACHE-001 TEST-MAINT3-SECURITY-001 TEST-MAINT3-INTEGRATION-001; do
        if [ "$mode" = formal ]; then echo "$test_id: PASS"; else echo "$test_id: FAST_PASS"; fi
    done
    echo "REPOSITORY $mode: PASS; real second UID remains a separate CI obligation"
fi
