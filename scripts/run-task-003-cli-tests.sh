#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

fixture=$(/usr/bin/mktemp -d "$repository_root/target/mengxia-task003-cli.XXXXXX")
daemon_pid=
cleanup() {
    if [ -n "$daemon_pid" ] && /bin/kill -0 "$daemon_pid" 2>/dev/null; then
        /bin/kill -TERM "$daemon_pid" 2>/dev/null || true
        wait "$daemon_pid" 2>/dev/null || true
    fi
    /bin/rm -rf -- "$fixture"
}
trap cleanup EXIT HUP INT TERM
/bin/chmod 700 "$fixture"

cargo test --locked --offline -p mengxiad -p mengxia \
    typed_layers_obey_cli_environment_library_default_precedence
cargo build --locked --offline -p mengxiad -p mengxia

target/debug/mengxiad --help >"$fixture/daemon-help.out" 2>"$fixture/daemon-help.err"
test ! -s "$fixture/daemon-help.err"
/usr/bin/grep -F 'mengxiad serve [--library-root PATH] [--client-endpoint PATH]' "$fixture/daemon-help.out" >/dev/null
target/debug/mengxia --help >"$fixture/client-help.out" 2>"$fixture/client-help.err"
test ! -s "$fixture/client-help.err"
/usr/bin/grep -F 'mengxia handshake [--client-endpoint PATH]' "$fixture/client-help.out" >/dev/null

set +e
target/debug/mengxia handshake --max-frame-bytes=-1 >"$fixture/invalid.out" 2>"$fixture/invalid.err"
invalid_status=$?
set -e
test "$invalid_status" -eq 2
test ! -s "$fixture/invalid.out"
test "$(/bin/cat "$fixture/invalid.err")" = 'MENGXIA_ERROR code=VALIDATION_ERROR'

set +e
MENGXIA_MAX_FRAME_BYTES=invalid target/debug/mengxia handshake \
    --client-endpoint "$fixture/missing-runtime/client.sock" \
    --max-frame-bytes 65536 \
    >"$fixture/precedence.out" 2>"$fixture/precedence.err"
precedence_status=$?
set -e
test "$precedence_status" -eq 1
test ! -s "$fixture/precedence.out"
test "$(/bin/cat "$fixture/precedence.err")" = \
    'MENGXIA_ERROR code=STORAGE_CONFIGURATION_ERROR'

set +e
MENGXIA_MAX_FRAME_BYTES=invalid target/debug/mengxia handshake \
    --client-endpoint "$fixture/missing-runtime/client.sock" \
    >"$fixture/environment-invalid.out" 2>"$fixture/environment-invalid.err"
environment_invalid_status=$?
set -e
test "$environment_invalid_status" -eq 2
test ! -s "$fixture/environment-invalid.out"
test "$(/bin/cat "$fixture/environment-invalid.err")" = \
    'MENGXIA_ERROR code=VALIDATION_ERROR'

set +e
target/debug/mengxiad serve \
    --library-root "$fixture/Depth2Library" \
    --client-endpoint "$fixture/depth2-runtime/client.sock" \
    --max-decode-depth 2 \
    >"$fixture/depth2.out" 2>"$fixture/depth2.err"
depth2_status=$?
set -e
test "$depth2_status" -eq 2
test ! -e "$fixture/Depth2Library"
test ! -e "$fixture/depth2-runtime"
test ! -s "$fixture/depth2.out"
test "$(/bin/cat "$fixture/depth2.err")" = 'MENGXIA_ERROR code=VALIDATION_ERROR'

endpoint="$fixture/runtime/client.sock"
target/debug/mengxiad serve \
    --library-root "$fixture/Library" \
    --client-endpoint "$endpoint" \
    --max-frame-bytes 65536 \
    --max-decode-depth 3 \
    --client-handshake-timeout-ms 100 \
    --max-pending-handshakes 1 \
    >"$fixture/daemon.out" 2>"$fixture/daemon.err" &
daemon_pid=$!
ready=0
attempt=0
while [ "$attempt" -lt 100 ]; do
    if [ -S "$endpoint" ]; then
        ready=1
        break
    fi
    /bin/kill -0 "$daemon_pid" 2>/dev/null || break
    /bin/sleep 0.05
    attempt=$((attempt + 1))
done
test "$ready" -eq 1

target/debug/mengxia handshake \
    --client-endpoint "$endpoint" \
    --max-frame-bytes 65536 \
    --max-decode-depth 3 \
    --client-handshake-timeout-ms 100 \
    >"$fixture/client.out" 2>"$fixture/client.err"
test ! -s "$fixture/client.err"
/usr/bin/grep -E '^MENGXIA_HANDSHAKE_OK protocol=1\.0 request_id=[0-9a-f-]{36} correlation_id=[0-9a-f-]{36}$' "$fixture/client.out" >/dev/null

target/debug/mengxia handshake \
    --client-endpoint "$endpoint" \
    --max-frame-bytes 65536 \
    --max-decode-depth 64 \
    --client-handshake-timeout-ms 100 \
    >"$fixture/client-depth64.out" 2>"$fixture/client-depth64.err"
test ! -s "$fixture/client-depth64.err"
/usr/bin/grep -E '^MENGXIA_HANDSHAKE_OK protocol=1\.0 request_id=[0-9a-f-]{36} correlation_id=[0-9a-f-]{36}$' "$fixture/client-depth64.out" >/dev/null

/bin/kill -TERM "$daemon_pid"
wait "$daemon_pid"
daemon_pid=
test ! -s "$fixture/daemon.out"
test ! -s "$fixture/daemon.err"
test ! -e "$endpoint"

target/debug/mengxiad serve \
    --library-root "$fixture/Library" \
    --client-endpoint "$endpoint" \
    >"$fixture/daemon-reopen.out" 2>"$fixture/daemon-reopen.err" &
daemon_pid=$!
ready=0
attempt=0
while [ "$attempt" -lt 100 ]; do
    if [ -S "$endpoint" ]; then
        ready=1
        break
    fi
    /bin/kill -0 "$daemon_pid" 2>/dev/null || break
    /bin/sleep 0.05
    attempt=$((attempt + 1))
done
test "$ready" -eq 1
/bin/kill -INT "$daemon_pid"
wait "$daemon_pid"
daemon_pid=
test ! -s "$fixture/daemon-reopen.out"
test ! -s "$fixture/daemon-reopen.err"
test ! -e "$endpoint"
