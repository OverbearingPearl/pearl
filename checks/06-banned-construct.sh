#!/usr/bin/env bash
# 06-banned-construct: no construct the policy bans
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "banned-construct" "file" "main business test tooling" "no construct the policy bans"
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
exec "$TOOLS" check banned-construct --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
