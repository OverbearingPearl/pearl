#!/usr/bin/env bash
# 00-rustdoc: documentation builds with warnings denied.
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf 'rustdoc\tglobal\tmain business test tooling\tdocumentation builds with -D warnings\n'
    exit 0
fi
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
. "$HERE/lib/cargo.sh"
ROOT="."; while [ $# -gt 0 ]; do case "$1" in --root) ROOT="$2"; shift 2;; *) shift;; esac; done
RUSTDOCFLAGS="-D warnings" ci_cargo_findings rustdoc "$ROOT" -- doc --workspace --no-deps
