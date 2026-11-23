#!/usr/bin/env bash
# Shared shell helpers. Bash 3.2 compatible (macOS ships it): no associative
# arrays, no mapfile. Parsing-heavy work lives in the ci-tools binary; this
# file is only the glue every script sources.

# The suite directory: where these scripts live, regardless of the caller.
ci_suite_dir() {
    local src="${BASH_SOURCE[0]}"
    cd "$(dirname "$src")/.." && pwd
}

# The consumer repository root.
ci_repo_root() {
    git rev-parse --show-toplevel 2>/dev/null || pwd
}

# Path to the ci-tools binary, building it on first use. The build is cached
# by cargo; subsequent calls are a no-op rebuild check.
ci_tools_bin() {
    local suite="$1"
    local bin="$suite/tools/target/release/ci-tools"
    if [ ! -x "$bin" ]; then
        bin="$suite/tools/target/debug/ci-tools"
    fi
    if [ ! -x "$bin" ]; then
        (cd "$suite/tools" && cargo build --quiet >&2) || return 1
        bin="$suite/tools/target/debug/ci-tools"
    fi
    printf '%s\n' "$bin"
}

# A scratch directory cleaned on exit. Caller sets CI_TMP.
ci_mktemp() {
    CI_TMP="$(mktemp -d "${TMPDIR:-/tmp}/ci-rust.XXXXXX")"
    trap 'rm -rf "$CI_TMP"' EXIT
}

ci_die() {
    printf 'ci: %s\n' "$*" >&2
    exit 2
}

# Is a command available?
ci_have() {
    command -v "$1" >/dev/null 2>&1
}