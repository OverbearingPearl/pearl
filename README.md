# ci-elisp

A check suite and pipeline for Emacs Lisp packages. It is vendored into a
project as the directory `ci/` and never edited there: a fix lands here, the
projects pull it.

## Add it to a project

```sh
git subtree add --prefix=ci git@github.com:OverbearingPearl/ci-elisp.git main --squash
ci/bootstrap.sh                 # Makefile, .pre-commit-config.yaml, config/ci.el
$EDITOR config/ci.el            # the Emacs version this package claims
make hooks                      # three hooks: lint, commit message, tests
make lint
```

`ci/bootstrap.sh` prints what it wrote, what it left alone and what can be
changed afterwards. `ci/bootstrap.sh --help` is the whole usage.

## Update a project

```sh
git subtree pull --prefix=ci git@github.com:OverbearingPearl/ci-elisp.git main --squash
```

The squash commit names the upstream commit, which is the pin. Then, if the
pull touched a template:

```sh
ci/bootstrap.sh --github-only --force    # refresh .github/ and nothing else
```

## What you own, and what a pull replaces

| half | lives in | pulled |
| --- | --- | --- |
| checks, scripts, benchmarks | `ci/` | yes |
| policy: Emacs versions, directories, budgets | `config/ci.el` | no |
| a benchmark of this package's own hot path | `config/perf.el` | no |
| the table of what ran | `CHECKS.md` | no |
| workflows, actions, `Makefile`, hook config | `.github/`, root files | no, written once |

Editing anything under `ci/` conflicts with the next pull. Everything else is
yours.

## Read on

| Document | Answers |
| --- | --- |
| [MANUAL.md](MANUAL.md) | what each check does, the policy tables, the CI pipeline, MELPA |
| `ci/bootstrap.sh --help` | the knobs a project has, at the moment it runs |
| a project's `config/ci.el` | the knobs, with the defaults commented next to them |
