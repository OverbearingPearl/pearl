#!/usr/bin/env bash
# Project discovery and configuration access, delegated to ci-tools.

# All tracked + untracked-but-not-ignored files, one per line.
ci_tracked() {
    local root="$1"
    git -C "$root" ls-files --cached --others --exclude-standard
}

# Files of a class (or all classified files when no class given).
ci_files() {
    local tools="$1" root="$2" suite="$3" class="${4:-}"
    if [ -n "$class" ]; then
        "$tools" files --root "$root" --suite "$suite" --class "$class"
    else
        "$tools" files --root "$root" --suite "$suite"
    fi
}

# The class of one file, or empty when it belongs to no class.
ci_class() {
    local tools="$1" root="$2" suite="$3" file="$4"
    "$tools" files --root "$root" --suite "$suite" --tsv \
        | awk -F'\t' -v f="$file" '$2 == f { print $1; exit }'
}

# A scalar configuration value.
ci_config() {
    local tools="$1" root="$2" suite="$3" key="$4"
    "$tools" config get --root "$root" --suite "$suite" "$key"
}