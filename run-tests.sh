#!/usr/bin/env bash
# `make test`: the test gate. Runs the workspace's tests; a failure fails the
# run. Uses cargo-nextest when present (nicer output, per-test process
# isolation), otherwise plain cargo test.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"

# A shard names the packages it owns; empty means the whole workspace. One
# platform can be slow enough that running its suites in a row costs more
# than the work they measure, so the workflow may hand each job a part.
SELECT=(--workspace)
if [ -n "${CI_TEST_PACKAGES:-}" ]; then
    SELECT=()
    for package in $CI_TEST_PACKAGES; do
        SELECT+=(-p "$package")
    done
fi

# A probe names one family of stories. While a platform's sandbox is being
# measured, twenty-five minutes per answer is too much to pay for a question
# about two of them. Empty runs everything.
FILTER=()
if [ -n "${CI_TEST_ONLY:-}" ]; then
    FILTER=(-E "$CI_TEST_ONLY")
fi

if ci_have cargo-nextest; then
    exec cargo nextest run "${SELECT[@]}" "${FILTER[@]}" --manifest-path "$ROOT/Cargo.toml"
fi
# Every failure at once. A run that stops at the first red suite hides the
# rest, and on a slow platform each hidden failure costs another whole run.
exec cargo test "${SELECT[@]}" --no-fail-fast --manifest-path "$ROOT/Cargo.toml"
