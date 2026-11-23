#!/usr/bin/env bash
# 03-blank-lines: no leading blank line, no three consecutive blanks
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "blank-lines" "file" "main business test tooling" "no leading blank line, no three consecutive blanks"
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
exec "$TOOLS" check blank-lines --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
