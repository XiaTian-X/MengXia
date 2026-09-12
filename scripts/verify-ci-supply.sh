#!/bin/sh
set -eu
repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"
test "$#" -eq 0
native=0
. scripts/ci-evidence.sh
printf 'SUPPLY_CHECKOUT_SHA: %s\n' "$(git rev-parse HEAD)"
printf 'SUPPLY_RUN: %s/%s\n' "${GITHUB_RUN_ID-local}" "${GITHUB_RUN_ATTEMPT-1}"
ci_supply_unavailable
ci_supply
echo 'SHARED_SUPPLY: PASS'
