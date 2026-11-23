#!/usr/bin/env bash
# `make size`: a report, never a gate. Delegates to ci-tools size and always
# exits zero -- a number here is information, not a verdict.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"
TOOLS="$(ci_tools_bin "$HERE")" || ci_die "could not build ci-tools"
"$TOOLS" size --root "$ROOT" --suite "$HERE"
exit 0