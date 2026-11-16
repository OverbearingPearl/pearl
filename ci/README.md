# ci-elisp

A check suite and pipeline for Emacs Lisp packages. It is vendored into a
project as the directory `ci/` and never edited there: a fix lands here, the
projects pull it.

## Add it to a project

```sh
git subtree add --prefix=ci git@github.com:OverbearingPearl/ci-elisp.git main --squash
ci/bootstrap.sh
```

Two commands. The second writes the project's files, installs the three hooks
when `pre-commit` is present, and runs a first `make lint`, saying either that
the package is clean or which message is the first to fix.

`ci/bootstrap.sh --help` is the whole usage, and it ends with the four things
the project owns from then on.

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
