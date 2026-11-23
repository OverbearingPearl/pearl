#!/usr/bin/env bash
# `make test`: the test gate. Runs the workspace's tests; a failure fails the
# run. Uses cargo-nextest when present (nicer output, per-test process
# isolation), otherwise plain cargo test.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"

if ci_have cargo-nextest; then
    exec cargo nextest run --workspace --manifest-path "$ROOT/Cargo.toml"
fi
exec cargo test --workspace --manifest-path "$ROOT/Cargo.toml"