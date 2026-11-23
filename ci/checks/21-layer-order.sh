#!/usr/bin/env bash
# 21-layer-order: crate dependencies respect the declared layer order
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "layer-order" "global" "main business" "crate dependencies respect the declared layer order"
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
exec "$TOOLS" check layer-order --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
