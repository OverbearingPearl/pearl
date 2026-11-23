#!/usr/bin/env bash
# Install the external tools the suite shells out to, and build ci-tools.
# Network is touched here, once; afterwards the suite runs offline. Every
# install is idempotent -- an already-present tool is left alone.
#
#   cargo-audit     advisory database (the audit check / make audit)
#   cargo-deny      licenses, bans, advisories (make deny)
#   cargo-llvm-cov  coverage (make coverage)
#   cargo-nextest   nicer test runner (make test, optional)
#
# Usage: ci/deps.sh [--check]   --check only reports what is missing.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"

CHECK=0
[ "${1:-}" = "--check" ] && CHECK=1

need() {
    local bin="$1" crate="$2"
    if ci_have "$bin"; then
        printf '  ok      %s\n' "$bin"
        return 0
    fi
    if [ "$CHECK" = "1" ]; then
        printf '  missing %s  (cargo install %s)\n' "$bin" "$crate"
        return 1
    fi
    printf '  install %s ...\n' "$bin"
    cargo install "$crate" --locked
}

rc=0
printf 'external tools:\n'
need cargo-audit cargo-audit || rc=1
need cargo-deny cargo-deny || rc=1
need cargo-llvm-cov cargo-llvm-cov || rc=1
need cargo-nextest cargo-nextest || rc=1

printf 'ci-tools:\n'
if [ "$CHECK" = "1" ]; then
    if [ -x "$HERE/tools/target/debug/ci-tools" ] || [ -x "$HERE/tools/target/release/ci-tools" ]; then
        printf '  ok      ci-tools\n'
    else
        printf '  missing ci-tools  (cd ci/tools && cargo build)\n'
        rc=1
    fi
else
    (cd "$HERE/tools" && cargo build --release --quiet) && printf '  ok      ci-tools\n'
fi

exit "$rc"