#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

account=mengxia-task003-ci
created=0
task003_cleanup_second_uid() {
    original_status=$?
    trap - EXIT HUP INT TERM
    cleanup_status=0
    if [ "$created" -eq 1 ]; then
        /usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -delete /Users/mengxia-task003-ci || cleanup_status=$?
        created=0
    fi
    if /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -read /Users/mengxia-task003-ci >/dev/null 2>&1; then
        cleanup_status=1
    fi
    if [ "$cleanup_status" -ne 0 ]; then
        exit "$cleanup_status"
    fi
    exit "$original_status"
}
trap task003_cleanup_second_uid EXIT HUP INT TERM

if /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -read /Users/mengxia-task003-ci >/dev/null 2>&1; then
    exit 1
fi

snapshot=$(/usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -list /Users UniqueID)
printf '%s\n' "$snapshot" | /usr/bin/awk '
    NF != 2 || $1 == "" || $2 !~ /^-?[0-9]+$/ || $2 == "-0" || $2 ~ /^-?0[0-9]+$/ { exit 1 }
    seen_name[$1]++ > 0 || seen_uid[$2]++ > 0 { exit 1 }
'
selected_uid=
candidate=600
while [ "$candidate" -le 699 ]; do
    if ! printf '%s\n' "$snapshot" | /usr/bin/awk -v uid="$candidate" 'NF == 2 && $2 == uid { found = 1 } END { exit found ? 0 : 1 }'; then
        selected_uid=$candidate
        break
    fi
    candidate=$((candidate + 1))
done
test -n "$selected_uid"
case "$selected_uid" in *[!0-9]*|'') exit 1 ;; esac

created=1
/usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci
/usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci UniqueID "$selected_uid"
/usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci PrimaryGroupID 20
/usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci NFSHomeDirectory /var/empty
/usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -create /Users/mengxia-task003-ci UserShell /usr/bin/false

record=$(/usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -read /Users/mengxia-task003-ci UniqueID)
actual_uid=$(printf '%s\n' "$record" | /usr/bin/awk 'NF == 2 && $1 == "UniqueID:" { print $2 }')
test "$actual_uid" = "$selected_uid"
test "$actual_uid" -ne 0
test "$actual_uid" -ne "$(/usr/bin/id -u)"

cargo test -p mengxiad --bin mengxiad --locked --offline task_003_real_second_uid_peer_is_rejected_before_frame -- --exact --ignored --nocapture

/usr/bin/sudo -n -- /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -delete /Users/mengxia-task003-ci
created=0
if /usr/bin/env -i LC_ALL=C LANG=C /usr/bin/dscl . -read /Users/mengxia-task003-ci >/dev/null 2>&1; then
    exit 1
fi
