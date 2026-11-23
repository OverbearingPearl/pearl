#!/usr/bin/env bash
# 02-build: the workspace compiles.
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf 'build\tglobal\tmain business test tooling\tthe workspace compiles\n'
    exit 0
fi
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$HERE/lib/cargo.sh"
ROOT="."; while [ $# -gt 0 ]; do case "$1" in --root) ROOT="$2"; shift 2;; *) shift;; esac; done
ci_cargo_findings build "$ROOT" -- build --workspace
