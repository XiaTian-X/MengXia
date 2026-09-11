#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
manifest=$repository_root/docs/provenance/macos-acl-ffi-toolchain-v1.toml

manifest_value() {
    key=$1
    count=$(/usr/bin/awk -F' = ' -v key="$key" '$1 == key { count += 1 } END { print count + 0 }' "$manifest")
    [ "$count" -eq 1 ] || return 2
    /usr/bin/awk -F' = ' -v key="$key" '$1 == key { value=$2; gsub(/^"|"$/, "", value); print value }' "$manifest"
}

logical_developer=$(/usr/bin/xcode-select -p) || {
    echo UNVERIFIABLE
    exit 2
}
case "$logical_developer" in
    /Applications/Xcode.app/Contents/Developer|/Applications/Xcode_[0-9]*.app/Contents/Developer) ;;
    *) echo UNSUPPORTED; exit 1 ;;
esac

if ! /usr/bin/env -u MENGXIA_ACL_BUILD_CLASS cargo check --locked --offline -p mengxia-platform-fs; then
    echo UNSUPPORTED
    exit 1
fi

xcode_version=$(/usr/bin/xcodebuild -version) || { echo UNVERIFIABLE; exit 2; }
sdk_version=$(/usr/bin/xcrun --no-cache --sdk macosx --show-sdk-version) || { echo UNVERIFIABLE; exit 2; }
expected_xcode="Xcode $(manifest_value xcode_version)
Build version $(manifest_value xcode_build)"
expected_sdk=$(manifest_value sdk_version)

if [ "$xcode_version" = "$expected_xcode" ] && [ "$sdk_version" = "$expected_sdk" ]; then
    echo COMPATIBLE_DEVELOPER
else
    echo REATTEST_REQUIRED
fi
