#!/bin/sh
# Run the performance budgets. What is measured lives in ci/perf.el; the
# numbers live in ci/config.el. Pass --report to print the table without
# failing on it.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null || printf '%s' "$here/..")
emacs_bin=${EMACS:-emacs}

command -v "$emacs_bin" >/dev/null 2>&1 || {
  echo "emacs not found; install Emacs or set EMACS=/path/to/emacs" >&2
  exit 1
}

cd "$root"

out=$(mktemp -t perf-out)
status=$(mktemp -t perf-status)
trap 'rm -f "$out" "$status"' EXIT

eval_form="(kill-emacs (p/perf-main command-line-args-left))"

# Redisplay paints nothing in batch mode, and `script' is the portable way to
# hand Emacs a terminal. Its argument order differs between BSD and GNU, so
# both are spelled out. Without it the redisplay benchmarks report `skip'
# instead of failing.
if command -v script >/dev/null 2>&1; then
  case $(uname -s) in
    Darwin)
      # No --batch here: batch mode never paints, so redisplays would measure
      # nothing. -nw uses the terminal that `script' provided.
      PERF_OUT="$out" PERF_STATUS="$status" PERF_TTY=1 script -q /dev/null \
        "$emacs_bin" -Q -nw -l "$here/config.el" -l "$here/perf.el" \
        --eval "$eval_form" -- "$@" >/dev/null 2>&1 || true
      ;;
    *)
      # `script -c' takes one string; the only argument this target accepts is
      # --report, so quoting it is not a problem here.
      PERF_OUT="$out" PERF_STATUS="$status" PERF_TTY=1 script -q -c \
        "'$emacs_bin' -Q -nw -l '$here/config.el' -l '$here/perf.el' --eval '$eval_form' -- $*" \
        /dev/null >/dev/null 2>&1 || true
      ;;
  esac
else
  PERF_OUT="$out" PERF_STATUS="$status" PERF_TTY=0 \
    "$emacs_bin" -Q --batch -l "$here/config.el" -l "$here/perf.el" \
    --eval "$eval_form" -- "$@" >/dev/null 2>&1 || true
fi

cat "$out"
exit "$(cat "$status" 2>/dev/null || echo 1)"
