#!/usr/bin/env bash
# 12-dependency-direction: crate dependencies respect class may-require
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "dependency-direction" "global" "main business test tooling vendor" "crate dependencies respect class may-require"
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
exec "$TOOLS" check dependency-direction --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
