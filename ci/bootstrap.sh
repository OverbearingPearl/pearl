#!/bin/sh
# Write the files a project owns, in the repository that contains this
# directory. The suite in ci/ is shared and gets replaced on a pull;
# everything written here belongs to the project and is never overwritten
# without --force.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)

usage() {
  cat <<'USAGE'
usage: ci/bootstrap.sh [--force] [--github | --github-only]

Writes the files a project owns. Run it once after adding ci/ as a subtree,
and again only when a pull changes a template.

  --force        overwrite files that already exist, and take over the git
                 hooks even when the project has its own
  --github       also copy ci/templates/github/ to .github/
  --github-only  only do that; leaves the Makefile and the config alone
  --help         this text

What it writes, and what to do with each:
  Makefile                  make lint | test | perf | hooks
  .pre-commit-config.yaml   the message before it is created, lint before a
                            commit, tests before a push
  config/ci.el              the Emacs version this package claims; every other
                            knob of ci/config.el can be overridden there too
  .github/                  with --github: the two gates, lint and test

Hooks are installed only when this run wrote the hook config: a project that
already has `.pre-commit-config.yaml`, or hooks in .git/hooks, keeps them, and
the script says so instead of replacing them. It never installs pre-commit
itself.

After it runs: edit config/ci.el, then `make lint`.
USAGE
}

die() {
  printf 'bootstrap: %s\n' "$1" >&2
  shift
  [ $# -eq 0 ] || printf '  %s\n' "$@" >&2
  exit 1
}

force=0
github=0
github_only=0
for arg in "$@"; do
  case $arg in
    --force) force=1 ;;
    --github) github=1 ;;
    --github-only) github=1; github_only=1 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $arg" "run ci/bootstrap.sh --help" ;;
  esac
done

# Three ways this script is run from a place where its output would be wrong.
[ -d "$root/.git" ] || [ -f "$root/.git" ] || \
  die "$root is not a git repository" \
      "this script writes into the project that contains ci/, and there is none here"

# Run from the suite's own checkout, the parent directory is some other
# repository entirely, and writing there is never what was meant. The suite's
# checkout is the case where ci/ *is* a git toplevel.
here_top=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null || printf '')
[ "$here_top" != "$(cd "$here" && pwd)" ] || \
  die "this is the suite's own repository" \
      "run bootstrap from a project that vendors ci/, not from the suite" \
      "the suite is developed in $(pwd); it is used from <project>/ci"
[ -f "$here/templates/Makefile" ] || \
  die "ci/templates/ is missing" \
      "ci/ was copied rather than added as a subtree, or the subtree is incomplete" \
      "restore it with: git subtree add --prefix=ci <url> main --squash"

missing_targets() {
  file=$1
  missing=""
  for target in lint test perf hooks; do
    grep -q "^$target:" "$file" 2>/dev/null || missing="$missing $target"
  done
  printf '%s' "$missing"
}

write() {
  target=$1
  source=$2
  if [ -e "$target" ] && [ "$force" = 0 ]; then
    if cmp -s "$source" "$target"; then
      echo "kept $target (same as the template)"
    else
      changed=$(diff "$source" "$target" | grep -c '^[<>]')
      echo "kept $target (differs from the template by $changed lines)"
      echo "  copy $source over it to take the new version; --force does the same"
    fi
    case $target in
      Makefile)
        absent=$(missing_targets "$target")
        [ -z "$absent" ] || {
          echo "  it has no$absent target, which is what the hooks and CI call"
          echo "  add them from ci/templates/Makefile, or --force to replace the file"
          echo "  (that discards any target this file already has)"
        }
        ;;
    esac
    return 0
  fi
  mkdir -p "$(dirname "$target")"
  cp "$source" "$target"
  echo "wrote $target"
}

cd "$root"

if [ "$github_only" = 0 ]; then
  write "Makefile" "$here/templates/Makefile"
  write "config/ci.el" "$here/templates/ci.el"
fi

# Where git keeps this repository's hooks. `--git-path' follows core.hooksPath
# and worktrees; the fallback is for a git too old to answer.
hooks_dir=$(git rev-parse --git-path hooks 2>/dev/null || printf '.git/hooks')
case $hooks_dir in
  /*) ;;
  *) hooks_dir="$root/$hooks_dir" ;;
esac

existing_hooks() {
  for hook in pre-commit commit-msg pre-push; do
    if [ -f "$hooks_dir/$hook" ] && [ -x "$hooks_dir/$hook" ]; then
      printf '%s ' "$hook"
    fi
  done
}
have_hooks=$(existing_hooks)

# Installing is decided by who owns the hooks, not by whether the tool is
# present. A project that already configured pre-commit, or already has hooks
# in .git/hooks, keeps them; replacing them is what --force is for. Installing
# ci's hooks over somebody else's setup is the silent takeover this script is
# supposed to prevent.
install_hooks=0

if [ "$github_only" = 1 ]; then
  : # --github-only leaves the Makefile, the config and the hooks alone
elif [ -e ".pre-commit-config.yaml" ] && [ "$force" = 0 ]; then
  echo "kept .pre-commit-config.yaml (exists; --force replaces it)"
  echo "  its hooks are the project's, so none were installed"
  echo "  run \`make hooks' to install them, or --force to take ci's instead"
elif [ -n "$have_hooks" ] && [ "$force" = 0 ]; then
  echo "kept the hooks already in $hooks_dir: $have_hooks"
  echo "  no .pre-commit-config.yaml was written and none were installed"
  echo "  --force writes ci's config and installs its hooks over them"
else
  cat > .pre-commit-config.yaml <<'YAML'
# Local hooks only: the message is checked before it exists, lint before a
# commit, tests before a push. The message shape is ci/check-commit-message.sh
# and is not configurable; this file is the project's now, so drop that hook or
# point it at your own script if your rules differ.
minimum_pre_commit_version: '3.0.0'
fail_fast: true
repos:
  - repo: local
    hooks:
      - id: checks
        name: checks (identical to CI lint)
        entry: make lint
        language: system
        always_run: true
        pass_filenames: false
        require_serial: true

      - id: commit-message
        name: commit message shape
        entry: ci/check-commit-message.sh
        language: system
        stages: [commit-msg]

      # A commit may be rough; a push should be clean.
      - id: tests
        name: ERT (identical to CI test)
        entry: make test
        language: system
        always_run: true
        pass_filenames: false
        require_serial: true
        stages: [pre-push]
YAML
  echo "wrote .pre-commit-config.yaml"
  install_hooks=1
fi

# GitHub reads workflows from the repository root and nowhere else, so this
# part cannot travel in ci/: it is copied, and only when asked for.
if [ "$github" = 1 ]; then
  if [ -d .github ] && [ "$force" = 0 ]; then
    differ=$(diff -rq "$here/templates/github" .github 2>/dev/null | wc -l | tr -d ' ')
    echo "kept .github/ (exists; --force replaces it)"
    if [ "$differ" = 0 ]; then
      echo "  same as the template"
    else
      echo "  differs from ci/templates/github/ in $differ places"
      echo "  copy the files you want by hand; --force replaces the whole directory"
      echo "  (that deletes every workflow already in .github/)"
    fi
  else
    rm -rf .github
    cp -R "$here/templates/github" .github
    echo "wrote .github/ (lint and test; add more jobs by copying them in)"
  fi
fi

if [ "$github_only" = 0 ]; then
  # Installing the hooks is the step people skip, and a hook whose stage was
  # never installed fails silently. Do it here, but only for hooks this run
  # wrote: a project's own setup was reported above and is left alone.
  if [ "$install_hooks" = 1 ]; then
    if command -v pre-commit >/dev/null 2>&1; then
      echo
      echo "installing the hooks:"
      make --no-print-directory hooks
    else
      echo
      echo "pre-commit is not installed, so the hooks are not installed either."
      echo "  bootstrap does not install pre-commit; install it yourself,"
      echo "  then run: make hooks"
    fi
  fi

  # A first lint is worth having immediately: it either confirms the package
  # already satisfies the suite or names the first thing to fix.
  if ls "$root"/*.el >/dev/null 2>&1; then
    echo
    echo "first lint:"
    if make --no-print-directory lint; then
      echo "  clean"
    else
      echo "  the messages above are the first thing to fix; a package that is"
      echo "  still being written is expected to fail some of them"
    fi
  else
    echo
    echo "no .el at the root yet; make lint will say so once there is one"
  fi
fi

cat <<'NEXT'

What you can change from here:
  config/ci.el      directories and the :perf budgets; nothing has to be set
  config/perf.el    a benchmark of this package's own hot path (optional)
  Makefile          add targets; a pull never touches it
  .github/          add jobs; a pull never touches it

Then keep working: the hooks run lint before a commit, the message checker
before a commit is created, and the tests before a push.
NEXT
