#!/usr/bin/env bash
# 09-crate-prefix: business crates carry the configured crate prefix
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "crate-prefix" "global" "main business" "business crates carry the configured crate prefix"
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
exec "$TOOLS" check crate-prefix --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
