#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
test "$#" -eq 1
case "$1" in report|gate) ;; *) exit 64 ;; esac
events=docs/provenance/toolchain-security-events-v1.tsv
test -f "$events" && test ! -L "$events"
test "$(/usr/bin/stat -f %z "$events")" -le 65536
# Closed reviewed records. No shell commands, credentials, local paths, exception
# switches or unbounded advisory text. A missing source is never a safety claim.
/usr/bin/awk -F '|' '
    NR==1 { if($0!="# mengxia-toolchain-security-events-v1") exit 1; next }
    /^#/ { next }
    {
      if(NF!=6 || $1 !~ /^[A-Z0-9-]+$/ || length($1)>96 || seen[$1]++) exit 1
      if($2 !~ /^(cargo-product|cargo-deny|rust|rustup|macos|xcode|protoc|sqlite|actions)$/) exit 1
      if($3 !~ /^(AFFECTED|UNKNOWN|RESOLVED|NOT_APPLICABLE)$/) exit 1
      for(i=4;i<=5;i++) if($i !~ /^https:\/\/[A-Za-z0-9][A-Za-z0-9._\/-]*$/ || length($i)>512) exit 1
      if($6 !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]$/) exit 1
    }
    END { if(NR==0) exit 1 }
' "$events" || { echo 'UNVERIFIABLE: invalid security event inventory' >&2; exit 2; }
while IFS='|' read -r event_id event_tool event_state event_source event_evidence event_date || [ -n "$event_id" ]; do
    case "$event_id" in '#'*|'') continue ;; esac
    event_epoch=$(/bin/date -j -u -f '%Y-%m-%d' "$event_date" '+%s' 2>/dev/null) \
        || { echo 'UNVERIFIABLE: invalid review date' >&2; exit 2; }
    [ "$(/bin/date -u -r "$event_epoch" '+%Y-%m-%d')" = "$event_date" ] && \
    [ "$event_epoch" -le "$(/bin/date -u '+%s')" ] \
        || { echo 'UNVERIFIABLE: invalid/future review date' >&2; exit 2; }
done < "$events"
printf 'TOOL_SECURITY_REPORT: checkout=%s report_generated_at=%s\n' "$(git rev-parse HEAD)" "$(/bin/date -u +%Y-%m-%dT%H:%M:%SZ)"
echo 'cargo-product: existing cargo-deny fetch/check is required separately; https://rustsec.org/'
echo 'cargo-deny: UNKNOWN tool-build dependency review; https://github.com/EmbarkStudios/cargo-deny/security/advisories'
echo 'rust/rustup: UNKNOWN official advisory applicability; https://blog.rust-lang.org/'
echo 'macos/xcode: UNKNOWN official advisory applicability; https://support.apple.com/en-us/100100'
echo 'protoc: UNKNOWN compiler advisory applicability; https://github.com/protocolbuffers/protobuf/security/advisories'
echo 'sqlite: UNKNOWN vendored-source advisory applicability; https://sqlite.org/changes.html'
echo 'actions: UNKNOWN runtime/EOL applicability; Dependabot and CodeQL remain enabled'
echo 'CONTINUOUS_REPAIR_AGENT: NOT_ENABLED'
echo 'TOOL_SECURITY_COVERAGE: PARTIAL; report is not full security PASS'
/usr/bin/awk -F '|' '!/^#/ {print "SECURITY_EVENT: " $1 " tool=" $2 " state=" $3 " reviewed=" $6}' "$events"
if [ "$1" = gate ]; then
    if ! /usr/bin/awk -F '|' '!/^#/ && ($3=="AFFECTED" || $3=="UNKNOWN") {exit 1}' "$events"; then
        echo 'SECURITY_UPDATE_REQUIRED: unresolved advisory event blocks supply acceptance' >&2
        exit 1
    fi
    echo 'SECURITY_EVENT_GATE: no recorded unresolved event (not a complete advisory review)'
fi
