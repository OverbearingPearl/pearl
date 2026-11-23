#!/usr/bin/env bash
# 17-project-neutral: the suite source names no project
set -euo pipefail
if [ "${1:-}" = "--meta" ]; then
    printf '%s\t%s\t%s\t%s\n' "project-neutral" "global" "tooling" "the suite source names no project"
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
exec "$TOOLS" check project-neutral --root "$ROOT" --suite "$SUITE" ${FILES[@]+"${FILES[@]}"}
