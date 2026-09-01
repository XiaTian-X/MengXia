#!/bin/sh
set -eu

repository_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository_root"

classify_paths() {
    if [ "$#" -eq 0 ]; then
        echo code
        return
    fi
    for path in "$@"; do
        case "$path" in
            ""|/*|../*|*/../*|*/..|./*|*/./*) echo code; return ;;
            AGENTS.md|docs/spec/*|docs/proposals/*) ;;
            *) echo code; return ;;
        esac
    done
    echo docs
}

if [ "${1-}" = "--paths" ]; then
    shift
    classify_paths "$@"
    exit 0
fi

if [ "$#" -ne 2 ]; then
    echo "usage: scripts/classify-ci-change.sh BASE HEAD | --paths [PATH ...]" >&2
    exit 64
fi

base=$1
head=$2
if ! git rev-parse --verify --quiet "$base^{commit}" >/dev/null ||
   ! git rev-parse --verify --quiet "$head^{commit}" >/dev/null; then
    echo code
    exit 0
fi

set +e
git diff --quiet "$base" "$head" --
all_status=$?
set -e
case "$all_status" in
    0) echo code; exit 0 ;;
    1) ;;
    *) exit "$all_status" ;;
esac

set +e
git diff --quiet "$base" "$head" -- . \
    ':(exclude)docs/spec/**' \
    ':(exclude)docs/proposals/**' \
    ':(exclude)AGENTS.md'
code_status=$?
set -e
case "$code_status" in
    0) echo docs ;;
    1) echo code ;;
    *) exit "$code_status" ;;
esac
