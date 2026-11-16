#!/bin/sh
# Write the files a project owns, in the repository that contains this
# directory. The suite in ci/ is shared and gets replaced; everything written
# here is written once and then belongs to the project.
#
# Refuses to overwrite. Pass --force to replace a file you have edited, and
# --github to also install the workflow and action templates.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)

force=0
github=0
for arg in "$@"; do
  case $arg in
    --force) force=1 ;;
    --github) github=1 ;;
    -h|--help)
      cat <<'USAGE'
usage: ci/bootstrap.sh [--force] [--github]

  --force    overwrite files that already exist
  --github   copy templates/github/ to .github/ as well
USAGE
      exit 0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

write() {
  target=$1
  source=$2
  if [ -e "$target" ] && [ "$force" = 0 ]; then
    echo "kept $target (exists; --force replaces it)"
    return 0
  fi
  mkdir -p "$(dirname "$target")"
  cp "$source" "$target"
  echo "wrote $target"
}

cd "$root"

write "Makefile" "$here/templates/Makefile"
write "config/ci.el" "$here/templates/ci.el"

# The hook configuration is small enough to write here rather than as a
# template with one substitution.
if [ -e ".pre-commit-config.yaml" ] && [ "$force" = 0 ]; then
  echo "kept .pre-commit-config.yaml (exists; --force replaces it)"
else
  cat > .pre-commit-config.yaml <<'YAML'
# Local hooks only. The rules live in .pi/ai-git-commit.json as prose; this is
# the half of them a script can see, plus the two gates the suite provides.
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
fi

# GitHub reads workflows from the repository root and nowhere else, so this
# part can never be shared through ci/: it is copied, and it is opt-in because
# a project may already have its own.
if [ "$github" = 1 ]; then
  if [ -d .github ] && [ "$force" = 0 ]; then
    echo "kept .github/ (exists; --force replaces it)"
  else
    rm -rf .github
    cp -R "$here/templates/github" .github
    echo "wrote .github/"
  fi
fi

cat <<'NEXT'

Next:
  1. edit config/ci.el if the defaults do not fit this project
  2. make hooks          install the three hook types
  3. make lint           a suite that cannot find a module says so precisely
NEXT
