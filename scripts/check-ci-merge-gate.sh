#!/bin/sh
set -eu
if [ "${1-}" = --self-test ]; then
    test "$#" -eq 1
    gate_script=$0
    gate_fixture() {
        expected=$1; event=$2; scope=$3; shift 3
        native_result=success; fast_result=skipped; dependency_result=skipped
        if [ "$scope" = docs ]; then native_result=skipped; fast_result=success; fi
        if [ "$event" = pull_request ]; then fast_result=success; dependency_result=success; fi
        fixture_sha=0123456789abcdef0123456789abcdef01234567
        if env EVENT_NAME="$event" SCOPE="$scope" CLASSIFY_RESULT=success \
            FORMAL_RESULT="$native_result" SECOND_UID_RESULT="$native_result" SUPPLY_RESULT="$native_result" \
            PR_VALIDATION_RESULT="$fast_result" DEPENDENCY_RESULT="$dependency_result" \
            EXPECTED_SHA="$fixture_sha" FORMAL_SHA="$fixture_sha" SECOND_UID_SHA="$fixture_sha" SUPPLY_SHA="$fixture_sha" \
            "$@" /bin/sh "$gate_script" >/dev/null 2>&1; then
            test "$expected" = success
        else
            test "$expected" = failure
        fi
    }
    for event in pull_request push schedule workflow_dispatch; do
        gate_fixture success "$event" code
        for key in CLASSIFY_RESULT FORMAL_RESULT SECOND_UID_RESULT SUPPLY_RESULT; do
            for bad in failure cancelled timed_out skipped neutral unknown ''; do
                gate_fixture failure "$event" code "$key=$bad"
            done
        done
        for key in FORMAL_SHA SECOND_UID_SHA SUPPLY_SHA EXPECTED_SHA; do
            gate_fixture failure "$event" code "$key=ffffffffffffffffffffffffffffffffffffffff"
            gate_fixture failure "$event" code "$key="
        done
        gate_fixture failure "$event" unknown
        case "$event" in
            pull_request|push)
                gate_fixture success "$event" docs
                gate_fixture failure "$event" docs PR_VALIDATION_RESULT=skipped
                gate_fixture failure "$event" docs SUPPLY_RESULT=success
                ;;
            *) gate_fixture failure "$event" docs ;;
        esac
    done
    gate_fixture failure pull_request code DEPENDENCY_RESULT=skipped
    gate_fixture failure pull_request code PR_VALIDATION_RESULT=skipped
    gate_fixture failure pull_request_target code
    echo 'MERGE_GATE_SELF_TEST: PASS (fixtures, not repository acceptance)'
    exit 0
fi
test "$#" -eq 0
case "${EVENT_NAME-}" in pull_request|push|workflow_dispatch|schedule) ;; *) exit 64 ;; esac
[ "${CLASSIFY_RESULT-}" = success ]
case "${SCOPE-}" in
    docs)
        case "$EVENT_NAME" in pull_request|push) ;; *) exit 1 ;; esac
        [ "${PR_VALIDATION_RESULT-}" = success ]
        [ "${FORMAL_RESULT-}" = skipped ]
        [ "${SECOND_UID_RESULT-}" = skipped ]
        [ "${SUPPLY_RESULT-}" = skipped ]
        ;;
    code)
        [ "${FORMAL_RESULT-}" = success ]
        [ "${SECOND_UID_RESULT-}" = success ]
        [ "${SUPPLY_RESULT-}" = success ]
        [ "${#EXPECTED_SHA}" -eq 40 ]
        case "$EXPECTED_SHA" in *[!0-9a-f]*) exit 1 ;; esac
        [ "${FORMAL_SHA-}" = "$EXPECTED_SHA" ]
        [ "${SECOND_UID_SHA-}" = "$EXPECTED_SHA" ]
        [ "${SUPPLY_SHA-}" = "$EXPECTED_SHA" ]
        if [ "$EVENT_NAME" = pull_request ]; then
            [ "${PR_VALIDATION_RESULT-}" = success ]
        else
            [ "${PR_VALIDATION_RESULT-}" = skipped ]
        fi
        ;;
    *) exit 1 ;;
esac
if [ "$EVENT_NAME" = pull_request ]; then
    [ "${DEPENDENCY_RESULT-}" = success ]
else
    [ "${DEPENDENCY_RESULT-}" = skipped ]
fi
echo "REPOSITORY_EVIDENCE: PASS ($EVENT_NAME/$SCOPE)"
