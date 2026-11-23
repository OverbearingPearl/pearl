#!/usr/bin/env bash
# Export the resolved configuration as CI_* shell variables, for workflows and
# project targets that want the same values the checks use. Source it, or run
# it to print the assignments.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"
TOOLS="$(ci_tools_bin "$HERE")" || ci_die "could not build ci-tools"
"$TOOLS" config env --root "$ROOT" --suite "$HERE"