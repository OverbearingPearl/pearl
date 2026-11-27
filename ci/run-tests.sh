#!/usr/bin/env bash
# `make test`: the test gate. Runs the workspace's tests; a failure fails the
# run. Uses cargo-nextest when present (nicer output, per-test process
# isolation), otherwise plain cargo test.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"

# A shard names the packages it owns; empty means the whole workspace. The
# Windows suite is long enough that one job per shard is the difference
# between half an hour and ten minutes.
SELECT=(--workspace)
if [ -n "${PEARL_TEST_PACKAGES:-}" ]; then
    SELECT=()
    for package in $PEARL_TEST_PACKAGES; do
        SELECT+=(-p "$package")
    done
fi

if ci_have cargo-nextest; then
    exec cargo nextest run "${SELECT[@]}" --manifest-path "$ROOT/Cargo.toml"
fi
# Every failure at once: a run that stops at the first red suite hides the
# rest, and on a slow runner each hidden failure costs another whole run.
exec cargo test "${SELECT[@]}" --no-fail-fast --manifest-path "$ROOT/Cargo.toml"