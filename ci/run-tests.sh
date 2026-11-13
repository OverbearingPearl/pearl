#!/bin/sh
# Run the ERT test suite. Test files are discovered, never listed:
# every .el file under the test directory.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null || printf '%s' "$here/..")
emacs_bin=${EMACS:-emacs}
submodule=$("$here/config-export.sh" submodule)
testdir=$("$here/config-export.sh" test-dir)

cd "$root"

set --
found=0
for candidate in "$root/$testdir"/*.el; do
  [ -f "$candidate" ] || continue
  set -- "$@" -l "$candidate"
  found=1
done

if [ "$found" -eq 0 ]; then
  echo "no test modules found under $testdir/; skipping ERT"
  exit 0
fi

exec "$emacs_bin" -Q --batch \
  -L "$root" -L "$root/$submodule" -L "$root/$testdir" \
  "$@" \
  --eval '(ert-run-tests-batch-and-exit)'
