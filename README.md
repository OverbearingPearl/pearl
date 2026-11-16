# ci-elisp

A check suite and pipeline for Emacs Lisp packages. It is vendored into a
project as the directory `ci/` and never edited there: a fix lands here, the
projects pull it.

## Add it to a project

Once, in a project that has no `ci/` yet:

```sh
git subtree add --prefix=ci git@github.com:OverbearingPearl/ci-elisp.git main --squash \
  && ci/bootstrap.sh
```

After that, updating is a make target, because bootstrap writes one:

```sh
make update
```

It runs the pull and then `ci/bootstrap.sh`, which keeps every file that exists
and says whether a template moved.

Bootstrap writes the project's files, installs the three hooks when
`pre-commit` is present, and runs a first `make lint`, saying either that the
package is clean or which message is the first to fix. The line is two commands
joined, not a downloader: nothing is fetched but the subtree.

If the project already has a `Makefile` or a `.github/`, bootstrap keeps them
and says what is missing from them: the targets the hooks call, and the
workflows the templates would have provided. `--force` replaces a file, which
is worth reading twice for `.github/`: it deletes every workflow already
there.

`ci/bootstrap.sh --help` is the whole usage, and it ends with the four things
the project owns from then on.

## Update a project

`make update`, which is `git subtree pull` followed by `ci/bootstrap.sh`. The
squash commit names the upstream commit, which is the pin.

If the pull touched `.github/` and the project has customised nothing there:

```sh
ci/bootstrap.sh --github-only --force    # refresh .github/ and nothing else
```

Without the target — a project that has its own Makefile and kept it — the pull
is:

```sh
git subtree pull --prefix=ci git@github.com:OverbearingPearl/ci-elisp.git main --squash
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
