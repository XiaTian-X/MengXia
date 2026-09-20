#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
script_root="$repository_root/scripts/native-research"
if ! command -v compile_with_deadline >/dev/null 2>&1; then
    exec "$script_root/run.sh" self-test
fi
runner="$script_root/run.sh"
case_manifest="$script_root/cases.tsv"
self_test_root=$(/usr/bin/mktemp -d /tmp/mengxia-r0a-self-test.XXXXXXXX)
umask 077

cleanup() {
    case "$self_test_root" in /tmp/mengxia-r0a-self-test.*)
        [ -d "$self_test_root" ] && [ ! -L "$self_test_root" ] && rm -rf -- "$self_test_root" ;;
    esac
}
trap cleanup EXIT
trap cancel_run HUP INT TERM

sha256_file() {
    /usr/bin/shasum -a 256 "$1" | /usr/bin/awk '{print $1}'
}

new_root() {
    created=$(/usr/bin/mktemp -d "$self_test_root/fixture.XXXXXXXX")
    chmod 700 "$created"
    printf '%s\n' "$created"
}

write_metadata() {
    directory=$1
    zero=0000000000000000000000000000000000000000000000000000000000000000
    {
        printf 'schema\t2\n'
        printf 'evidence_kind\tSYNTHETIC\n'
        printf 'budget_version\tR0A-2\n'
        printf 'created_utc\t1970-01-01T00:00:00Z\n'
        printf 'os_product\tSYNTHETIC\n'
        printf 'os_version\t0\n'
        printf 'os_build\tSYNTHETIC\n'
        printf 'arch\tarm64\n'
        printf 'xcode_version\t0\n'
        printf 'xcode_build\tSYNTHETIC\n'
        printf 'sdk_version\t0\n'
        printf 'clang_path\t/usr/bin/false\n'
        printf 'clang_sha256\t%s\n' "$zero"
        printf 'controller_sha256\t%s\n' "$zero"
        printf 'probe_sha256\t%s\n' "$zero"
        printf 'toolchain_fingerprint\t%s\n' "$zero"
        printf 'repository_head\t%s\n' "$zero"
        printf 'repository_dirty\tYES\n'
        printf 'cases_source_sha256\t%s\n' "$(sha256_file "$case_manifest")"
        printf 'controller_source_sha256\t%s\n' "$(sha256_file "$script_root/controller.c")"
        printf 'probes_source_sha256\t%s\n' "$(sha256_file "$script_root/probes.c")"
        printf 'run_source_sha256\t%s\n' "$(sha256_file "$script_root/run.sh")"
        printf 'verifier_source_sha256\t%s\n' "$(sha256_file "$script_root/verify-evidence.awk")"
        printf 'self_test_source_sha256\t%s\n' "$(sha256_file "$script_root/self-test.sh")"
        printf 'readme_source_sha256\t%s\n' "$(sha256_file "$script_root/README.md")"
        printf 'build_controller_argv\tSYNTHETIC\n'
        printf 'build_probe_argv\tSYNTHETIC\n'
        printf 'work_root\tSYNTHETIC\n'
    } > "$directory/metadata.tsv"
}

write_probe_fixture() {
    printf 'origin=SYNTHETIC\n'
    case "$1" in
        R0A_BASELINE) printf 'as_soft=999999999999\nas_hard=999999999999\nfsize_soft=999999999\nfsize_hard=999999999\nnofile_soft=1024\nnofile_hard=1024\npid=1\nvm_regions=1\nvirtual_bytes=4096\nvm_complete=1\n' ;;
        R0A_ALLOC_CONTROL) printf 'malloc_success=1\nmalloc_errno=0\nmmap_success=1\nmmap_errno=0\n' ;;
        R0A_SMALL_CONTROL|R0A_AS_SMALL) printf 'target=500528857088\nmalloc_success=1\nerrno=0\ntouched_bytes=8388608\n' ;;
        R0A_AS_MALLOC|R0A_AS_MMAP) printf 'target=500528857088\nallocation_success=0\nerrno=12\n' ;;
        R0A_AS_EXEC) printf 'expected_target=500528857088\ninherited_soft=500528857088\ninherited_hard=500528857088\nraise_result=-1\nraise_errno=1\nallocation_success=0\nallocation_errno=12\n' ;;
        R0A_AS_FIXED) printf 'target=2147483648\nset_result=-1\nerrno=22\n' ;;
        R0A_VM_REGIONS) printf 'regions=1\nvirtual_bytes=4096\ncomplete=1\nprotection_0_bytes=4096\nprotection_1_bytes=0\nprotection_2_bytes=0\nprotection_3_bytes=0\nprotection_4_bytes=0\nprotection_5_bytes=0\nprotection_6_bytes=0\nprotection_7_bytes=0\n' ;;
        R0A_FSIZE) printf 'first_bytes=65536\nfirst_errno=27\nsecond_bytes=65536\nsecond_errno=27\naggregate_bytes=131072\nsaw_sigxfsz=1\ncontroller_first_bytes=65536\ncontroller_second_bytes=65536\n' ;;
        R0A_NOFILE) printf 'initially_open=3\nsuccessful_dups=29\nfinal_errno=24\n' ;;
        R0A_DEADLINE) printf 'deadline_probe_started=1\n'; return ;;
    esac
    printf 'probe_result=EXPECTED\n'
}

make_valid_fixture() {
    directory=$1
    cp "$case_manifest" "$directory/cases.tsv"
    write_metadata "$directory"
    printf '%s\n' 'case_id	attempt	result	exit_code	signal	timed_out	cleanup	stdout_sha256	stderr_sha256	stdout_bytes	stderr_bytes' > "$directory/summary.tsv"
    empty_digest=$(printf '' | /usr/bin/shasum -a 256 | /usr/bin/awk '{print $1}')
    /usr/bin/awk 'NR > 1 { print $1 }' "$case_manifest" |
    while read -r case_id; do
        attempt=1
        while [ "$attempt" -le 3 ]; do
            stdout_file="$directory/case-$case_id-$attempt.stdout"
            stderr_file="$directory/case-$case_id-$attempt.stderr"
            write_probe_fixture "$case_id" > "$stdout_file"
            : > "$stderr_file"
            output_digest=$(sha256_file "$stdout_file")
            output_bytes=$(wc -c < "$stdout_file" | tr -d ' ')
            if [ "$case_id" = R0A_DEADLINE ]; then
                exit_code=-1
                signal_number=15
                timed_out=1
            else
                exit_code=0
                signal_number=0
                timed_out=0
            fi
            printf '%s\t%s\tOBSERVED_EXPECTED\t%s\t%s\t%s\tCONFIRMED\t%s\t%s\t%s\t0\n' \
                "$case_id" "$attempt" "$exit_code" "$signal_number" "$timed_out" \
                "$output_digest" "$empty_digest" "$output_bytes" >> "$directory/summary.tsv"
            attempt=$((attempt + 1))
        done
    done
    chmod 400 "$directory"/*
}

clone_fixture() {
    source_directory=$1
    destination=$(new_root)
    cp -R "$source_directory/." "$destination/"
    chmod 700 "$destination"
    chmod 600 "$destination"/*
    printf '%s\n' "$destination"
}

expect_failure() {
    label=$1
    directory=$2
    expected_message=$3
    if "$runner" verify "$directory" > "$self_test_root/verify.log" 2>&1; then
        printf 'self-test unexpectedly accepted %s\n' "$label" >&2
        exit 1
    fi
    grep -F "$expected_message" "$self_test_root/verify.log" >/dev/null || {
        cat "$self_test_root/verify.log" >&2
        printf 'wrong rejection reason: %s\n' "$label" >&2
        exit 1
    }
}

replace_summary_row() {
    directory=$1
    case_id=$2
    attempt=$3
    replacement=$4
    /usr/bin/awk -F '\t' -v OFS='\t' -v case_id="$case_id" -v attempt="$attempt" \
        -v replacement="$replacement" '
        $1 == case_id && $2 == attempt { print replacement; next }
        { print }
    ' "$directory/summary.tsv" > "$directory/summary.new"
    mv "$directory/summary.new" "$directory/summary.tsv"
}

base=$(new_root)
make_valid_fixture "$base"
"$runner" verify "$base" >/dev/null

mutant=$(clone_fixture "$base")
/usr/bin/awk 'NR != 2' "$mutant/summary.tsv" > "$mutant/summary.new"
mv "$mutant/summary.new" "$mutant/summary.tsv"
expect_failure missing-attempt "$mutant" 'missing attempt'

mutant=$(clone_fixture "$base")
sed -n '2p' "$mutant/summary.tsv" >> "$mutant/summary.tsv"
expect_failure duplicate-attempt "$mutant" 'duplicate case attempt'

mutant=$(clone_fixture "$base")
/usr/bin/awk -F '\t' -v OFS='\t' '$1 == "cases_source_sha256" { $2="ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff" } { print }' \
    "$mutant/metadata.tsv" > "$mutant/metadata.new"
mv "$mutant/metadata.new" "$mutant/metadata.tsv"
expect_failure forged-source-hash "$mutant" 'cases source digest differs'

mutant=$(clone_fixture "$base")
/bin/echo x > "$mutant/case-R0A_BASELINE-1.stdout"
expect_failure truncated-output "$mutant" 'stdout byte count differs'

mutant=$(clone_fixture "$base")
/bin/dd if=/dev/zero of="$mutant/case-R0A_BASELINE-1.stdout" bs=32769 count=1 2>/dev/null
oversized_digest=$(sha256_file "$mutant/case-R0A_BASELINE-1.stdout")
awk -F '\t' -v OFS='\t' -v digest="$oversized_digest" \
    '$1 == "R0A_BASELINE" && $2 == 1 { $8=digest; $10=32769 } { print }' \
    "$mutant/summary.tsv" > "$mutant/summary.new"
mv "$mutant/summary.new" "$mutant/summary.tsv"
expect_failure oversized-output "$mutant" 'invalid output byte count'

mutant=$(clone_fixture "$base")
ln -s /tmp "$mutant/escape"
expect_failure symlink "$mutant" 'symlink or non-regular'

mutant=$(clone_fixture "$base")
/usr/bin/awk -F '\t' -v OFS='\t' '$1 == "evidence_kind" { $2="REAL" } { print }' \
    "$mutant/metadata.tsv" > "$mutant/metadata.new"
mv "$mutant/metadata.new" "$mutant/metadata.tsv"
printf 'SYNTHETIC\n' > "$mutant/case-R0A_BASELINE-1.stdout"
new_digest=$(sha256_file "$mutant/case-R0A_BASELINE-1.stdout")
replace_summary_row "$mutant" R0A_BASELINE 1 "R0A_BASELINE\t1\tOBSERVED_EXPECTED\t0\t0\t0\tCONFIRMED\t$new_digest\t$(printf '' | /usr/bin/shasum -a 256 | /usr/bin/awk '{print $1}')\t10\t0"
expect_failure synthetic-mixing "$mutant" 'synthetic output is mixed'

mutant=$(clone_fixture "$base")
line=$(/usr/bin/awk -F '\t' -v OFS='\t' 'NR == 2 { $4=1; print }' "$mutant/summary.tsv")
replace_summary_row "$mutant" R0A_BASELINE 1 "$line"
expect_failure nonzero-masquerade "$mutant" 'normal observation does not have a clean exit'

mutant=$(clone_fixture "$base")
line=$(/usr/bin/awk -F '\t' -v OFS='\t' 'NR == 2 { $5=9; print }' "$mutant/summary.tsv")
replace_summary_row "$mutant" R0A_BASELINE 1 "$line"
expect_failure signal-masquerade "$mutant" 'normal observation does not have a clean exit'

mutant=$(clone_fixture "$base")
printf 'exec_error=2\n' > "$mutant/case-R0A_AS_EXEC-1.stdout"
new_digest=$(sha256_file "$mutant/case-R0A_AS_EXEC-1.stdout")
empty_digest=$(printf '' | /usr/bin/shasum -a 256 | /usr/bin/awk '{print $1}')
replace_summary_row "$mutant" R0A_AS_EXEC 1 "R0A_AS_EXEC\t1\tOBSERVED_EXPECTED\t0\t0\t0\tCONFIRMED\t$new_digest\t$empty_digest\t13\t0"
expect_failure exec-failure-masquerade "$mutant" 'exec failure is presented'

mutant=$(clone_fixture "$base")
line=$(/usr/bin/awk -F '\t' -v OFS='\t' 'NR == 2 { $7="UNCONFIRMED"; print }' "$mutant/summary.tsv")
replace_summary_row "$mutant" R0A_BASELINE 1 "$line"
expect_failure cleanup-unconfirmed "$mutant" 'cleanup is not confirmed'

refresh_output_index() {
    directory=$1
    case_id=$2
    output="$directory/case-$case_id-1.stdout"
    digest=$(sha256_file "$output")
    bytes=$(wc -c < "$output" | tr -d ' ')
    awk -F '\t' -v OFS='\t' -v key="$case_id" -v digest="$digest" -v bytes="$bytes" \
        '$1 == key && $2 == 1 { $8=digest; $10=bytes } { print }' "$directory/summary.tsv" > "$directory/summary.new"
    mv "$directory/summary.new" "$directory/summary.tsv"
}

mutant=$(clone_fixture "$base")
sed 's/probe_result=EXPECTED/probe_result=COUNTEREXAMPLE/; s/allocation_success=0/allocation_success=1/; s/errno=12/errno=0/' \
    "$mutant/case-R0A_AS_MALLOC-1.stdout" > "$mutant/output.new"
mv "$mutant/output.new" "$mutant/case-R0A_AS_MALLOC-1.stdout"
refresh_output_index "$mutant" R0A_AS_MALLOC
expect_failure contradictory-result "$mutant" 'probe result contradicts summary'

mutant=$(clone_fixture "$base")
sed 's/errno=12/errno=22/' "$mutant/case-R0A_AS_MALLOC-1.stdout" > "$mutant/output.new"
mv "$mutant/output.new" "$mutant/case-R0A_AS_MALLOC-1.stdout"
refresh_output_index "$mutant" R0A_AS_MALLOC
expect_failure wrong-errno "$mutant" 'allocation refusal is not ENOMEM'

mutant=$(clone_fixture "$base")
sed 's/controller_first_bytes=65536/controller_first_bytes=0/' "$mutant/case-R0A_FSIZE-1.stdout" > "$mutant/output.new"
mv "$mutant/output.new" "$mutant/case-R0A_FSIZE-1.stdout"
refresh_output_index "$mutant" R0A_FSIZE
expect_failure independent-size "$mutant" 'controller file size contradicts probe'

mutant=$(clone_fixture "$base")
printf 'probe_result=EXPECTED\n' > "$mutant/case-R0A_AS_MALLOC-1.stdout"
refresh_output_index "$mutant" R0A_AS_MALLOC
expect_failure missing-fields "$mutant" 'missing or invalid observation'

mutant=$(clone_fixture "$base")
awk -F '\t' -v OFS='\t' '$1 == "probes_source_sha256" { $2="ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff" } { print }' \
    "$mutant/metadata.tsv" > "$mutant/metadata.new"
mv "$mutant/metadata.new" "$mutant/metadata.tsv"
expect_failure stale-probe-source "$mutant" 'probe source digest differs'

mutant=$(clone_fixture "$base")
awk -F '\t' -v OFS='\t' '$1 == "R0A_ALLOC_CONTROL" && $2 == 1 { $3="INCONCLUSIVE" } { print }' \
    "$mutant/summary.tsv" > "$mutant/summary.new"
mv "$mutant/summary.new" "$mutant/summary.tsv"
expect_failure missing-positive-control "$mutant" 'AS observation lacks successful paired controls'

# A consistent counterexample is useful complete evidence, not a verifier error.
mutant=$(clone_fixture "$base")
for attempt in 1 2 3; do
    sed 's/probe_result=EXPECTED/probe_result=COUNTEREXAMPLE/; s/allocation_success=0/allocation_success=1/; s/errno=12/errno=0/' \
        "$mutant/case-R0A_AS_MALLOC-$attempt.stdout" > "$mutant/output.new"
    mv "$mutant/output.new" "$mutant/case-R0A_AS_MALLOC-$attempt.stdout"
    digest=$(sha256_file "$mutant/case-R0A_AS_MALLOC-$attempt.stdout")
    bytes=$(wc -c < "$mutant/case-R0A_AS_MALLOC-$attempt.stdout" | tr -d ' ')
    awk -F '\t' -v OFS='\t' -v attempt="$attempt" -v digest="$digest" -v bytes="$bytes" \
        '$1 == "R0A_AS_MALLOC" && $2 == attempt { $3="OBSERVED_COUNTEREXAMPLE"; $8=digest; $10=bytes } { print }' \
        "$mutant/summary.tsv" > "$mutant/summary.new"
    mv "$mutant/summary.new" "$mutant/summary.tsv"
done
"$runner" verify "$mutant" > "$self_test_root/counterexample.log"
grep -F 'counterexamples=3' "$self_test_root/counterexample.log" >/dev/null

# Mixed observations remain unresolved rather than selecting the favorable run.
cp "$base/case-R0A_AS_MALLOC-1.stdout" "$mutant/case-R0A_AS_MALLOC-1.stdout"
refresh_output_index "$mutant" R0A_AS_MALLOC
awk -F '\t' -v OFS='\t' '$1 == "R0A_AS_MALLOC" && $2 == 1 { $3="OBSERVED_EXPECTED" } { print }' \
    "$mutant/summary.tsv" > "$mutant/summary.new"
mv "$mutant/summary.new" "$mutant/summary.tsv"
if "$runner" verify "$mutant" > "$self_test_root/unstable.log"; then exit 1; else [ "$?" -eq 2 ] || exit 1; fi
grep -F 'unstable_cases=1' "$self_test_root/unstable.log" >/dev/null

clang=$(/usr/bin/xcrun --sdk macosx --find clang)
sdk=$(/usr/bin/xcrun --sdk macosx --show-sdk-path)
for unit in controller probes; do
    compile_with_deadline /usr/bin/env -i LANG=C LC_ALL=C "$clang" -std=c11 -O0 -Wall -Wextra -Werror \
        -DR0_SELF_TEST -isysroot "$sdk" "$script_root/$unit.c" -o "$self_test_root/$unit"
    "$self_test_root/$unit" --self-test
done
if "$self_test_root/controller" /no/probe R0A_BASELINE 1 /no/evidence /no/work 0; then
    echo 'expired batch unexpectedly admitted' >&2; exit 1
else
    [ "$?" -eq 124 ] || exit 1
fi
runtime_evidence=$(new_root)
runtime_work=$(new_root)
deadline=$("$self_test_root/controller" --clock | awk '{printf "%.6f", $1+0.2}')
if "$self_test_root/controller" "$self_test_root/probes" R0A_DEADLINE 1 "$runtime_evidence" "$runtime_work" "$deadline" > "$self_test_root/batch.log"; then
    echo 'in-flight batch expiry unexpectedly succeeded' >&2; exit 1
else
    [ "$?" -eq 124 ] || exit 1
fi
grep -Fx 'cleanup=CONFIRMED' "$self_test_root/batch.log" >/dev/null
grep -Fx 'timed_out=1' "$self_test_root/batch.log" >/dev/null

runtime_evidence=$(new_root)
deadline=$("$self_test_root/controller" --clock | awk '{printf "%.6f", $1+10}')
"$self_test_root/controller" "$self_test_root/probes" R0A_DEADLINE 1 "$runtime_evidence" "$runtime_work" "$deadline" > "$self_test_root/cancel.log" &
owned_controller=$!
sleep 0.1
kill -TERM "$owned_controller"
if wait "$owned_controller"; then exit 1; else [ "$?" -eq 130 ] || exit 1; fi
grep -Fx 'cleanup=CONFIRMED' "$self_test_root/cancel.log" >/dev/null
grep -Fx 'cancelled=1' "$self_test_root/cancel.log" >/dev/null
echo 'R0A_SELF_TEST_OK negatives=17 evidence_outcomes=2 controller_cases=8 probe_assertions=3'
cleanup
[ ! -e "$self_test_root" ] || exit 1
trap - EXIT
echo 'R0A_SELF_TEST_CLEANUP_CONFIRMED'
