#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
manifest=$repository_root/docs/provenance/macos-acl-ffi-toolchain-v1.toml

fail() {
    /bin/echo "macOS ACL toolchain preflight rejected: $1" >&2
    exit 1
}

metadata() {
    /usr/bin/stat -f "$2" "$1" 2>/dev/null || fail "metadata unavailable"
}

validate_manifest() {
    [ -f "$manifest" ] || fail "attestation manifest is unavailable"
    /usr/bin/awk '
        BEGIN {
            section = "top"
            top = "|schema_version|abi_version|target|minimum_deployment_target|developer_directory_1|developer_directory_2|xcode_version|xcode_build|sdk_version|clang_version|clang_sha256|libtool_sha256|sys_acl_h_sha256|attested_distribution|attested_distribution_sha256|review_source_runner_image|trust_boundary|"
            inputs = "|\"include/mengxia_acl_shim.h\"|\"src/macos_acl_shim.c\"|\"src/macos_acl_abi_probe.c\"|\"tests/macos_acl_shim_test.c\"|"
        }
        /^$/ { next }
        /^\[inputs\]$/ {
            if (section != "top") exit 1
            section = "inputs"
            next
        }
        {
            split_at = index($0, " = ")
            if (split_at == 0) exit 1
            key = substr($0, 1, split_at - 1)
            value = substr($0, split_at + 3)
            allowed = section == "top" ? top : inputs
            if (index(allowed, "|" key "|") == 0 || seen[section SUBSEP key]++) exit 1
            if (section == "top" && (key == "schema_version" || key == "abi_version")) {
                if (value !~ /^[0-9]+$/) exit 1
            } else if (value !~ /^"[^"\\]+"$/) {
                exit 1
            }
            count[section]++
        }
        END { if (count["top"] != 17 || count["inputs"] != 4) exit 1 }
    ' "$manifest" || fail "attestation manifest schema is malformed"
}

manifest_value() {
    key=$1
    count=$(/usr/bin/awk -F' = ' -v key="$key" '$1 == key { count += 1 } END { print count + 0 }' "$manifest")
    [ "$count" -eq 1 ] || fail "manifest value $key is missing or duplicated"
    value=$(/usr/bin/awk -F' = ' -v key="$key" '$1 == key { print $2 }' "$manifest")
    case "$value" in
        \"*\") value=${value#\"}; value=${value%\"} ;;
    esac
    /bin/echo "$value"
}

require_safe_system_directory() {
    path=$1
    kind=$2
    directory_type=$(metadata "$path" %HT)
    uid=$(metadata "$path" %u)
    gid=$(metadata "$path" %g)
    mode=$(metadata "$path" %Lp)
    system_directory_metadata_is_safe "$directory_type" "$uid" "$gid" "$mode" "$kind" \
        || fail "system directory safety predicate rejected metadata"
}

system_directory_metadata_is_safe() {
    directory_type=$1
    uid=$2
    gid=$3
    mode=$4
    kind=$5
    [ "$directory_type" = Directory ] || return 1
    [ "$uid" = 0 ] || return 1
    case "$gid:$mode" in *[!0-9:]*) return 1 ;; esac
    [ $((0$mode & 0002)) -eq 0 ] || return 1
    if [ $((0$mode & 0020)) -ne 0 ]; then
        [ "$kind" = applications ] && [ "$gid" = 80 ] || return 1
    fi
    return 0
}

valid_xcode_bundle_name() {
    name=$1
    [ "$name" = Xcode.app ] && return 0
    case "$name" in Xcode_*.app) version=${name#Xcode_}; version=${version%.app} ;; *) return 1 ;; esac
    case "$version" in ""|.*|*.|*..*|*[!0-9.]*) return 1 ;; esac
    return 0
}

valid_dotted_decimal() {
    value=$1
    case "$value" in ""|.*|*.|*..*|*[!0-9.]*) return 1 ;; esac
    return 0
}

valid_sha256() {
    value=$1
    [ "${#value}" -eq 64 ] || return 1
    case "$value" in *[!0-9a-f]*) return 1 ;; esac
    return 0
}

valid_manifest_developer_directory() {
    developer=$1
    bundle=${developer%/Contents/Developer}
    [ "$bundle" != "$developer" ] || return 1
    name=${bundle#/Applications/}
    [ "/Applications/$name" = "$bundle" ] || return 1
    valid_xcode_bundle_name "$name"
}

validate_manifest_semantics() {
    [ "$(manifest_value schema_version)" = 1 ] || fail "attestation schema is unsupported"
    [ "$(manifest_value abi_version)" = 1 ] || fail "attestation ABI is unsupported"
    [ "$(manifest_value target)" = aarch64-apple-darwin ] \
        || fail "attestation target is unsupported"
    valid_dotted_decimal "$(manifest_value minimum_deployment_target)" \
        || fail "deployment target is malformed"
    valid_manifest_developer_directory "$(manifest_value developer_directory_1)" \
        || fail "first attested developer directory is malformed"
    valid_manifest_developer_directory "$(manifest_value developer_directory_2)" \
        || fail "second attested developer directory is malformed"
    for digest_key in clang_sha256 libtool_sha256 sys_acl_h_sha256 attested_distribution_sha256; do
        valid_sha256 "$(manifest_value "$digest_key")" \
            || fail "attestation digest $digest_key is malformed"
    done
    for input_key in \
        '"include/mengxia_acl_shim.h"' \
        '"src/macos_acl_shim.c"' \
        '"src/macos_acl_abi_probe.c"' \
        '"tests/macos_acl_shim_test.c"'
    do
        valid_sha256 "$(manifest_value "$input_key")" \
            || fail "attestation input digest is malformed"
    done
}

self_test_policy() {
    system_directory_metadata_is_safe Directory 0 0 0755 root
    system_directory_metadata_is_safe Directory 0 80 0775 applications
    system_directory_metadata_is_safe Directory 0 999 0755 applications
    ! system_directory_metadata_is_safe Symbolic 0 0 0755 root
    ! system_directory_metadata_is_safe Directory 501 0 0755 root
    ! system_directory_metadata_is_safe Directory 0 999 0775 applications
    ! system_directory_metadata_is_safe Directory 0 0 0777 root
    valid_xcode_bundle_name Xcode.app
    valid_xcode_bundle_name Xcode_27.1.app
    ! valid_xcode_bundle_name Xcode_beta.app
    ! valid_xcode_bundle_name Xcode_27..1.app
    validate_manifest
    validate_manifest_semantics

    mkdir -p "$repository_root/target"
    fixture=$(/usr/bin/mktemp "$repository_root/target/mengxia-acl-manifest.XXXXXX")
    /bin/cp "$manifest" "$fixture"
    /bin/echo 'unknown_key = "rejected"' >> "$fixture"
    if (manifest=$fixture; validate_manifest; validate_manifest_semantics) 2>/dev/null; then
        /bin/rm -f -- "$fixture"
        fail "manifest unknown-key negative self-test unexpectedly passed"
    fi
    /bin/rm -f -- "$fixture"
    /bin/echo "TOOLCHAIN_POLICY_SELF_TEST_OK"
}

select_attested_xcode() {
    validate_manifest
    validate_manifest_semantics
    require_safe_system_directory / root
    require_safe_system_directory /Applications applications
    selected=$(manifest_value developer_directory_2)
    selected_bundle=${selected%/Contents/Developer}
    [ "$selected_bundle" != "$selected" ] \
        || fail "attested developer directory shape is invalid"
    selected_name=${selected_bundle#/Applications/}
    [ "/Applications/$selected_name" = "$selected_bundle" ] \
        || fail "attested Xcode bundle is outside /Applications"
    valid_xcode_bundle_name "$selected_name" \
        || fail "attested Xcode bundle name is invalid"
    [ -d "$selected" ] || fail "attested developer directory is unavailable"
    /usr/bin/sudo /usr/bin/xcode-select --switch "$selected" \
        || fail "attested Xcode selection failed"
}

require_root_owned_tool() {
    path=$1
    [ "$(metadata "$path" %u)" = "0" ] || fail "system tool is not root-owned"
    mode=$(metadata "$path" %Lp)
    [ $((0$mode & 0022)) -eq 0 ] || fail "system tool is group/world writable"
}

require_accepted_component() {
    path=$1
    uid=$(metadata "$path" %u)
    [ "$uid" = "0" ] || [ "$uid" = "$build_euid" ] \
        || fail "Xcode component owner is outside the accepted set"
    mode=$(metadata "$path" %Lp)
    [ $((0$mode & 0022)) -eq 0 ] || fail "Xcode component is group/world writable"
}

require_canonical_chain() {
    target=$1
    case "$target" in
        "$canonical_bundle"|"$canonical_bundle"/*) ;;
        *) fail "Xcode component escaped the canonical bundle" ;;
    esac
    current=$canonical_bundle
    require_accepted_component "$current"
    remaining=${target#"$canonical_bundle"}
    remaining=${remaining#/}
    while [ -n "$remaining" ]; do
        case "$remaining" in
            */*) component=${remaining%%/*}; remaining=${remaining#*/} ;;
            *) component=$remaining; remaining= ;;
        esac
        [ -n "$component" ] || fail "empty Xcode path component"
        current=$current/$component
        [ "$(metadata "$current" %HT)" != "Symbolic Link" ] \
            || fail "canonical Xcode chain retained a symlink"
        require_accepted_component "$current"
    done
}

case ${1-} in
    --select-attested) select_attested_xcode; exit 0 ;;
    --self-test-policy) self_test_policy; exit 0 ;;
    "") ;;
    *) fail "usage: verify-macos-acl-toolchain.sh [--select-attested]" ;;
esac

validate_manifest
validate_manifest_semantics
[ "$(/usr/bin/uname -m)" = "arm64" ] || fail "runner architecture is not arm64"
require_safe_system_directory / root
require_safe_system_directory /Applications applications
for system_tool in /usr/bin/id /usr/bin/xcode-select /usr/bin/xcodebuild /usr/bin/xcrun; do
    require_root_owned_tool "$system_tool"
done

build_euid=$(/usr/bin/id -u)
build_gid=$(/usr/bin/id -g)
build_groups=$(/usr/bin/id -G)
case "$build_euid:$build_gid:$build_groups" in
    *[!0-9:\ ]*) fail "build identity output is malformed" ;;
esac
case " $build_groups " in
    *" $build_gid "*) ;;
    *) fail "primary GID is missing from supplementary groups" ;;
esac
if [ "$build_euid" != "0" ]; then
    case " $build_groups " in
        *" 80 "*) ;;
        *) fail "non-root build account is not in numeric GID 80" ;;
    esac
fi

logical_developer=$(/usr/bin/xcode-select -p)
logical_bundle=${logical_developer%/Contents/Developer}
[ "$logical_bundle" != "$logical_developer" ] || fail "selected developer directory shape is invalid"
logical_bundle_name=${logical_bundle#/Applications/}
[ "/Applications/$logical_bundle_name" = "$logical_bundle" ] \
    || fail "selected developer directory is outside /Applications"
valid_xcode_bundle_name "$logical_bundle_name" \
    || fail "selected Xcode bundle name is invalid"
require_accepted_component "$logical_bundle"
canonical_bundle=$(/bin/realpath "$logical_bundle")
canonical_bundle_name=${canonical_bundle#/Applications/}
[ "/Applications/$canonical_bundle_name" = "$canonical_bundle" ] \
    || fail "canonical Xcode bundle escaped /Applications"
valid_xcode_bundle_name "$canonical_bundle_name" \
    || fail "canonical Xcode bundle name is invalid"

attested_developer_1=$(manifest_value developer_directory_1)
attested_developer_2=$(manifest_value developer_directory_2)
case "$logical_developer" in
    "$attested_developer_1"|"$attested_developer_2") ;;
    *) fail "selected developer directory is outside the attested manifest" ;;
esac

canonical_developer=$(/bin/realpath "$logical_developer")
clang=$(/usr/bin/xcrun --no-cache --sdk macosx --find clang)
libtool=$(/usr/bin/xcrun --no-cache --sdk macosx --find libtool)
sdk=$(/usr/bin/xcrun --no-cache --sdk macosx --show-sdk-path)
acl_header=$sdk/usr/include/sys/acl.h
for component in "$canonical_developer" "$clang" "$libtool" "$sdk" "$acl_header"; do
    canonical_component=$(/bin/realpath "$component")
    require_canonical_chain "$canonical_component"
done

xcode_version=$(/usr/bin/xcodebuild -version)
sdk_version=$(/usr/bin/xcrun --no-cache --sdk macosx --show-sdk-version)
clang_version=$($clang --version)
clang_sha256=$(/usr/bin/shasum -a 256 "$clang" | /usr/bin/awk '{print $1}')
libtool_sha256=$(/usr/bin/shasum -a 256 "$libtool" | /usr/bin/awk '{print $1}')
acl_header_sha256=$(/usr/bin/shasum -a 256 "$acl_header" | /usr/bin/awk '{print $1}')

# These values are non-secret supply-chain evidence. Emit the observed tuple before
# comparing it so a fail-closed hosted-image rejection remains independently
# reviewable and can never be mistaken for an unrecorded local attestation.
/bin/echo "ImageOS=${ImageOS-unavailable}"
/bin/echo "ImageVersion=${ImageVersion-unavailable}"
/bin/echo "RUNNER_OS=${RUNNER_OS-unavailable}"
/bin/echo "RUNNER_ARCH=${RUNNER_ARCH-unavailable}"
/usr/bin/sw_vers
/bin/echo "build_euid=$build_euid build_gid=$build_gid groups=$build_groups"
/bin/echo "logical_developer=$logical_developer"
/bin/echo "canonical_developer=$canonical_developer"
/bin/echo "sdk=$sdk"
/bin/echo "$xcode_version"
/bin/echo "sdk_version=$sdk_version"
/bin/echo "$clang_version"
/bin/echo "clang_sha256=$clang_sha256"
/bin/echo "libtool_sha256=$libtool_sha256"
/bin/echo "sys_acl_h_sha256=$acl_header_sha256"

expected_xcode_version="Xcode $(manifest_value xcode_version)
Build version $(manifest_value xcode_build)"
expected_sdk_version=$(manifest_value sdk_version)
expected_clang_banner="Apple clang version $(manifest_value clang_version)"
[ "$xcode_version" = "$expected_xcode_version" ] || fail "Xcode version/build drifted"
[ "$sdk_version" = "$expected_sdk_version" ] || fail "SDK version drifted"
case "$clang_version" in
    "$expected_clang_banner"|"$expected_clang_banner
"*) ;;
    *) fail "Apple clang version drifted" ;;
esac

[ "$clang_sha256" = "$(manifest_value clang_sha256)" ] \
    || fail "clang digest drifted"
[ "$libtool_sha256" = "$(manifest_value libtool_sha256)" ] \
    || fail "libtool digest drifted"
[ "$acl_header_sha256" = "$(manifest_value sys_acl_h_sha256)" ] \
    || fail "sys/acl.h digest drifted"
