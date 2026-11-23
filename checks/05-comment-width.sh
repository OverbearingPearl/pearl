#!/usr/bin/env bash
# 05-comment-width: comments stay within comment-width columns
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "comment-width" "file" "main business test tooling" "comments stay within comment-width columns"
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
exec "$TOOLS" check comment-width --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
