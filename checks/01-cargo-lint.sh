#!/usr/bin/env bash
# 01-cargo-lint: clippy with warnings denied, across all targets.
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf 'cargo-lint\tglobal\tmain business test tooling\tclippy -D warnings across all targets\n'
    exit 0
fi
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$HERE/lib/cargo.sh"
ROOT="."; while [ $# -gt 0 ]; do case "$1" in --root) ROOT="$2"; shift 2;; *) shift;; esac; done
ci_cargo_findings cargo-lint "$ROOT" -- clippy --workspace --all-targets -- -D warnings
