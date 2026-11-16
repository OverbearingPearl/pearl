#!/bin/sh
# Run the performance budgets. What is measured lives in ci/perf.el; the
# numbers live in the :perf table of ci/config.el. Pass --report to print the
# table without failing on it.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null || printf '%s' "$here/..")
emacs_bin=${EMACS:-emacs}

command -v "$emacs_bin" >/dev/null 2>&1 || {
  echo "emacs not found; install Emacs or set EMACS=/path/to/emacs" >&2
  exit 1
}

cd "$root"

# A template with the X's spelled out: BSD `mktemp -t NAME` treats NAME as a
# prefix and appends its own suffix, while GNU mktemp requires a template and
# fails with "too few X's".  The explicit form is the one both accept.
out=$(mktemp "${TMPDIR:-/tmp}/perf-out.XXXXXX")
status=$(mktemp "${TMPDIR:-/tmp}/perf-status.XXXXXX")
trap 'rm -f "$out" "$status"' EXIT

eval_form="(kill-emacs (p/perf-main command-line-args-left))"

run_batch() {
  PERF_OUT="$out" PERF_STATUS="$status" PERF_TTY=0 \
    "$emacs_bin" -Q --batch -l "$here/config.el" -l "$here/perf.el" \
    --eval "$eval_form" -- "$@" >/dev/null 2>&1 || true
}

# Redisplay paints nothing in batch mode, so the redisplay benchmarks need a
# terminal. `script' is the portable way to hand one over, but its argument
# order differs between the BSD and GNU versions, so both are spelled out, and
# a run that produces no table falls back to batch mode where those two
# benchmarks report skip instead of losing the whole report.
# A terminal type has to be spelled out: CI sometimes runs with TERM=dumb,
# and Emacs refuses to paint on one of those.
run_tty() {
  TERM=${PERF_TERM:-xterm-256color}
  export TERM
  case $(uname -s) in
    Darwin)
      PERF_OUT="$out" PERF_STATUS="$status" PERF_TTY=1 script -q /dev/null \
        "$emacs_bin" -Q -nw -l "$here/config.el" -l "$here/perf.el" \
        --eval "$eval_form" -- "$@" >/dev/null 2>&1 || true
      ;;
    *)
      # `script -c' takes one string, so the command is assembled once and
      # run by the shell `script' starts.
      perf_cmd="'$emacs_bin' -Q -nw -l '$here/config.el' -l '$here/perf.el' --eval '$eval_form' -- $*"
      if command -v timeout >/dev/null 2>&1; then
        PERF_OUT="$out" PERF_STATUS="$status" PERF_TTY=1 \
          timeout 300 script -q -c "$perf_cmd" /dev/null >/dev/null 2>&1 || true
      else
        PERF_OUT="$out" PERF_STATUS="$status" PERF_TTY=1 \
          script -q -c "$perf_cmd" /dev/null >/dev/null 2>&1 || true
      fi
      ;;
  esac
}

if command -v script >/dev/null 2>&1; then
  run_tty "$@"
fi

if [ ! -s "$out" ]; then
  run_batch "$@"
fi

cat "$out"
exit "$(cat "$status" 2>/dev/null || echo 1)"
