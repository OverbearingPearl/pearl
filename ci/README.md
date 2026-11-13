# Checks and pipeline

Everything here is project-agnostic. Copy `ci/`, `.github/`, `Makefile` and
`.pre-commit-config.yaml` into another Emacs Lisp package and it works; the
only file with project policy is `ci/config.el`.

## Layout the policy assumes

```
<pkg>.el           the package entry point, the only .el at the repo root
lisp/*.el          business modules
test/*-tests.el    test modules
```

The root directory holds exactly one `.el` file; anything else is a
violation with a explicit message rather than a file that silently escapes
the checks.  The main file does not have to be named after the repository:
it is discovered, not assumed.

### Test modules

Test modules are the only place where naming is free: neither the file name
nor the definitions inside it need a `<pkg>-` prefix. Three checks relax for
them, all for the same reason — a test module is not published:

| relaxed for `test/` | why |
|---|---|
| `module-prefix` | namespacing exists to avoid clashes between published modules |
| `package-lint` | it models a published package; its prefix rule would reject plain test names |
| `byte-compile`, `lexical` warnings | the "dynamic var lacks a prefix" warning is the same convention |

Everything else still applies: whitespace, heading comments, banned
constructs, byte-compilation, checkdoc, header rules, `provide` matching the
file name, dependency direction.

A `-tests` suffix is still recommended, and the reason is mechanical rather
than stylistic: every module must `(provide)` a feature named after its own
file, so `test/<name>.el` would provide the same feature as
`lisp/<name>.el` and the two would shadow each other on `load-path`. The
suffix keeps the feature distinct. Emacs itself does the same
(`test/lisp/foo-tests.el` against `lisp/foo.el`), and MELPA's default file
spec excludes `*-tests.el`, so tests never reach a published package.

### One header list, three views

`ci/config.el` spells the package headers once:

```elisp
:package-headers ("Copyright" "Author" "Assisted-by" "URL" "Version"
                  "SPDX-License-Identifier" "Package-Requires" "Keywords")
:header-sets ((:package    :all)
              (:submodule  (:but "Version" "Package-Requires" "Keywords"))
              (:entry-only (:only "Version" "Package-Requires" "Keywords")))
```

`:package` is the whole list, `:submodule` is that list minus the headers
only an entry point declares, and `:entry-only` is exactly those three.  The
profiles then say:

| class | rule |
|---|---|
| `main` | `(:require :package)` |
| `business` | `(:require :submodule :forbid :entry-only)` |
| `test`, `tooling` | `(:forbid :package)` |

So a submodule cannot declare a version, its own dependencies or keywords
(`Package-Requires` outside the main file has no effect anyway), and a file
that is not the package carries no package header at all.

### Commentary length depends on the class

The `:commentary` slot of each class profile sets `:min` and `:max`
descriptive lines: 2 or more for published modules, 1 to 3 for tests, 1 or
more for tooling.  Published modules have to carry their weight; a test
file says what it exercises in a line or two, and a long Commentary on a
test module is itself the smell.

## Scope

Two tables, one axis each, and nothing else:

| axis | where | answers |
|---|---|---|
| per check | `:classes (...)` in the check's own file | which kinds of file this rule applies to |
| per class | `:profiles` in `ci/config.el` | what a file of that kind must be: headers, allowed requires, Commentary length |

A file's class comes from its directory (`main` = the single `.el` at the
root, `business` = `lisp/`, `test` = `test/`, `tooling` = `ci/`), decided
by `p/module-class` in `ci/lib/project.el`.  The join is one `memq` inside
`p/defcheck` in `ci/lib/registry.el`, and that is the only place class
dispatch happens.

The current profiles:

```
main      headers (:require :package)                          may require business            commentary 2-
business  headers (:require :submodule, :forbid :entry-only)   may require business            commentary 2-
test      headers (:forbid  :package)   may require main business test  commentary 1-3
tooling   headers (:forbid  :package)   may require tooling             commentary 1-
```

Reading a row answers "what is a test file" in one line.  And these two
views are generated, not maintained by hand:

```sh
ci/run-checks.sh --list                  # check x class matrix
ci/run-checks.sh --explain FILE          # one file: its class, its profile, every check
ci/run-checks.sh --files                 # every file the suite inspects, with its class
```

`ci/CHECKS.md` commits both tables, and `make lint` fails if it is stale.
A third check, `policy`, validates the tables themselves: every class has a
profile with all three properties, every class named exists, every header
set resolves, and every class a check claims exists.  A misspelled class
would otherwise disable a rule silently.

## Layers

```
ci/                  what "correct" means          (runs anywhere, no GitHub)
ci/config.el         the only file with policy
ci/lib/              shared plumbing (no policy)
ci/checks/NN-*.el    one check per file, name = check name
ci/CHECKS.md         generated from ci/checks/, verified in CI

.github/actions/     one composite action per stage
.github/workflows/   ci.yml and release.yml: stage orchestration only
```

A check is added by creating `ci/checks/NN-<name>.el` and calling
`p/defcheck` (per file) or `p/defcheck-global` (whole repository). There is
no central list to update: files are loaded in name order, and
`make checks-doc` regenerates `ci/CHECKS.md`.

## When what runs

```
local (seconds)     commit -> pre-commit -> make lint
                                              |
CI (minutes)        PR / main -> lint          (same make lint + docs freshness)
                    PR / main -> test          ERT over test/*.el
                    PR / main -> melpazoid     develop, zero tolerance
                                              |
tag v*.*.*          release -> lint -> tag==Version -> make dist -> GitHub Release
                                              |
any failure         notify-failure -> email with the full log attached
```

Only melpazoid is exclusive to CI; `make lint` covers everything else,
including the `ci/CHECKS.md` freshness assertion.

## Emacs versions

CI runs the Emacs that `ubuntu-latest` ships, the same way melpazoid's own
workflow does.  The version the package claims lives in one place,
`:emacs-versions` in `ci/config.el`, and two checks keep it honest:

| check | question it answers |
|---|---|
| `package-requires-emacs` | does the main file declare that version or newer? |
| `emacs-version-minimum` | is the Emacs actually running the suite that version or newer? |

So a runner that is too old fails with a precise message instead of quietly
testing a different version than the package advertises.

## melpazoid, zero tolerance

`WARN_IS_ERROR=1` turns melpazoid's `:Warning:` lines into failures, and the
`with [melpazoid]` section is printed only when melpazoid has suggestions and
never changes the exit code, so it is gated with a log assertion:

| reported as | caught by |
|---|---|
| `:Error:` / `: error:` | exit code |
| `:Warning:` / `: warning:` | exit code, only with `WARN_IS_ERROR=1` |
| `━ ... with [melpazoid]` suggestions | `grep -qF 'with [melpazoid]'` |

`EXIST_OK` stays `false` until the package is on MELPA: it enables the
package-name collision check.  The recipe is built from
`ci/config-export.sh package`, so no package name appears in the workflow.

## pre-commit is local-only, on purpose

`.pre-commit-config.yaml` declares exactly one hook, and it is local:
`make lint`. There is no remote hook repository, so `pre-commit run
--all-files` needs no network, builds no virtualenv and starts instantly.
That is the point of a pre-commit gate: it has to work on a plane.

The hygiene checks that usually come from `pre-commit-hooks` are part of the
suite instead, so local and CI enforce the same code:

| used to be | now |
|---|---|
| `trailing-whitespace` | `text-hygiene`, for every tracked text file |
| `end-of-file-fixer` | `text-hygiene` (exactly one final newline) |
| `check-yaml` | not in the gate; see below |

YAML syntax is not gated locally: Emacs has no YAML parser, and adding Ruby
or PyYAML would reintroduce exactly the dependency this section removes.
GitHub validates workflow and action files when it loads them, and a broken
one is loud. If you want it locally anyway, run it yourself:

```sh
ruby -ryaml -e 'Dir[".github/**/*.yml"].each { |f| YAML.load_file(f, aliases: true) }'
```

## Requirements

- `emacs` on `PATH`, or `EMACS=/path/to/emacs`.
- Network on first run only: `ci/deps.el` installs `package-lint` into
  `.cache/elpa` unless it is already available in `~/.emacs.d/elpa`.

## Secrets (notifications)

| secret | meaning |
|---|---|
| `SMTP_HOST`, `SMTP_PORT` | SMTP server (port 465, implicit TLS) |
| `SMTP_USER`, `SMTP_PASS` | SMTP credentials; `SMTP_USER` is also the sender |
| `NOTIFY_EMAIL` | recipient |

When `SMTP_HOST` is unset (for example on a fork pull request) the
notification step is skipped silently.

## Copying into another package

Copy these four things and you are done:

```
ci/  .github/  Makefile  .pre-commit-config.yaml
```

Then, in the target repository:

1. Add `.cache/` and `build/` to `.gitignore` (the suite creates both).
2. Keep the layout the policy describes: one `.el` at the root, business
   modules in `lisp/`, tests in `test/`.  Other layouts only need
   `:submodule-dir` / `:test-dir` in `ci/config.el` changed.
3. If the package is already on MELPA, pass `exist-ok: true` to the
   melpazoid action in `.github/workflows/ci.yml`.
4. Add the five SMTP secrets, or leave them out and the failure mail is
   skipped.

Policy knobs, all in `ci/config.el`: `:emacs-versions`, `:submodule-dir`,
`:test-dir`, `:tooling-dir`, `:profiles`, `:header-sets`, `:banned`,
`:def-kinds`, `:commentary-width`.

The first run needs network once, to install `package-lint` into
`.cache/elpa`; after that everything is offline.
