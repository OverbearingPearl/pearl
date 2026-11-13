#!/bin/sh
# Export project metadata derived from ci/config.el and the repository.
# Usage: ci/config-export.sh versions|lint|package|version
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null || printf '%s' "$here/..")
emacs_bin=${EMACS:-emacs}
key=${1:?usage: config-export.sh versions|lint|package|version}

cd "$root"

exec "$emacs_bin" -Q --batch \
  -l "$here/config.el" \
  -l "$here/lib/util.el" \
  -l "$here/lib/project.el" \
  --eval '(p/config-export command-line-args-left)' \
  -- "$key"
