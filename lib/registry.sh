#!/usr/bin/env bash
# Check discovery and the registry. There is no committed list of checks: the
# table is generated from the files in checks/, sorted by name. Each check is
# an executable that answers two questions:
#   --meta            -> name<TAB>scope<TAB>classes<TAB>description
#   (run with files)  -> TSV findings on stdout, exit 0
# scope is "file" (runs once per applicable file, given the file list) or
# "global" (runs once over the repository, given no files). classes is a
# space-separated list of the classes a file-scoped check applies to.

# Print the generated check table (markdown), reading each check's --meta.
ci_registry_list() {
    local suite="$1"
    local f meta name scope classes desc
    printf '| check | scope | runs on | description |\n'
    printf '| --- | --- | --- | --- |\n'
    for f in "$suite"/checks/*.sh; do
        [ -e "$f" ] || continue
        meta="$(bash "$f" --meta)" || continue
        name="$(printf '%s' "$meta" | cut -f1)"
        scope="$(printf '%s' "$meta" | cut -f2)"
        classes="$(printf '%s' "$meta" | cut -f3)"
        desc="$(printf '%s' "$meta" | cut -f4)"
        printf '| %s | %s | %s | %s |\n' "$name" "$scope" "$classes" "$desc"
    done
}

# The meta line of one check file.
ci_check_meta() {
    bash "$1" --meta
}

# Run one check, emitting its TSV findings. For a file-scoped check the
# applicable files are passed as arguments; a global check gets none.
ci_run_check() {
    local checkfile="$1" tools="$2" root="$3" suite="$4"
    local meta scope classes
    meta="$(ci_check_meta "$checkfile")"
    scope="$(printf '%s' "$meta" | cut -f2)"
    classes="$(printf '%s' "$meta" | cut -f3)"
    if [ "$scope" = "global" ]; then
        bash "$checkfile" --root "$root" --suite "$suite" --tools "$tools"
    else
        # Collect the files whose class is in the check's class list.
        local files="" cls f
        while IFS=$'\t' read -r cls f; do
            [ -z "$f" ] && continue
            for c in $classes; do
                if [ "$cls" = "$c" ]; then
                    files="$files $f"
                    break
                fi
            done
        done <<EOF
$("$tools" files --root "$root" --suite "$suite" --tsv)
EOF
        # shellcheck disable=SC2086
        bash "$checkfile" --root "$root" --suite "$suite" --tools "$tools" $files
    fi
}