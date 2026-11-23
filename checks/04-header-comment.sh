#!/usr/bin/env bash
# 04-header-comment: a business module opens with a //! module doc
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "header-comment" "file" "main business test tooling" "a business module opens with a //! module doc"
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
exec "$TOOLS" check header-comment --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
