#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
script_root="$repository_root/scripts/native-research"
case_manifest="$script_root/cases.tsv"
active_pid=''
evidence=''
build=''
work=''

cancel_run() {
    trap '' INT TERM HUP
    if [ -n "$active_pid" ]; then
        kill -TERM "$active_pid" 2>/dev/null || true
        wait "$active_pid" 2>/dev/null || true
        active_pid=''
    fi
    printf 'R0A_CANCELLED evidence=%s build=%s work=%s\n' "$evidence" "$build" "$work" >&2
    exit 130
}

fail() {
    printf 'R0A_ERROR: %s\n' "$1" >&2
    exit 1
}

usage() {
    echo 'usage: scripts/native-research/run.sh self-test|observe|run-a|verify EVIDENCE_DIR' >&2
    exit 64
}

sha256_file() {
    /usr/bin/shasum -a 256 "$1" | /usr/bin/awk '{print $1}'
}

metadata_value() {
    metadata_file=$1
    metadata_key=$2
    /usr/bin/awk -F '\t' -v key="$metadata_key" '
        $1 == key { if (++count != 1) exit 2; value=$2 }
        END { if (count != 1) exit 2; print value }
    ' "$metadata_file"
}

reject_ambient_build_overrides() {
    ambient=$(/usr/bin/env | /usr/bin/awk -F= '
        $1 ~ /^(CC|CFLAGS|CPPFLAGS|LDFLAGS|SDKROOT|MACOSX_DEPLOYMENT_TARGET)$/ ||
        $1 ~ /^DYLD_/ { print $1; exit }
    ')
    [ -z "$ambient" ] || fail "ambient build override is set: $ambient"
}

compile_with_deadline() {
    "$@" &
    compiler_pid=$!
    active_pid=$compiler_pid
    elapsed=0
    while kill -0 "$compiler_pid" 2>/dev/null; do
        if [ "$elapsed" -ge 120 ]; then
            kill -TERM "$compiler_pid" 2>/dev/null || true
            /bin/sleep 1
            kill -KILL "$compiler_pid" 2>/dev/null || true
            wait "$compiler_pid" 2>/dev/null || true
            fail 'trusted compiler exceeded the 120-second research budget'
        fi
        /bin/sleep 1
        elapsed=$((elapsed + 1))
    done
    wait "$compiler_pid"
    active_pid=''
}

observe() {
    cd "$repository_root"
    ./scripts/dev-toolchain.sh inspect
}

verify_metadata() {
    evidence=$1
    metadata="$evidence/metadata.tsv"
    [ -f "$metadata" ] && [ ! -L "$metadata" ] || fail 'metadata.tsv must be a regular file'
    [ "$(wc -c < "$metadata" | tr -d ' ')" -le 524288 ] || fail 'metadata exceeds 512 KiB'
    required='schema evidence_kind budget_version created_utc os_product os_version os_build arch xcode_version xcode_build sdk_version clang_path clang_sha256 controller_sha256 probe_sha256 toolchain_fingerprint repository_head repository_dirty cases_source_sha256 controller_source_sha256 probes_source_sha256 run_source_sha256 verifier_source_sha256 self_test_source_sha256 readme_source_sha256 build_controller_argv build_probe_argv work_root'
    /usr/bin/awk -F '\t' -v required="$required" '
        BEGIN {
            count=split(required, names, " ")
            for (i=1; i<=count; i++) allowed[names[i]]=1
        }
        NF != 2 || !($1 in allowed) || seen[$1]++ { bad=1 }
        length($1) > 64 || length($2) > 4096 || $2 ~ /[\r\n]/ { bad=1 }
        END {
            for (name in allowed) if (seen[name] != 1) bad=1
            exit bad ? 1 : 0
        }
    ' "$metadata" || fail 'metadata keys are incomplete, duplicated, or malformed'
    [ "$(metadata_value "$metadata" schema)" = 2 ] || fail 'unsupported metadata schema'
    kind=$(metadata_value "$metadata" evidence_kind)
    case "$kind" in REAL|SYNTHETIC) ;; *) fail 'unknown evidence kind' ;; esac
    [ "$(metadata_value "$metadata" budget_version)" = R0A-2 ] || fail 'unknown budget version'
    for digest_key in clang_sha256 controller_sha256 probe_sha256 cases_source_sha256 controller_source_sha256 probes_source_sha256 run_source_sha256 verifier_source_sha256 self_test_source_sha256 readme_source_sha256; do
        digest=$(metadata_value "$metadata" "$digest_key")
        [ "${#digest}" -eq 64 ] && ! printf '%s' "$digest" | /usr/bin/grep -q '[^0-9a-f]' \
            || fail "invalid digest: $digest_key"
    done
    [ "$(metadata_value "$metadata" cases_source_sha256)" = "$(sha256_file "$case_manifest")" ] \
        || fail 'cases source digest differs from repository input'
    [ "$(metadata_value "$metadata" controller_source_sha256)" = "$(sha256_file "$script_root/controller.c")" ] \
        || fail 'controller source digest differs from repository input'
    [ "$(metadata_value "$metadata" probes_source_sha256)" = "$(sha256_file "$script_root/probes.c")" ] \
        || fail 'probe source digest differs from repository input'
    [ "$(metadata_value "$metadata" run_source_sha256)" = "$(sha256_file "$script_root/run.sh")" ] \
        || fail 'runner source digest differs from repository input'
    [ "$(metadata_value "$metadata" verifier_source_sha256)" = "$(sha256_file "$script_root/verify-evidence.awk")" ] \
        || fail 'verifier source digest differs from repository input'
    [ "$(metadata_value "$metadata" self_test_source_sha256)" = "$(sha256_file "$script_root/self-test.sh")" ] \
        || fail 'self-test source digest differs from repository input'
    [ "$(metadata_value "$metadata" readme_source_sha256)" = "$(sha256_file "$script_root/README.md")" ] \
        || fail 'readme source digest differs from repository input'
}

verify_evidence() {
    [ "$#" -eq 1 ] || usage
    evidence_input=$1
    [ -d "$evidence_input" ] && [ ! -L "$evidence_input" ] || fail 'evidence path must be a directory'
    evidence=$(CDPATH= cd -- "$evidence_input" && pwd -P)
    case "$evidence" in "$repository_root"/target/native-research/evidence.*|/private/tmp/mengxia-r0a-self-test.*|/tmp/mengxia-r0a-self-test.*) ;; *) fail 'evidence path is outside an allowed owned root' ;; esac
    owner=$(/usr/bin/stat -f '%u' "$evidence")
    mode=$(/usr/bin/stat -f '%Lp' "$evidence")
    [ "$owner" -eq "$(/usr/bin/id -u)" ] || fail 'evidence owner differs from current user'
    [ $((0$mode & 077)) -eq 0 ] || fail 'evidence directory is not owner-only'
    if find "$evidence" -mindepth 1 \( -type l -o ! -type f \) -print -quit | /usr/bin/grep . >/dev/null; then
        fail 'evidence contains a symlink or non-regular entry'
    fi
    total_bytes=$(find "$evidence" -type f -exec /usr/bin/stat -f '%z' {} + | /usr/bin/awk '{ total += $1 } END { print total + 0 }')
    [ "$total_bytes" -le 4194304 ] || fail 'evidence exceeds 4 MiB'
    verify_metadata "$evidence"
    [ -f "$evidence/cases.tsv" ] && [ -f "$evidence/summary.tsv" ] || fail 'evidence index is incomplete'
    cmp -s "$case_manifest" "$evidence/cases.tsv" || fail 'evidence cases differ from reviewed manifest'
    /usr/bin/awk -f "$script_root/verify-evidence.awk" "$evidence/cases.tsv" "$evidence/summary.tsv" \
        || fail 'evidence summary is invalid'

    kind=$(metadata_value "$evidence/metadata.tsv" evidence_kind)
    expected_files=$(/usr/bin/awk 'NR > 1 { count += 6 } END { print count + 3 }' "$case_manifest")
    actual_files=$(find "$evidence" -type f | wc -l | tr -d ' ')
    [ "$actual_files" -eq "$expected_files" ] || fail 'evidence has missing or extra files'
    /usr/bin/awk -F '\t' 'NR > 1 { print $1, $2, $3, $8, $9, $10, $11, $4, $5 }' "$evidence/summary.tsv" |
    while read -r case_id attempt result stdout_digest stderr_digest stdout_bytes stderr_bytes exit_code signal_number; do
        stdout_file="$evidence/case-$case_id-$attempt.stdout"
        stderr_file="$evidence/case-$case_id-$attempt.stderr"
        [ -f "$stdout_file" ] && [ -f "$stderr_file" ] || fail 'case output is missing'
        [ "$(wc -c < "$stdout_file" | tr -d ' ')" -eq "$stdout_bytes" ] || fail 'stdout byte count differs'
        [ "$(wc -c < "$stderr_file" | tr -d ' ')" -eq "$stderr_bytes" ] || fail 'stderr byte count differs'
        [ "$(sha256_file "$stdout_file")" = "$stdout_digest" ] || fail 'stdout digest differs'
        [ "$(sha256_file "$stderr_file")" = "$stderr_digest" ] || fail 'stderr digest differs'
        if [ "$kind" = REAL ] && /usr/bin/grep -q 'SYNTHETIC' "$stdout_file" "$stderr_file"; then
            fail 'synthetic output is mixed into real evidence'
        fi
        if [ "$case_id" = R0A_AS_EXEC ] && [ "$result" = OBSERVED_EXPECTED ] && \
            /usr/bin/grep -q '^exec_error=' "$stdout_file"; then
            fail 'exec failure is presented as an expected inheritance observation'
        fi
        if [ "$exit_code" -eq 0 ] && [ "$signal_number" -eq 0 ] || [ "$case_id" = R0A_DEADLINE ]; then
            /usr/bin/awk -v mode=probe -v case_id="$case_id" -v result="$result" \
                -f "$script_root/verify-evidence.awk" "$stdout_file" || fail 'probe observation contradicts summary'
        elif [ "$result" != INCONCLUSIVE ] && [ "$result" != NOT_RUN ]; then
            fail 'abnormal termination cannot establish an observation'
        fi
    done
    unresolved=$(/usr/bin/awk -F '\t' 'NR > 1 && ($3 == "INCONCLUSIVE" || $3 == "NOT_RUN") { count++ } END { print count + 0 }' "$evidence/summary.tsv")
    unstable=$(/usr/bin/awk -F '\t' 'NR > 1 { if ($1 in first && first[$1] != $3) mixed[$1]=1; first[$1]=$3 } END { for (key in mixed) count++; print count+0 }' "$evidence/summary.tsv")
    printf 'R0A_EVIDENCE_VALID kind=%s directory=%s bytes=%s unresolved=%s unstable_cases=%s\n' "$kind" "$evidence" "$total_bytes" "$unresolved" "$unstable"
    [ "$unresolved" -eq 0 ] && [ "$unstable" -eq 0 ] || exit 2
}

status_value() {
    status_text=$1
    status_key=$2
    printf '%s\n' "$status_text" | /usr/bin/awk -F= -v key="$status_key" '
        $1 == key { if (++count != 1) exit 2; value=$2 }
        END { if (count != 1) exit 2; print value }
    '
}

run_a() {
    [ "$#" -eq 0 ] || usage
    reject_ambient_build_overrides
    trap cancel_run INT TERM HUP
    umask 077
    base="$repository_root/target/native-research"
    /bin/mkdir -p "$base"
    chmod 700 "$base"
    evidence=$(/usr/bin/mktemp -d "$base/evidence.XXXXXXXX")
    build=$(/usr/bin/mktemp -d "$base/build.XXXXXXXX")
    work=$(/usr/bin/mktemp -d "$base/work.XXXXXXXX")
    chmod 700 "$evidence" "$build" "$work"

    original_script_root=$script_root
    snapshot="$build/source"
    mkdir "$snapshot"
    for source in README.md cases.tsv controller.c probes.c run.sh verify-evidence.awk self-test.sh; do
        cp "$script_root/$source" "$snapshot/$source"
        cmp -s "$script_root/$source" "$snapshot/$source" || fail 'source changed during snapshot'
    done
    chmod 400 "$snapshot"/*
    script_root=$snapshot
    case_manifest="$snapshot/cases.tsv"

    inspection=$(cd "$repository_root" && ./scripts/dev-toolchain.sh inspect)
    fingerprint=$(printf '%s\n' "$inspection" | /usr/bin/awk -F= '$1 == "environment_fingerprint" { print $2 }')
    [ "${#fingerprint}" -eq 64 ] || fail 'toolchain fingerprint unavailable'
    clang=$(/usr/bin/xcrun --no-cache --sdk macosx --find clang)
    sdk=$(/usr/bin/xcrun --no-cache --sdk macosx --show-sdk-path)
    [ "${clang#/}" != "$clang" ] && [ "${sdk#/}" != "$sdk" ] || fail 'toolchain paths are not absolute'
    controller="$build/controller"
    probe="$build/probes"
    controller_argv="$clang -std=c11 -O0 -g -Wall -Wextra -Werror -isysroot $sdk $script_root/controller.c -o $controller"
    probe_argv="$clang -std=c11 -O0 -g -Wall -Wextra -Werror -isysroot $sdk $script_root/probes.c -o $probe"
    compile_with_deadline /usr/bin/env -i LANG=C LC_ALL=C TZ=UTC SDKROOT="$sdk" "$clang" \
        -std=c11 -O0 -g -Wall -Wextra -Werror -isysroot "$sdk" \
        "$script_root/controller.c" -o "$controller"
    compile_with_deadline /usr/bin/env -i LANG=C LC_ALL=C TZ=UTC SDKROOT="$sdk" "$clang" \
        -std=c11 -O0 -g -Wall -Wextra -Werror -isysroot "$sdk" \
        "$script_root/probes.c" -o "$probe"
    chmod 500 "$controller" "$probe"

    cp "$case_manifest" "$evidence/cases.tsv"
    printf '%s\n' 'case_id	attempt	result	exit_code	signal	timed_out	cleanup	stdout_sha256	stderr_sha256	stdout_bytes	stderr_bytes' > "$evidence/summary.tsv"

    batch_deadline=$("$controller" --clock | /usr/bin/awk '{ printf "%.6f", $1 + 600 }')
    check_batch_deadline() {
        "$controller" --clock | /usr/bin/awk -v deadline="$batch_deadline" '$1 < deadline { ok=1 } END { exit !ok }' \
            || fail '600-second batch deadline exceeded; batch stopped'
    }
    for case_id in $(/usr/bin/awk 'NR > 1 { print $1 }' "$case_manifest"); do
        attempt=1
        while [ "$attempt" -le 3 ]; do
            case_work="$work/$case_id-$attempt"
            /bin/mkdir "$case_work"
            check_batch_deadline
            "$controller" "$probe" "$case_id" "$attempt" "$evidence" "$case_work" "$batch_deadline" > "$case_work/controller.status" &
            active_pid=$!
            if wait "$active_pid"; then
                controller_rc=0
            else
                controller_rc=$?
            fi
            active_pid=''
            controller_status=$(cat "$case_work/controller.status")
            check_batch_deadline
            exit_code=$(status_value "$controller_status" exit_code) || fail 'controller status lacks exit code'
            signal_number=$(status_value "$controller_status" signal) || fail 'controller status lacks signal'
            timed_out=$(status_value "$controller_status" timed_out) || fail 'controller status lacks timeout state'
            cleanup=$(status_value "$controller_status" cleanup) || fail 'controller status lacks cleanup state'
            stdout_bytes=$(status_value "$controller_status" stdout_bytes) || fail 'controller status lacks stdout count'
            stderr_bytes=$(status_value "$controller_status" stderr_bytes) || fail 'controller status lacks stderr count'
            [ "$cleanup" = CONFIRMED ] || fail 'probe cleanup is unconfirmed; batch stopped'
            stdout_file="$evidence/case-$case_id-$attempt.stdout"
            stderr_file="$evidence/case-$case_id-$attempt.stderr"
            if [ "$case_id" = R0A_FSIZE ] && [ "$exit_code" -eq 0 ]; then
                first_size=$(status_value "$controller_status" fsize_1)
                second_size=$(status_value "$controller_status" fsize_2)
                printf 'controller_first_bytes=%s\ncontroller_second_bytes=%s\n' "$first_size" "$second_size" >> "$stdout_file"
                stdout_bytes=$(wc -c < "$stdout_file" | tr -d ' ')
            fi
            [ "$controller_rc" -ne 130 ] || cancel_run
            [ "$controller_rc" -ne 74 ] || fail 'controller output or IO failed'
            if [ "$case_id" = R0A_DEADLINE ] && [ "$controller_rc" -eq 0 ] && [ "$timed_out" -eq 1 ]; then
                result=OBSERVED_EXPECTED
            elif [ "$controller_rc" -eq 0 ] && [ "$exit_code" -eq 0 ] && [ "$signal_number" -eq 0 ]; then
                probe_result=$(/usr/bin/awk -F= '$1 == "probe_result" { if (++count != 1) exit 2; value=$2 } END { if (count != 1) exit 2; print value }' "$stdout_file") \
                    || fail 'probe emitted no unique result'
                case "$probe_result" in
                    EXPECTED) result=OBSERVED_EXPECTED ;;
                    COUNTEREXAMPLE) result=OBSERVED_COUNTEREXAMPLE ;;
                    INCONCLUSIVE) result=INCONCLUSIVE ;;
                    *) fail 'probe emitted an unknown result' ;;
                esac
            else
                result=INCONCLUSIVE
            fi
            stdout_digest=$(sha256_file "$stdout_file")
            stderr_digest=$(sha256_file "$stderr_file")
            printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
                "$case_id" "$attempt" "$result" "$exit_code" "$signal_number" \
                "$timed_out" "$cleanup" "$stdout_digest" "$stderr_digest" \
                "$stdout_bytes" "$stderr_bytes" >> "$evidence/summary.tsv"
            attempt=$((attempt + 1))
        done
    done
    check_batch_deadline
    for source in README.md cases.tsv controller.c probes.c run.sh verify-evidence.awk self-test.sh; do
        cmp -s "$original_script_root/$source" "$snapshot/$source" || fail 'source changed since compilation; evidence not finalized'
    done

    xcode_lines=$(/usr/bin/xcodebuild -version)
    xcode_version=$(printf '%s\n' "$xcode_lines" | /usr/bin/awk 'NR == 1 { print $2 }')
    xcode_build=$(printf '%s\n' "$xcode_lines" | /usr/bin/awk 'NR == 2 { print $3 }')
    repository_head=$(git -C "$repository_root" rev-parse HEAD)
    if git -C "$repository_root" diff --quiet --ignore-submodules -- && \
       git -C "$repository_root" diff --cached --quiet --ignore-submodules -- && \
       [ -z "$(git -C "$repository_root" ls-files --others --exclude-standard)" ]; then
        repository_dirty=NO
    else
        repository_dirty=YES
    fi
    {
        printf 'schema\t2\n'
        printf 'evidence_kind\tREAL\n'
        printf 'budget_version\tR0A-2\n'
        printf 'created_utc\t%s\n' "$(/bin/date -u '+%Y-%m-%dT%H:%M:%SZ')"
        printf 'os_product\t%s\n' "$(/usr/bin/sw_vers -productName)"
        printf 'os_version\t%s\n' "$(/usr/bin/sw_vers -productVersion)"
        printf 'os_build\t%s\n' "$(/usr/bin/sw_vers -buildVersion)"
        printf 'arch\t%s\n' "$(/usr/bin/uname -m)"
        printf 'xcode_version\t%s\n' "$xcode_version"
        printf 'xcode_build\t%s\n' "$xcode_build"
        printf 'sdk_version\t%s\n' "$(/usr/bin/xcrun --no-cache --sdk macosx --show-sdk-version)"
        printf 'clang_path\t%s\n' "$clang"
        printf 'clang_sha256\t%s\n' "$(sha256_file "$clang")"
        printf 'controller_sha256\t%s\n' "$(sha256_file "$controller")"
        printf 'probe_sha256\t%s\n' "$(sha256_file "$probe")"
        printf 'toolchain_fingerprint\t%s\n' "$fingerprint"
        printf 'repository_head\t%s\n' "$repository_head"
        printf 'repository_dirty\t%s\n' "$repository_dirty"
        printf 'cases_source_sha256\t%s\n' "$(sha256_file "$case_manifest")"
        printf 'controller_source_sha256\t%s\n' "$(sha256_file "$script_root/controller.c")"
        printf 'probes_source_sha256\t%s\n' "$(sha256_file "$script_root/probes.c")"
        printf 'run_source_sha256\t%s\n' "$(sha256_file "$script_root/run.sh")"
        printf 'verifier_source_sha256\t%s\n' "$(sha256_file "$script_root/verify-evidence.awk")"
        printf 'self_test_source_sha256\t%s\n' "$(sha256_file "$script_root/self-test.sh")"
        printf 'readme_source_sha256\t%s\n' "$(sha256_file "$script_root/README.md")"
        printf 'build_controller_argv\t%s\n' "$controller_argv"
        printf 'build_probe_argv\t%s\n' "$probe_argv"
        printf 'work_root\t%s\n' "$work"
    } > "$evidence/metadata.tsv"
    chmod 400 "$evidence"/*
    check_batch_deadline
    if verify_evidence "$evidence"; then
        verification_rc=0
    else
        verification_rc=$?
    fi
    check_batch_deadline
    printf 'R0A_EVIDENCE_DIRECTORY=%s\nR0A_BUILD_DIRECTORY=%s\nR0A_WORK_DIRECTORY=%s\n' \
        "$evidence" "$build" "$work"
    exit "$verification_rc"
}

command=${1-}
case "$command" in
    self-test)
        [ "$#" -eq 1 ] || usage
        reject_ambient_build_overrides
        observe >/dev/null
        . "$script_root/self-test.sh"
        ;;
    observe)
        [ "$#" -eq 1 ] || usage
        observe
        ;;
    run-a)
        shift
        run_a "$@"
        ;;
    verify)
        shift
        verify_evidence "$@"
        ;;
    *) usage ;;
esac
