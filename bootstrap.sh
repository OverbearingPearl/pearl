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
usage: ci/bootstrap.sh [--force]

Writes the files a project owns. Run it once after adding ci/ as a subtree,
and again only when a pull changes a template.

  --force        replace what it is allowed to replace: the hook config and
                 .github/.  It never replaces the Makefile, config/ci.toml or
                 .gitignore, because project targets belong in
                 config/targets.mk, the config is the project's alone, and an
                 ignore file usually lists paths ci knows nothing about.  It
                 also takes over the git hooks when the project has its own.
                 It refuses to replace a file with uncommitted changes; stash
                 or commit them first
  --help         this text

What it writes, and what to do with each:
  Makefile                  make lint | test | perf | perf-check | size |
                            coverage | hooks
  .pre-commit-config.yaml   the message before it is created, lint before a
                            commit, tests before a push
  config/ci.toml            the layers, class profiles and budgets this
                            project claims; every knob of ci/config.toml can
                            be overridden there too
  .pi/ai-git-commit.json    the prose half of the message rules -- why the
                            change was made -- which no script reads
  .gitignore                target/, dist/ and CI logs; written only when the
                            project has none
  .github/                  lint, test, size, perf and coverage workflows

Hooks are installed only when this run wrote the hook config: a project that
already has `.pre-commit-config.yaml`, or hooks in .git/hooks, keeps them, and
the script says so instead of replacing them. It never installs pre-commit
itself.

After it runs: edit config/ci.toml, then `make lint`.
USAGE
}

die() {
  printf 'bootstrap: %s\n' "$1" >&2
  shift
  [ $# -eq 0 ] || printf '  %s\n' "$@" >&2
  exit 1
}

force=0
for arg in "$@"; do
  case $arg in
    --force) force=1 ;;
    -h|--help) usage; exit 0 ;;
    *) die "unknown argument: $arg" "run ci/bootstrap.sh --help" ;;
  esac
done

touched=""

refuse_if_dirty() {
  [ -n "$(git status --porcelain -- "$1" 2>/dev/null)" ] || return 0
  die "$1 has uncommitted changes, and --force would replace them" \
      "keep a copy first: git stash push -- $1" \
      "or commit them, then run again"
}

[ -d "$root/.git" ] || [ -f "$root/.git" ] || \
  die "$root is not a git repository" \
      "this script writes into the project that contains ci/, and there is none here"

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
  for target in lint test perf perf-check size coverage hooks; do
    grep -q "^$target:" "$file" 2>/dev/null || missing="$missing $target"
  done
  printf '%s' "$missing"
}

write() {
  target=$1
  source=$2
  if [ -e "$target" ] && { [ "$force" = 0 ] || [ "$target" = "Makefile" ] ||
       [ "$target" = "config/ci.toml" ]; }; then
    if cmp -s "$source" "$target"; then
      echo "kept $target (same as the template)"
    else
      changed=$(diff "$source" "$target" | grep -c '^[<>]')
      echo "kept $target (differs from the template by $changed lines)"
      case $target in
        Makefile|config/ci.toml)
          echo "  this file is the project's once written; --force does not replace it"
          echo "  read the difference with: diff $source $target"
          echo "  take what you want from it; copying it over would drop what is yours" ;;
        *)
          echo "  copy $source over it to take the new version; --force does the same" ;;
      esac
    fi
    case $target in
      Makefile)
        absent=$(missing_targets "$target")
        [ -z "$absent" ] || {
          echo "  it has no$absent target, which is what the hooks and CI call"
          echo "  add them from ci/templates/Makefile, and put this project's own"
          echo "  targets in config/targets.mk, which the template includes"
        }
        ;;
    esac
    return 0
  fi
  if cmp -s "$source" "$target"; then
    echo "kept $target (already the template's)"
    return 0
  fi
  refuse_if_dirty "$target"
  mkdir -p "$(dirname "$target")"
  cp "$source" "$target"
  echo "wrote $target"
  touched="$touched $target"
}

cd "$root"

write "Makefile" "$here/templates/Makefile"
write "config/ci.toml" "$here/templates/ci.toml"
write ".pi/ai-git-commit.json" "$here/templates/ai-git-commit.json"

if [ -e .gitignore ]; then
  echo "kept .gitignore (exists; bootstrap only writes one when absent)"
else
  cp "$here/templates/gitignore" .gitignore
  echo "wrote .gitignore (target, dist and packaging artifacts)"
  touched="$touched .gitignore"
fi

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

install_hooks=0

if [ -e ".pre-commit-config.yaml" ] && [ "$force" = 0 ]; then
  echo "kept .pre-commit-config.yaml (exists; --force replaces it)"
  echo "  its hooks are the project's, so none were installed"
  echo "  run \`make hooks' to install them, or --force to take ci's instead"
elif [ -n "$have_hooks" ] && [ "$force" = 0 ]; then
  echo "kept the hooks already in $hooks_dir: $have_hooks"
  echo "  no .pre-commit-config.yaml was written and none were installed"
  echo "  --force writes ci's config and installs its hooks over them"
else
  config_staged=$(mktemp "${TMPDIR:-/tmp}/pre-commit-config.XXXXXX")
  cat > "$config_staged" <<'YAML'
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
        stages: [pre-commit]
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
        name: cargo test (identical to CI test)
        entry: make test
        language: system
        always_run: true
        pass_filenames: false
        require_serial: true
        stages: [pre-push]
YAML
  if cmp -s "$config_staged" .pre-commit-config.yaml 2>/dev/null; then
    echo "kept .pre-commit-config.yaml (already ci's)"
  else
    refuse_if_dirty .pre-commit-config.yaml
    mv "$config_staged" .pre-commit-config.yaml
    echo "wrote .pre-commit-config.yaml"
    touched="$touched .pre-commit-config.yaml"
  fi
  rm -f "$config_staged"
  install_hooks=1
fi

differ=$(diff -rq "$here/templates/github" .github 2>/dev/null | wc -l | tr -d ' ')
if [ "$differ" = 0 ] && [ -d .github ]; then
  echo "kept .github/ (already the template's)"
elif [ -d .github ] && [ "$force" = 0 ]; then
  echo "kept .github/ (exists; --force replaces it)"
  echo "  differs from ci/templates/github/ in $differ places"
  echo "  copy the files you want by hand; --force replaces the whole directory"
  echo "  (that deletes every workflow already in .github/)"
else
  [ ! -d .github ] || refuse_if_dirty .github
  rm -rf .github
  cp -R "$here/templates/github" .github
  echo "wrote .github/ (ci, perf and release)"
  touched="$touched .github"
fi

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

if [ -f "$root/Cargo.toml" ]; then
  echo
  echo "first lint:"
  if make --no-print-directory lint; then
    echo "  clean"
  else
    echo "  the messages above are the first thing to fix; a workspace that is"
    echo "  still being written is expected to fail some of them"
  fi
else
  echo
  echo "no Cargo.toml at the root yet; make lint will say so once there is one"
fi

report_changes() {
  [ -n "$touched" ] || return 0
  changed=""
  for path in $touched; do
    [ -n "$(git status --porcelain -- "$path" 2>/dev/null)" ] || continue
    changed="$changed $path"
  done
  [ -n "$changed" ] || return 0
  status=$(git status --short -- $changed)
  files=$(printf '%s\n' "$status" | cut -c4- | sed 's/.* -> //')
  printf '\nwritten, and not committed:\n'
  printf '%s\n' "$status" | sed 's/^/  /'
  git diff --stat -- $changed 2>/dev/null | sed 's/^/  /'
  printf '  review with: git diff --\n'
  printf '%s\n' "$files" | sed 's/^/    /'
  case $status in
    *'??'*) printf '  (a new file has nothing to diff yet: git add -N it first)\n' ;;
  esac
}

report_changes

cat <<'NEXT'

What you can change from here:
  config/ci.toml    the layers, class profiles, banned constructs, the perf
                    budgets, the size policy, the coverage directories;
                    nothing has to be set
  config/targets.mk targets of your own, including ones `make lint' runs;
                    the Makefile itself is replaced by a pull, so nothing of
                    yours belongs in it
  .github/          add jobs; a pull never touches it
  .pi/ai-git-commit.json
                    the prose half of the message rules: why the change was
                    made, which the hook cannot check.  Rewrite it when this
                    project's messages follow other rules, and edit the hook
                    config to match

Then keep working: the hooks run lint before a commit, the message checker
before a commit is created, and the tests before a push.
NEXT