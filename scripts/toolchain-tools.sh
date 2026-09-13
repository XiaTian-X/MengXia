#!/bin/sh
# Sourced library; all paths are rooted by the repository-owned caller.
tool_error() { printf '%s: %s\n' "$1" "$2" >&2; exit 2; }
tool_sha() {
    tool_digest_output=$(/usr/bin/shasum -a 256 "$1") || return 2
    printf '%s\n' "$tool_digest_output" | /usr/bin/awk '{print $1}'
}
tool_value() {
    /usr/bin/awk -F ' = ' -v key="$1" '$1 == key { print substr($2, 2, length($2)-2) }' "$tool_manifest"
}
tool_load() {
    tool_manifest=$repository_root/docs/provenance/developer-tools-v1.toml
    [ -f "$tool_manifest" ] && [ ! -L "$tool_manifest" ] || tool_error UNVERIFIABLE 'tool manifest unavailable'
    [ "$(/usr/bin/stat -f %z "$tool_manifest")" -le 4096 ] || tool_error UNVERIFIABLE 'oversized tool manifest'
    /usr/bin/awk '
      BEGIN { allowed="|schema_version|target|rust_toolchain|attestation|protoc_provenance|cargo_deny_version|cargo_deny_archive_sha256|cargo_deny_binary_sha256|" }
      {
        n=split($0,a," = "); k=a[1]; v=a[2]
        if(n!=2 || index(allowed,"|" k "|")==0 || seen[k]++ || v !~ /^"[a-zA-Z0-9_.\/-]+"$/) exit 1
        count++
      }
      END { if(count!=8) exit 1 }
    ' "$tool_manifest" || tool_error UNVERIFIABLE 'invalid tool manifest schema'
    [ "$(tool_value schema_version)" = 1 ] &&
    [ "$(tool_value target)" = aarch64-apple-darwin ] &&
    [ "$(tool_value rust_toolchain)" = rust-toolchain.toml ] &&
    [ "$(tool_value attestation)" = docs/provenance/macos-acl-ffi-toolchain-v1.toml ] &&
    [ "$(tool_value protoc_provenance)" = proto/core/v1/handshake.provenance ] || tool_error UNVERIFIABLE 'unsupported tool manifest'
    tool_version=$(tool_value cargo_deny_version)
    case "$tool_version" in ''|.*|*.|*..*|*[!0-9.]*) tool_error UNVERIFIABLE 'invalid tool version' ;; esac
    tool_archive_sha=$(tool_value cargo_deny_archive_sha256)
    tool_binary_sha=$(tool_value cargo_deny_binary_sha256)
    for tool_hash in "$tool_archive_sha" "$tool_binary_sha"; do
        [ "${#tool_hash}" -eq 64 ] || tool_error UNVERIFIABLE 'invalid tool digest'
        case "$tool_hash" in *[!0-9a-f]*) tool_error UNVERIFIABLE 'invalid tool digest' ;; esac
    done
    tool_base=$repository_root/target/mengxia-tools
    tool_binary=$tool_base/cargo-deny-$tool_version-$tool_binary_sha
}
# Check every ancestor without following a symlink. Same-UID malicious code is
# outside the build-host trust boundary; never accept group/world writable edges.
tool_safe_path() {
    tool_path=$1
    while [ "$tool_path" != / ]; do
        [ ! -L "$tool_path" ] && [ -e "$tool_path" ] || return 1
        tool_uid=$(/usr/bin/stat -f %u "$tool_path") || return 1
        tool_mode=$(/usr/bin/stat -f %Lp "$tool_path") || return 1
        [ "$tool_uid" = 0 ] || [ "$tool_uid" = "$(/usr/bin/id -u)" ] || return 1
        [ $((0$tool_mode & 0022)) -eq 0 ] || return 1
        tool_path=${tool_path%/*}
        [ -n "$tool_path" ] || tool_path=/
    done
}
tool_directory() {
    if [ ! -e "$1" ] && [ ! -L "$1" ]; then
        tool_safe_path "${1%/*}" || tool_error UNVERIFIABLE 'unsafe tool parent'
        (umask 077; /bin/mkdir "$1") 2>/dev/null || [ -d "$1" ] || tool_error NEEDS_PREPARATION 'cannot create tool directory'
    fi
    [ -d "$1" ] && tool_safe_path "$1" || tool_error UNVERIFIABLE 'unsafe tool directory'
}
tool_verify_binary() {
    [ -f "$tool_binary" ] && [ -x "$tool_binary" ] && tool_safe_path "$tool_binary" || return 1
    [ "$(tool_sha "$tool_binary")" = "$tool_binary_sha" ]
}
tool_resolve() {
    tool_load
    [ "$(/usr/bin/uname -m)" = arm64 ] || tool_error UNVERIFIABLE 'tool platform unavailable'
    if [ ! -e "$tool_binary" ] && [ ! -L "$tool_binary" ]; then
        tool_error NEEDS_PREPARATION 'run scripts/dev-toolchain.sh prepare --network'
    fi
    tool_verify_binary || tool_error UNVERIFIABLE 'isolated tool identity/permissions rejected'
    # No ambient verifier fallback, and no execution before byte verification.
    CARGO_DENY_BIN=$tool_binary
    export CARGO_DENY_BIN
}
tool_publish() {
    [ -f "$1" ] && tool_safe_path "$1" && [ "$(tool_sha "$1")" = "$tool_binary_sha" ] \
        || tool_error UNVERIFIABLE 'candidate binary rejected'
    # link(1), unlike ln, never treats an existing directory as a destination
    # container. Existing files/directories/symlinks all fail atomically.
    /bin/link "$1" "$tool_binary" 2>/dev/null || tool_verify_binary \
        || tool_error UNVERIFIABLE 'concurrent publication rejected'
    tool_verify_binary || tool_error UNVERIFIABLE 'published tool changed'
}
tool_prepare() (
    set -eu
    umask 077
    tool_load
    [ "$(/usr/bin/uname -m)" = arm64 ] || tool_error UNVERIFIABLE 'tool platform unavailable'
    tool_directory "$repository_root/target"
    tool_directory "$tool_base"
    if [ -e "$tool_binary" ] || [ -L "$tool_binary" ]; then
        tool_verify_binary || tool_error UNVERIFIABLE 'existing tool is unsafe; not overwritten'
        echo 'TOOLS_PREPARED: unchanged'; exit 0
    fi
    tool_stage=$(/usr/bin/mktemp -d "$tool_base/prepare.XXXXXXXX")
    tool_cleanup() {
        /bin/rm -f -- "$tool_stage/archive" "$tool_stage/binary"
        /bin/rmdir -- "$tool_stage"
    }
    trap tool_cleanup EXIT
    trap 'exit 2' HUP INT TERM
    tool_artifact=cargo-deny-$tool_version-aarch64-apple-darwin
    /usr/bin/env -i PATH=/usr/bin:/bin LC_ALL=C /usr/bin/curl --fail --location \
        --proto '=https' --proto-redir '=https' --tlsv1.2 --connect-timeout 20 --max-time 120 \
        --max-filesize 100000000 --silent --show-error \
        --output "$tool_stage/archive" \
        "https://github.com/EmbarkStudios/cargo-deny/releases/download/$tool_version/$tool_artifact.tar.gz" \
        || tool_error NEEDS_PREPARATION 'official tool download failed; no tool published'
    [ "$(tool_sha "$tool_stage/archive")" = "$tool_archive_sha" ] || tool_error UNVERIFIABLE 'archive checksum rejected'
    # Extract one reviewed member to stdout, never archive paths into the workspace.
    /usr/bin/tar -xOzf "$tool_stage/archive" "$tool_artifact/cargo-deny" > "$tool_stage/binary" \
        || tool_error UNVERIFIABLE 'archive member unavailable'
    [ "$(tool_sha "$tool_stage/binary")" = "$tool_binary_sha" ] || tool_error UNVERIFIABLE 'binary checksum rejected'
    /bin/chmod 500 "$tool_stage/binary"
    # Hard-link publication is atomic and no-clobber, including concurrent prepare.
    tool_publish "$tool_stage/binary"
    echo 'TOOLS_PREPARED: verified official cargo-deny; no global tools changed'
)
