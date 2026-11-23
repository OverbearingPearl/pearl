#!/usr/bin/env bash
# The lint orchestrator. Discovers the checks in checks/, runs each over the
# files its class profile admits, collects their findings, reports them, and
# gates on the ERROR count. Modes:
#   (none)        run every check; exit nonzero if any ERROR finding
#   --list        print the generated check table (never committed)
#   --files       print discovered files by class
#   --explain F   print the class and profile of one file
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/util.sh
. "$HERE/lib/util.sh"
# shellcheck source=lib/project.sh
. "$HERE/lib/project.sh"
# shellcheck source=lib/report.sh
. "$HERE/lib/report.sh"
# shellcheck source=lib/registry.sh
. "$HERE/lib/registry.sh"

ROOT="$(ci_repo_root)"
SUITE="$HERE"
MODE="run"
EXPLAIN_FILE=""

for arg in "$@"; do
    case "$arg" in
        --list) MODE="list" ;;
        --files) MODE="files" ;;
        --explain) MODE="explain" ;;
        --help|-h) MODE="help" ;;
        *)
            if [ "$MODE" = "explain" ] && [ -z "$EXPLAIN_FILE" ]; then
                EXPLAIN_FILE="$arg"
            fi
            ;;
    esac
done

case "$MODE" in
    help)
        sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
        exit 0
        ;;
    list)
        ci_registry_list "$SUITE"
        exit 0
        ;;
esac

TOOLS="$(ci_tools_bin "$SUITE")" || ci_die "could not build ci-tools"

case "$MODE" in
    files)
        "$TOOLS" files --root "$ROOT" --suite "$SUITE"
        exit 0
        ;;
    explain)
        [ -n "$EXPLAIN_FILE" ] || ci_die "--explain needs a file"
        "$TOOLS" explain --root "$ROOT" --suite "$SUITE" "$EXPLAIN_FILE"
        exit 0
        ;;
esac

ci_mktemp
FINDINGS="$CI_TMP/findings.tsv"
: > "$FINDINGS"

for checkfile in "$SUITE"/checks/*.sh; do
    [ -e "$checkfile" ] || continue
    ci_run_check "$checkfile" "$TOOLS" "$ROOT" "$SUITE" >> "$FINDINGS" || true
done

# Report once, from the collected records.
ci_report_human < "$FINDINGS"
ci_report_github < "$FINDINGS"

if [ "${CI_ERRORS:-0}" -gt 0 ]; then
    printf '\nFAILED: %s error(s), %s warning(s)\n' "$CI_ERRORS" "${CI_WARNS:-0}" >&2
    exit 1
fi
printf 'OK: %s warning(s)\n' "${CI_WARNS:-0}"
exit 0