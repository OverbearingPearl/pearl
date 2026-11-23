#!/usr/bin/env bash
# 22-stub-orphans: no todo!/unimplemented! stub left in business code
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "stub-orphans" "file" "main business" "no todo!/unimplemented! stub left in business code"
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
exec "$TOOLS" check stub-orphans --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
