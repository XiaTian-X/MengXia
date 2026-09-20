#!/bin/sh
set -eu
# No signing, registration, mounting, downloads or arbitrary execution arguments.
[ "$#" -eq 0 ] || exit 64
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
cd "$repository_root"
umask 077
research_root=$(/usr/bin/mktemp -d /tmp/mengxia-r0b001.XXXXXXXX)
printf 'R0B001_DIRECTORY=%s\n' "$research_root"
active_pid=''
cancel() {
    trap '' INT TERM HUP
    if [ -n "$active_pid" ]; then kill -TERM "$active_pid" 2>/dev/null || true; wait "$active_pid" 2>/dev/null || true; fi
    printf 'R0B001_CANCELLED directory=%s\n' "$research_root" >&2
    exit 130
}
trap cancel INT TERM HUP
./scripts/dev-toolchain.sh inspect > "$research_root/toolchain.txt"
/bin/mkdir "$research_root/source" "$research_root/evidence"
for file in r0b-001.c r0b-001.sh controller.c; do
    /bin/cp "scripts/native-research/$file" "$research_root/source/$file"
    /bin/chmod 400 "$research_root/source/$file"
done
clang=$(/usr/bin/xcrun --no-cache --sdk macosx --find clang)
sdk=$(/usr/bin/xcrun --no-cache --sdk macosx --show-sdk-path)
printf 'clang=%s\nsdk=%s\nflags=-std=c11 -O0 -g -Wall -Wextra -Werror -isysroot SDK\n' "$clang" "$sdk" > "$research_root/build.txt"
/usr/bin/env -i LANG=C LC_ALL=C TZ=UTC "$clang" -std=c11 -O0 -g -Wall -Wextra -Werror \
    -isysroot "$sdk" "$research_root/source/r0b-001.c" -o "$research_root/probe" > "$research_root/compile.stdout" 2> "$research_root/compile.stderr" &
active_pid=$!
# Trusted compiler only. Monitor and retain its direct shell child until wait.
count=0
while kill -0 "$active_pid" 2>/dev/null; do
    if [ "$count" -ge 120 ]; then cancel; fi
    /bin/sleep 1
    count=$((count+1))
done
wait "$active_pid"
active_pid=''
/bin/chmod 500 "$research_root/probe"
/usr/bin/shasum -a 256 "$research_root/source/"* "$research_root/probe" "$clang" > "$research_root/sha256.txt"
/usr/bin/sw_vers > "$research_root/host.txt"
/usr/bin/uname -m >> "$research_root/host.txt"
/usr/bin/xcodebuild -version >> "$research_root/host.txt"
git rev-parse HEAD > "$research_root/head.txt"
(cd "$research_root/evidence" && exec /usr/bin/env -i LANG=C LC_ALL=C TZ=UTC "$research_root/probe") \
    > "$research_root/controller.stdout" 2> "$research_root/controller.stderr" &
active_pid=$!
if wait "$active_pid"; then result=0; else result=$?; fi
active_pid=''
for file in r0b-001.c r0b-001.sh controller.c; do
    /usr/bin/cmp "scripts/native-research/$file" "$research_root/source/$file" || exit 1
done
printf 'R0B001_CAPTURE_EXIT=%s directory=%s\n' "$result" "$research_root"
printf '%s\n' 'This is raw research evidence requiring semantic review, never product qualification.'
exit "$result"
