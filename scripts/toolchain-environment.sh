#!/bin/sh
# Sourced by supported developer entries. No network or global state mutation.
. "$repository_root/scripts/toolchain-tools.sh"

toolchain_rust() {
    # Overrides must not silently change the accepted build/verification inputs.
    for tool_env in RUSTC RUSTDOC RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS \
        CARGO_ENCODED_RUSTFLAGS CARGO_BUILD_RUSTFLAGS CARGO_BUILD_TARGET \
        CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER \
        DEVELOPER_DIR SDKROOT TOOLCHAINS CC CFLAGS CPPFLAGS CPATH C_INCLUDE_PATH \
        CPLUS_INCLUDE_PATH OBJC_INCLUDE_PATH MACOSX_DEPLOYMENT_TARGET ARCHFLAGS \
        LD LDFLAGS LIBRARY_PATH AR ARFLAGS RANLIB RANLIBFLAGS ZERO_AR_DATE; do
        if /usr/bin/printenv "$tool_env" >/dev/null 2>&1; then
            tool_error UNVERIFIABLE "unsupported override: $tool_env"
        fi
    done
    tool_rust_version=$(/usr/bin/awk -F '"' '/^channel = "[0-9]+\.[0-9]+\.[0-9]+"$/ {print $2; n++} END {if(n!=1) exit 1}' "$repository_root/rust-toolchain.toml") \
        || tool_error UNVERIFIABLE 'Rust pin unavailable'
    # rustup itself is a user-installed bootstrap prerequisite, not auto-upgraded.
    command -v rustup >/dev/null || tool_error NEEDS_PREPARATION 'rustup is required'
    tool_rustc=$(rustup which --toolchain "$tool_rust_version" rustc 2>/dev/null) \
        || tool_error NEEDS_PREPARATION 'install repository-pinned Rust with rustup'
    tool_cargo=$(rustup which --toolchain "$tool_rust_version" cargo 2>/dev/null) \
        || tool_error NEEDS_PREPARATION 'pinned Cargo is unavailable'
    tool_safe_path "$tool_rustc" && tool_safe_path "$tool_cargo" \
        || tool_error UNVERIFIABLE 'unsafe Rust tool path'
    tool_rust_banner=$("$tool_rustc" --version) || tool_error UNVERIFIABLE 'Rust version unavailable'
    case "$tool_rust_banner" in "rustc $tool_rust_version "*) ;; *) tool_error UNVERIFIABLE 'Rust version differs from pin' ;; esac
    RUSTUP_TOOLCHAIN=$tool_rust_version
    PATH=${tool_cargo%/*}:$PATH
    export RUSTUP_TOOLCHAIN PATH
}

toolchain_observe() {
    tool_load
    toolchain_rust
    tool_observation=$(/bin/sh "$repository_root/scripts/verify-macos-acl-toolchain.sh" --developer) \
        || tool_error UNVERIFIABLE 'developer tool identity/preparation check failed; see preflight diagnostic'
    tool_rust_sha=$(tool_sha "$tool_rustc") || tool_error UNVERIFIABLE 'Rust digest unavailable'
    tool_cargo_sha=$(tool_sha "$tool_cargo") || tool_error UNVERIFIABLE 'Cargo digest unavailable'
    tool_policy_sha=$(/usr/bin/shasum -a 256 \
        "$repository_root/scripts/toolchain-environment.sh" \
        "$repository_root/scripts/toolchain-tools.sh" \
        "$repository_root/scripts/verify-macos-acl-toolchain.sh" \
        "$repository_root/crates/mengxia-platform-fs/build.rs" \
        "$repository_root/third_party/libsqlite3-sys-0.38.2/build.rs" \
        "$repository_root/docs/provenance/developer-tools-v1.toml" \
        "$repository_root/docs/provenance/macos-acl-ffi-toolchain-v1.toml" \
        "$repository_root/Cargo.lock") || tool_error UNVERIFIABLE 'policy digest unavailable'
    tool_fingerprint_output=$(printf '%s\n' "$tool_observation" "$tool_rust_banner" "$tool_rust_sha" "$tool_cargo_sha" "$tool_policy_sha" \
        | /usr/bin/shasum -a 256) || tool_error UNVERIFIABLE 'fingerprint unavailable'
    tool_fingerprint=${tool_fingerprint_output%% *}
    [ "${#tool_fingerprint}" -eq 64 ] || tool_error UNVERIFIABLE 'invalid fingerprint'
}

toolchain_environment() {
    [ -z "${MENGXIA_ACL_BUILD_CLASS-}" ] || tool_error UNVERIFIABLE 'developer entry cannot accept an attested override'
    if [ -n "${CARGO_TARGET_DIR-}" ] && [ "$CARGO_TARGET_DIR" != "$repository_root/target" ]; then
        tool_error UNVERIFIABLE 'developer task gates require repository target directory'
    fi
    toolchain_observe
    MENGXIA_TOOLCHAIN_FINGERPRINT=$tool_fingerprint
    tool_entry_fingerprint=$tool_fingerprint
    export MENGXIA_TOOLCHAIN_FINGERPRINT
    printf 'DEVELOPER_ENVIRONMENT: %s (invalidation input, not compatibility PASS)\n' "$tool_fingerprint"
}

toolchain_environment_finish() {
    toolchain_observe
    [ "$tool_fingerprint" = "$tool_entry_fingerprint" ] \
        || tool_error UNVERIFIABLE 'environment changed during developer validation; rerun required'
}
