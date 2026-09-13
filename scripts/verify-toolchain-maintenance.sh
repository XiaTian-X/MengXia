#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
test "$#" -eq 0
native=0
. scripts/ci-evidence.sh
# One group executes all five obligations; labels remain component-only. Formal
# delivery additionally requires the actual supply/second-UID/PR/main evidence.
cargo test --locked --offline -p mengxia-testkit --test toolchain_maintenance
for test_id in TEST-MAINT3-ENV-001 TEST-MAINT3-INSTALL-001 TEST-MAINT3-CACHE-001 TEST-MAINT3-SECURITY-001 TEST-MAINT3-INTEGRATION-001; do
    echo "$test_id: COMPONENT_PASS"
done
