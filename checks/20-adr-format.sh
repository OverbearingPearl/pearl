#!/usr/bin/env bash
# 20-adr-format: decision records keep the Nygard shape with Options and Cost
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "adr-format" "global" "main business test tooling" "decision records keep the Nygard shape with Options and Cost"
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
exec "$TOOLS" check adr-format --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
