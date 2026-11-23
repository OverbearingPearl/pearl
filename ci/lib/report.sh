#!/usr/bin/env bash
# Finding reporting. Checks emit TSV records on stdout:
#   SEVERITY<TAB>check<TAB>file<TAB>line<TAB>message
# This file turns them into human lines and, under GitHub Actions, workflow
# annotations. The gate counts ERROR records; WARN records never fail a run.

# Read TSV findings on stdin, print human-readable lines. Returns the number
# of ERROR records via the CI_ERRORS variable (set in the caller's shell when
# sourced, or printed as the last line when run in a subshell).
ci_report_human() {
    local errors=0 warns=0
    local sev check file line msg
    while IFS=$'\t' read -r sev check file line msg; do
        [ -z "$sev" ] && continue
        if [ "$sev" = "ERROR" ]; then
            errors=$((errors + 1))
            printf '%s:%s: [%s] %s\n' "$file" "$line" "$check" "$msg"
        else
            warns=$((warns + 1))
            printf '%s:%s: [%s] warning: %s\n' "$file" "$line" "$check" "$msg"
        fi
    done
    CI_ERRORS="$errors"
    CI_WARNS="$warns"
}

# Under GitHub Actions, also emit workflow annotations from TSV on stdin.
ci_report_github() {
    [ "${GITHUB_ACTIONS:-}" = "true" ] || return 0
    local sev check file line msg level
    while IFS=$'\t' read -r sev check file line msg; do
        [ -z "$sev" ] && continue
        if [ "$sev" = "ERROR" ]; then level="error"; else level="warning"; fi
        printf '::%s file=%s,line=%s,title=%s::%s\n' "$level" "$file" "$line" "$check" "$msg"
    done
}