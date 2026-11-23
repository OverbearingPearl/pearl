#!/usr/bin/env bash
# `make coverage`: a report, never a gate. Uses cargo-llvm-cov when present;
# otherwise prints how to get it and exits zero. An empty report is a failure
# of the tool, not of the code, so it is reported but does not gate.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"

if ! ci_have cargo-llvm-cov; then
    printf 'coverage: cargo-llvm-cov is not installed.\n'
    printf '  install: cargo install cargo-llvm-cov  (or: ci/deps.sh)\n'
    exit 0
fi

DIRS="$("$HERE/tools/target/debug/ci-tools" config get --root "$ROOT" --suite "$HERE" coverage-dirs 2>/dev/null || echo crates)"
exec cargo llvm-cov --workspace --manifest-path "$ROOT/Cargo.toml" --summary-only