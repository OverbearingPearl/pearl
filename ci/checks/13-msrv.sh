#!/usr/bin/env bash
# 13-msrv: rust-version is at or above the suite floor
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "msrv" "global" "main business" "rust-version is at or above the suite floor"
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
exec "$TOOLS" check msrv --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
