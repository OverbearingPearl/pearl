#!/usr/bin/env bash
# 11-test-suffix: a -tests file lives in the test class
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "test-suffix" "file" "main business test" "a -tests file lives in the test class"
    exit 0
fi
TOOLS=""; ROOT="."; SUITE="ci"; FILES=()
while [ $# -gt 0 ]; do
    case "$1" in
        --tools) TOOLS="$2"; shift 2 ;;
        --root) ROOT="$2"; shift 2 ;;
        --suite) SUITE="$2"; shift 2 ;;
        *) FILES+=("$1"); shift ;;
    esac
done
exec "$TOOLS" check test-suffix --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
