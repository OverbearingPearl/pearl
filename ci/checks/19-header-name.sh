#!/usr/bin/env bash
# 19-header-name: a //! name --- header names its module
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "header-name" "file" "main business test tooling" "a //! name --- header names its module"
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
exec "$TOOLS" check header-name --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
