#!/bin/sh
set -eu

fail() {
    /bin/echo "macOS ACL toolchain preflight rejected: $1" >&2
    exit 1
}

metadata() {
    /usr/bin/stat -f "$2" "$1" 2>/dev/null || fail "metadata unavailable"
}

require_exact_directory() {
    path=$1
    expected_uid=$2
    expected_gid=$3
    expected_mode=$4
    [ "$(metadata "$path" %HT)" = "Directory" ] || fail "fixed directory type drifted"
    [ "$(metadata "$path" %u)" = "$expected_uid" ] || fail "fixed directory UID drifted"
    [ "$(metadata "$path" %g)" = "$expected_gid" ] || fail "fixed directory GID drifted"
    [ "$(metadata "$path" %Lp)" = "$expected_mode" ] || fail "fixed directory mode drifted"
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

[ "$(/usr/bin/uname -m)" = "arm64" ] || fail "runner architecture is not arm64"
require_exact_directory / 0 0 755
require_exact_directory /Applications 0 80 775
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
case "$logical_developer" in
    /Applications/Xcode.app/Contents/Developer|/Applications/Xcode_26.6.app/Contents/Developer) ;;
    *) fail "selected developer directory is outside the closed allowlist" ;;
esac
logical_bundle=${logical_developer%/Contents/Developer}
require_accepted_component "$logical_bundle"
canonical_bundle=$(/bin/realpath "$logical_bundle")
case "$canonical_bundle" in
    /Applications/Xcode.app|/Applications/Xcode_26.6.app) ;;
    *) fail "canonical Xcode bundle is outside the closed allowlist" ;;
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

[ "$xcode_version" = "Xcode 26.6
Build version 17F113" ] || fail "Xcode version/build drifted"
[ "$sdk_version" = "26.5" ] || fail "SDK version drifted"
expected_clang_banner='Apple clang version 21.0.0 (clang-2100.1.1.101)'
case "$clang_version" in
    "$expected_clang_banner"|"$expected_clang_banner
"*) ;;
    *) fail "Apple clang version drifted" ;;
esac

[ "$clang_sha256" = \
    "7def90dd8829726686213a747fc5bff1583df933dae5edc55d755479e0bfe00a" ] \
    || fail "clang digest drifted"
[ "$libtool_sha256" = \
    "229eb9d8027953d2aee0590f983eed587d52bdd1ebc21114a62ce693f77b03f1" ] \
    || fail "libtool digest drifted"
[ "$acl_header_sha256" = \
    "9511f84f0abe1e108e10979900d4fea8567534aef78f0984f7050c49f6c29ff7" ] \
    || fail "sys/acl.h digest drifted"
