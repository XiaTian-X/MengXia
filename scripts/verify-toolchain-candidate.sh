#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
test "$#" -eq 0
. scripts/toolchain-environment.sh
toolchain_environment
candidate_fingerprint=$tool_fingerprint
tool_directory "$repository_root/target"
tool_directory "$repository_root/target/toolchain-evidence"
candidate_directory=$(/usr/bin/mktemp -d "$repository_root/target/toolchain-evidence/candidate.XXXXXXXX")
# Preserve generated logs for diagnosis; never reuse a candidate build directory.
export CARGO_TARGET_DIR=$candidate_directory/build
if ! cargo metadata --locked --offline --format-version 1 > "$candidate_directory/metadata.json" 2> "$candidate_directory/preparation.log"; then
    tool_error NEEDS_PREPARATION "offline resolution failed; inspect $candidate_directory/preparation.log (not ABI incompatibility)"
fi
if ! cargo test --locked --offline -p mengxia-store-sqlite --lib runtime::tests \
    > "$candidate_directory/native.log" 2>&1; then
    tool_error SOURCE_CHECK_FAILED "clean native build/test failed; compatibility unproven; see $candidate_directory/native.log"
fi
/usr/bin/grep -E '^test result: ok\. [1-9][0-9]* passed;' "$candidate_directory/native.log" >/dev/null \
    || tool_error UNVERIFIABLE 'candidate ran no runtime tests'
toolchain_observe
[ "$tool_fingerprint" = "$candidate_fingerprint" ] || tool_error UNVERIFIABLE 'environment changed during candidate verification'
printf '%s\n' "$tool_observation" "environment_fingerprint=$tool_fingerprint" > "$candidate_directory/environment.txt"
echo "CANDIDATE_EVIDENCE: $candidate_directory"
echo 'COMPATIBILITY: PASS (clean native ABI/SQLite candidate only)'
printf '%s\n' "$tool_observation" | /usr/bin/grep '^ATTESTATION_MATCH:'
echo 'SECURITY_COVERAGE: UNKNOWN (run supply and advisory review separately)'
