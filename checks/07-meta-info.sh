#!/usr/bin/env bash
# 07-meta-info: Cargo.toml [package] carries name/version/license/description/edition
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "meta-info" "global" "main business" "Cargo.toml [package] carries name/version/license/description/edition"
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
exec "$TOOLS" check meta-info --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
