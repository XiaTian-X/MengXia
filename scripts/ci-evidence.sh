#!/bin/sh
# Sourced after each task parses its explicit native-component argument.
# No environment variable can turn standalone verification into a partial result.

ci_supply() {
    if [ "$native" -eq 1 ]; then
        echo 'SHARED_SUPPLY: REQUIRED_BY_AGGREGATE'
    else
        scripts/check-supply-chain.sh
    fi
}

ci_supply_unavailable() {
    if [ "$native" -eq 0 ]; then
        ci_negative_status=0
        ci_negative_output=$(scripts/check-supply-chain.sh --simulate-advisory-unavailable 2>&1) || ci_negative_status=$?
        test "$ci_negative_status" -eq 2
        printf '%s\n' "$ci_negative_output" | grep '^UNVERIFIABLE:'
    fi
}

ci_result() {
    if [ "$native" -eq 1 ]; then
        echo "$1: COMPONENT_PASS"
    elif [ "${mode-formal}" = developer ]; then
        echo "$1: FAST_PASS"
    else
        echo "$1: PASS"
    fi
}

ci_run_group() {
    ci_group_ids=$1
    shift
    test -n "$ci_group_ids"
    test "$#" -gt 0
    ci_seen_ids=' '
    for ci_group_id in $ci_group_ids; do
        grep -F -x "$ci_group_id" scripts/ci-baseline-mappings.txt >/dev/null || return 64
        case "$ci_seen_ids" in *" $ci_group_id "*) return 64 ;; esac
        ci_seen_ids="$ci_seen_ids$ci_group_id "
    done
    test "$ci_seen_ids" != ' '
    # Never place the command in an if/|| condition: POSIX shell would disable
    # errexit inside a called function and could hide an earlier failed assertion.
    "$@"
    for ci_group_id in $ci_group_ids; do
        ci_result "$ci_group_id"
    done
}

# Cargo considers a test filter matching nothing successful. Reject this when a
# task maps that invocation to evidence. Preserve every argument and environment.
cargo() {
    if [ "${1-}" != test ]; then
        command cargo "$@"
        return
    fi
    mkdir -p target
    ci_test_log=$(mktemp target/ci-test.XXXXXX)
    ci_test_status=0
    command cargo "$@" >"$ci_test_log" 2>&1 || ci_test_status=$?
    cat "$ci_test_log"
    if [ "$ci_test_status" -eq 0 ]; then
        if ! grep -E '^test result: ok\. [1-9][0-9]* passed;' "$ci_test_log" >/dev/null; then
            echo 'UNVERIFIABLE: mapped cargo test executed no passing tests' >&2
            ci_test_status=1
        fi
    fi
    rm -f -- "$ci_test_log"
    return "$ci_test_status"
}
