#!/usr/bin/env bash
# 16-policy: the configuration is self-consistent
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "policy" "global" "main business test tooling" "the configuration is self-consistent"
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
exec "$TOOLS" check policy --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
