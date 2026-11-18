# The suite in detail

## Layout the policy assumes

```
<pkg>.el            the package entry point, the only .el at the repo root
lisp/<pkg>-*.el     business modules; the package prefix is required
test/*-tests.el     test modules; the package prefix is forbidden
vendor/**/*.el      vendored packages, not ours to restyle
```

The root directory holds exactly one `.el` file; anything else is a
violation with a clear message rather than a file that silently escapes
the checks.  The main file does not have to be named after the repository:
it is discovered, not assumed -- and its base name *is* the package name.

## Naming

Three strings have to agree, and the suite checks the two adjacent pairs
rather than the transitive one:

| string | example | the rule |
|---|---|---|
| package name | `pkg` | it is the main module's name, by definition |
| module name | `pkg-loop`, `loop-tests` | starts with `<pkg>-` under `lisp/`, and nowhere else |
| definition name | `pkg-loop-step`, `loop-tests-step` | carries the module's full name |

`module-prefix` answers the first pair, per class: required in `lisp/`
because those modules are published, forbidden in `test/` and `ci/` because
they are not, and repeating the package name in every test file name is
noise.  `definition-prefix` answers the second pair: the prefix is the
whole module name, not just the package name, so a definition says which
module a failure came from.  `test-suffix` adds the one suffix rule:
`test/` modules end in `-tests`.

Nothing compares a definition with the package name directly, and nothing
needs to: the package prefix is on the module name (`module-prefix`) and
the module name is on the definition (`definition-prefix`).

`vendor/` is exempt from all three.  It carries somebody else's package
prefix, and its file names and definitions are not ours to change.  The
tooling under `ci/` is exempt from `definition-prefix` for a related
reason: it is shared code that must not name the package at all
(`project-neutral`), so its definitions use the suite's own prefix.

A business module needs a name after the prefix: `lisp/<pkg>-.el` and
`lisp/<pkg>--name.el` both fail, because the module name would be empty or
start with a dash.  Inside any module, `defcustom`, `defvar`, `defun`,
macros, aliases and ERT tests follow the same rule, and a minor mode also
ends in `-mode`.

### The main module

The package name is read from the main file -- `p/package-name` is its base
name -- so the main module and the package are the same string and there
is nothing to compare.  The one naming exception sits here: the main
module may define an entry command named exactly `<pkg>`, because that is
what users type.

### Test modules

A test module is not published, so the package prefix is forbidden there;
its name is free in every other way, but its definitions are prefixed with
the module name like anywhere else (`loop-tests-...`).  The `-tests` suffix
is required, and it earns its place twice: MELPA's default file spec
excludes `*-tests.el`, so tests never reach a published tarball, and the
suffix marks a file as a test without opening it.

A test is also the only place where a name may appear before it exists. The
first step of a TDD cycle is a test that calls a function nobody has written
yet, and `banned-construct' forbids `declare-function', the other way to say
"this will exist at runtime", so the compiler has to be told not to mind.
Two checks relax for `test/`:

| relaxed for `test/` | why |
|---|---|
| `package-lint` | it models a published package; its prefix rule would ask for the package prefix |
| `byte-compile`: `lexical` | the "dynamic var lacks a prefix" warning is a convention for published code |
| `byte-compile`: `unresolved` | a test names the API before it is written; a mistyped name still fails when ERT runs it |

Everything else still applies: whitespace, heading comments, banned
constructs, byte-compilation, checkdoc, header rules, `provide` matching the
file name, dependency direction.

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
test      headers (:forbid  :package)   may require main business test vendor  commentary 1-3
tooling   headers (:forbid  :package)   may require tooling             commentary 1-
vendor    headers none                  may require vendor              commentary 0-
```

Reading a row answers "what is a test file" in one line.  And these two
views are generated, not maintained by hand:

```sh
ci/run-checks.sh --list                  # check x class matrix
ci/run-checks.sh --explain FILE          # one file: its class, its profile, every check
ci/run-checks.sh --files                 # every file the suite inspects, with its class
```

Both come from the registry and the policy, so neither can drift, and neither
is committed: a file that repeats what a command prints is a file that goes
stale on the one pull nobody re-ran it.  `policy` validates the tables
themselves: every class has a
profile with all three properties, every class named exists, every header
set resolves, and every class a check claims exists.  A misspelled class
would otherwise disable a rule silently.

## Decision records

A project that keeps architecture decision records gets two rules, and both
come from the `pi` extension that puts the record index into an agent's system
prompt: the directory is `docs/decisions`, and a record is
`NNNN-kebab-title.md` with four digits and no prefix.

Neither is a preference. The extension matches `^[0-9]{4}-` in that one
directory, so a record somewhere else, or a name with an `ADR-` prefix, is a
record it cannot see -- and a decision the agent cannot see vetoes nothing.
Both are reported, and `doc/adr`, `docs/adr`, `docs/adrs` and `adr` are named
as the shapes of the mistake. A project with no such directory has no records
and no rules.

What a record *says* is a separate question. Two content shapes are accepted,
because two are in the wild and both are readable:

- **MADR.** YAML front matter with at least `status` and a date, a title
  heading, then `Context and Problem Statement`, `Decision Outcome` and
  `Consequences` (the plain `Context`, `Decision` and `Considered Options`
  spellings pass too).
- **Nygard.** A numbered heading, a `Date: YYYY-MM-DD` line, `## Status`, then
  `## Context`, `## Decision` and `## Consequences`.

A file is asked to be one of them rather than a mixture. Headings may be level
one or two, since section names carry the meaning and templates disagree about
level. MADR constrains content only -- no directory, no file name -- so
accepting it costs the naming rule nothing.

`decisions.md` is the table of contents a recording tool writes, and `0000-*`
is a template; neither is checked. Numbers are unique across the directory and
`superseded/`, because the number is the reference a later record uses. A
record whose status says superseded belongs in `superseded/`, and one in
`superseded/` has to say `superseded` and nothing else. The two words are not
interchangeable: `amended by` means the record is still in force with a later
record growing it, so an amended record stays where the index reads it.

## Vendored code

A directory of third-party files is a class of its own, because the rules for
our source are wrong for somebody else's: the headers belong to its author,
the Commentary is not ours to shorten, and its file names carry its package
prefix rather than ours.  That last fact is why the naming checks skip the
class entirely.

| key | default | what it decides |
|---|---|---|
| `:vendor-dir` | `"vendor"` | where vendored packages live |

Vendored files are discovered, classified as `vendor`, and put on the load
path, so a test can `require` them exactly as it requires the package. No
per-file check runs on them: the checks that ask about our own source run on
`main`, `business`, `test` and `tooling`, and a check that wants vendor files
names the class itself. `text-hygiene` skips the directory too -- whitespace
in a copy is the copy's business, and fixing it would put the file at odds
with the release it came from. The profile exists so the policy can answer
for the class -- no header is required or forbidden, the Commentary minimum
is zero, and a vendored module may only require another vendored one.

That last rule is the one rule that reaches vendor, and it is the useful one:
it keeps a vendored package from quietly depending on the host's modules,
which is how a copy stops being replaceable.

## Module layers

An architecture with layers has one rule a `require` can express, and the
class profiles cannot: every module in `lisp/` is class `business`, so nothing
in `:profiles` stops one business module from requiring another that sits
above it.

| key | default | what it decides |
|---|---|---|
| `:layers` | nil | layer name -> the module names in it |
| `:layer-may-require` | nil | layer name -> the layers it may require |

The module name is the file name without the package prefix (`module-prefix`
already guarantees that shape), so `loop` answers for `lisp/<pkg>-loop.el`,
and `:layer-may-require` does not have to
list the layer itself: a layer may always require its own.

```elisp
(setq p/config
      (plist-put p/config :layers
                 '((core contract loop session)
                   (adapters provider-sse session-jsonl)
                   (product console tools undo))))
(setq p/config
      (plist-put p/config :layer-may-require
                 '((core) (adapters core) (product core adapters))))
```

A business module that is missing from `:layers` is reported rather than
exempted, because an undeclared module is the one a reviewer forgets to
place.  Nil disables the check.

## Layers


```
ci/                  what "correct" means          (runs anywhere, no GitHub)
ci/config.el         the only file with policy
ci/lib/              shared plumbing (no policy)
ci/checks/NN-*.el    one check per file, name = check name
ci/perf.el           the performance benchmarks (budgets live in config.el)
ci/run-*.sh          the entry points make calls: checks, tests, perf

.github/actions/     one composite action per stage
.github/workflows/   ci, melpa, perf and release: stage orchestration only
```

A check is added by creating `ci/checks/NN-<name>.el` and calling
`p/defcheck` (per file) or `p/defcheck-global` (whole repository). There is
no central list to update: files are loaded in name order, and
`ci/run-checks.sh --list` prints what was registered, which is the table a
README would otherwise duplicate and get wrong.

## When what runs

Placement is chosen by what a failure means, not by how important the check is.
A gate that can fail for reasons the author cannot control teaches people to
reach for `--no-verify`, and that one habit costs more than any check it skips.

| gate | pre-commit | pre-push | CI on push and pull request | nightly | on demand |
|---|---|---|---|---|---|
| `make lint` | yes | | yes | | |
| `make test` | | yes | yes | | |
| `make perf` (strict) | | | | yes | yes |
| `make perf-report` | | | yes, uploaded as an artifact | | |
| melpazoid | | | | | yes |
| `make dist`, version tag | | | | | tag |

- **Fast, deterministic and offline goes into the commit gate.** `make lint`
  is under a second once the dependencies are cached, so it runs before every
  commit. The test suite sits at pre-push rather than pre-commit on purpose: it
  is a second today and will not be once it starts real processes, and a slow
  gate is a skipped gate.
- **Timing never gates a commit and never fails a pull request.** The same
  budget moves by an order of magnitude between a quiet machine and a busy one.
  The nightly run enforces it, where a failure is a mail rather than a blocked
  merge; pull requests get the table as an artifact, which is worth reading and
  not worth arguing with.
- **Anything that needs the outside world is on demand.** melpazoid builds a
  container that compiles Emacs from source; it belongs before a release or a
  MELPA pull request, not on every push. Trigger it with *Run workflow* on CI
  and tick `melpazoid`, or `gh workflow run ci.yml -f melpazoid=true`.

Two stages run on a schedule rather than on an event: the nightly perf run,
which fails into the same mail as everything else, and the weekly
`melpa-stats`, which stays quiet until the package is published. A tag runs
lint, the version check, `make dist` and the release.

## Emacs versions

CI runs the Emacs that `ubuntu-latest` ships, the same way melpazoid's own
workflow does. The floor is the Emacs the *suite* runs on, `:emacs-versions` in
`ci/config.el`, and it is a property of the suite rather than a knob: a project
has nothing to set. Two checks read it.

| check | question it answers |
|---|---|
| `emacs-version-minimum` | is the running Emacs at or above the floor? |
| `package-requires-emacs` | does the package promise at least that Emacs? |

The second one is the interesting one. A package that declares `((emacs "27.1"))`
is promising support for a version the suite has never linted and the project
has probably never run. The check reports the package's number against the
suite's, and the fix is either to raise the header or to say why the claim is
real.

A project that genuinely needs a different floor changes it in `ci/config.el`,
which is a deliberate departure from the shared suite rather than a setting.

## melpazoid, zero tolerance

`WARN_IS_ERROR=1` turns melpazoid's `:Warning:` lines into failures, and the
`with [melpazoid]` section is printed only when melpazoid has suggestions and
never changes the exit code, so it is gated with a log assertion:

| reported as | caught by |
|---|---|
| `:Error:` / `: error:` | exit code |
| `:Warning:` / `: warning:` | exit code, only with `WARN_IS_ERROR=1` |
| `━ ... with [melpazoid]` suggestions | `grep -qF 'with [melpazoid]'` |

`EXIST_OK` enables the package-name collision check, which asks whether the
name is still free.  That question expires: the day the recipe is merged the
name is taken, by this package, and the check starts reporting a clash with
itself.  So the action answers it instead of a human — `exist-ok: auto`, the
default, looks for `recipes/<pkg>` in `melpa/melpa` and skips the check only
when the 404 disappears.  A probe that fails for any reason other than 404
is an error rather than a quiet `false`, because guessing "not published"
from a timeout would turn the check back on without anyone noticing.  The
recipe and the name both come from `ci/config-export.sh package`, so no
package name appears in the workflow.

### Why it runs on demand, not on every push

Everything in this file so far costs seconds, which is why every push runs
it. melpazoid does not, and the reason is one line of its Dockerfile:

```dockerfile
RUN ... curl .../emacs-31.1.tar.gz | tar -xz && ./configure ... && make -j2 install
```

It compiles Emacs from source inside the image. On a two-core runner that is
the whole job — tens of minutes, reported by docker as a single line — and
the container then runs checkdoc, package-lint and byte-compile, which are
the first three checks of `make lint`. For a package with no MELPA
dependencies, which is this one, the container adds a second Emacs version
and little else.

What melpazoid adds that the suite cannot is MELPA's own review: the recipe
built by package-build, the checklist, the license check, the package name.
That is worth reading before a MELPA pull request or a release, not on
every push. So the stage is manual:

```
Actions -> CI -> Run workflow -> melpazoid (checkbox)
```

When it was blocking on every event it never actually ran to completion:
the CI concurrency group cancels the previous run on the next push, so any
commit inside the compile killed it and nothing was ever reported. A
manual run is slower than a cached one but it finishes, and it costs
nothing on the pushes you do not ask about.

Three things make that half hour legible. `DOCKER_OUTPUT='--quiet'` is
rewritten to `--progress=plain`, so docker reports one line per step rather
than one for the build. melpazoid's own notes are unbuffered
(`PYTHONUNBUFFERED=1`) and flow through `sed -u`, because a plain `sed`
block-buffers a pipe and the log looks frozen. And the step ends with how
long the run took. The compile step itself is silent for the bulk of the
wait; the step timer is the only sign of life, which is why this is a run
you start deliberately.

There is no image cache. `docker save`/`docker load` restores the image, not
BuildKit's build cache, so the next `docker build` recompiles everything
anyway, and the tarball would cost hundreds of megabytes of the cache
budget that `.cache/elpa` already uses. The patch that would make the image
cheap — replacing the Emacs build with `apt-get install emacs-nox` — is ten
lines and works, but it makes CI responsible for someone else's Dockerfile
and lints with a different Emacs than upstream intends. Half an hour of
manual runs is the cheaper mistake.

## melpa-stats, weekly and quiet

Once MELPA builds the package, `.github/workflows/melpa.yml` mails the
download count every Sunday.  Before that it does nothing visible: the probe
in `.github/actions/melpa-status` prints `::notice::` and the mail step is
skipped.  A job that reports "not published yet" as a failure would be red
for weeks, and a job that is red for weeks is a job everyone learns to
ignore.

The status is read from `recipes/<pkg>` in `melpa/melpa`, not from
`melpa.org/download_counts.json`.  The two answer different questions: the
recipe appears when the pull request is merged, while the download counts
only know about packages someone has installed — a package published an hour
ago is missing from that file, which is not the same as unpublished.  The
counts are fetched separately, and a published package with no downloads yet
is reported as exactly that, not as zero.

Only `404` means "not yet".  Network failures fail the run and reach
`notify-failure` like every other stage; otherwise a broken probe looks
exactly like an unpublished package.

## Notification mail

Both mails carry two parts: a plain-text body with the same facts, for
terminals and mailboxes that are grepped rather than read, and an HTML card
for clients that render it.  The two cards are the same skeleton — rounded
card, coloured header, white body, grey footer — and the header colour is
the message: red for a failure, blue for the weekly count.  That is the
one thing worth scanning a subject line for.

No images and one colour each, so a client that strips styles down to text
loses nothing.  Logs are deliberately kept out of the HTML: they are in the
text part and in `failure-report.txt`, and pasting arbitrary log lines into
a document means either escaping them or occasionally sending mail that
renders as garbage from the first `<` onward.

Every fact in either card is a link.  A failure mail opens the run, the
branch and the commit; the count mail opens the package's MELPA page, the
recipe that publishes it and the repository.  The test is that nothing in
the inbox needs a search afterwards -- a notification earns its place by
saving the trip, not by reporting the news.

Nothing project-specific is written into `.github/`.  The package name comes
from `ci/config-export.sh package`; the repository, ref, commit, job, run
number and workflow name come from the `github` context; MELPA's URL layout
is written down once in `melpa-status`, which hands the page and recipe URLs
out as outputs.  That is what makes the directory copyable -- drop it into
another Emacs Lisp package and the mails name that package with no edit.

Cards live where their call sites are.  The failure card is inside an action
because six jobs send it, and the weekly card is inside the workflow
because exactly one job does.  A renderer shared between them would be one
more layer for twenty lines of inline CSS, so the skeleton is duplicated and
this paragraph is the reason it is allowed to be.  A third card is the
signal to build the renderer instead.

## pre-commit is local-only, on purpose

`.pre-commit-config.yaml` declares three hooks, all local: `make lint` at the
commit stage, `ci/check-commit-message.sh` at the commit-msg stage and
`make test` at the push stage. There is no remote hook repository, so
`pre-commit run --all-files` needs no network, builds no virtualenv and starts
instantly. That is the point of a local gate: it has to work on a plane.

Both stages have to be installed, or the second one silently never runs:

```sh
make hooks          # installs all three
```

or, by hand: `pre-commit install`, then `--hook-type commit-msg` and
`--hook-type pre-push`.

`pre-commit run --all-files` runs the commit stage only; add
`--hook-stage pre-push` to exercise the other one without pushing.

The message hook enforces a fixed shape, because the alternative was worse:
once the hook reads its rules from configuration, changing one line means
tracing a plist through an export script into shell. The shape is the one
`ci/check-commit-message.sh` writes down -- capital subject, no type prefix,
80/72 columns -- and the prose rules stay in the project's own file, here
`.pi/ai-git-commit.json`, which no script reads.

So a project whose messages follow different rules does one of two things,
both of which the ownership rule above makes safe. It defines its own hooks,
in which case bootstrap never touches them; or it edits the
`.pre-commit-config.yaml` bootstrap wrote, which is a project file from then on
and is never rewritten.

Who owns the hooks decides what bootstrap does, and it asks two questions:

| the project already has | bootstrap does |
|---|---|
| `pre-commit` not on PATH | writes the config, installs nothing, says which command to run |
| `.pre-commit-config.yaml` | keeps it, installs nothing, points at `make hooks` |
| hooks in `.git/hooks` but no config | keeps them, writes no config, installs nothing |
| neither | writes the config, installs all three hooks with `make hooks` |

`--force` answers "take ci's hooks" for the two middle rows. It does not make
`pre-commit` appear; without the tool the config is a file that nothing runs.

The script itself enforces only what a script can see: a capitalised subject
with no type prefix and no trailing period, 80/72 columns, no marketing words,
no "Generated with" and no co-author trailer. It cannot see whether the change
is one change or whether the body explains why, which is where the rules spend
most of their words.

The hygiene checks that usually come from `pre-commit-hooks` are part of the
suite instead, so local and CI enforce the same code:

| used to be | now |
|---|---|
| `trailing-whitespace` | `text-hygiene`, for every tracked text file outside `vendor/` |
| `end-of-file-fixer` | `text-hygiene` (exactly one final newline, same files) |
| `check-yaml` | not in the gate; see below |

YAML syntax is not gated locally: Emacs has no YAML parser, and adding Ruby
or PyYAML would reintroduce exactly the dependency this section removes.
GitHub validates workflow and action files when it loads them, and a broken
one is loud. If you want it locally anyway, run it yourself:

```sh
ruby -ryaml -e 'Dir[".github/**/*.yml"].each { |f| YAML.load_file(f, aliases: true) }'
```

## Performance budgets

`make perf` measures the handful of substrate numbers this package's design is
budgeted against, and fails when one goes over. The benchmarks are in
`ci/perf.el`; the numbers are in the `:perf` table of `ci/config.el`, where
every other policy number lives. `make perf-report` prints the same table and
never fails, for a machine that is not quiet.

| benchmark | budget | why it is guarded |
|---|---|---|
| `insert-per-chunk` | max 0.10 ms | an absolute sanity floor for a 50-byte insert |
| `insert-size-ratio` | max 3.0 | insert in a 20 000-line buffer over an empty one; an accidental walk shows up here first |
| `redisplay-per-chunk` | max 4.00 ms | the only expensive thing a stream does |
| `batch-ratio` | min 4.0 | how much cheaper a redisplay every 20 chunks is; a ratio survives a slow machine |
| `filter-events-per-call` | max 2.0 | above one means the process layer coalesces deliveries |
| `filter-gap-spread` | max 25.0 ms | widest minus narrowest delivery gap; a stall shows up here |
| `json-per-event` | max 0.10 ms | parsing one streamed event |

Two of them need a terminal, because `redisplay` paints nothing in batch mode.
`ci/run-perf.sh` hands Emacs a terminal through `script` when it can, spelling
out the BSD and GNU argument orders; without it those two report `skip` rather
than fail. That is deliberate: a budget that cannot be measured is not a
failure, and pretending otherwise teaches people to ignore the output.

A package's own hot path is measured in `config/perf.el`, which `ci/perf.el`
loads when it exists: register one there with `p/defperf` and add its budget to
the `:perf` table in `config/ci.el`. A benchmark without a budget is reported as
`no budget` rather than silently passing. Nothing is written for you, because
an empty file is a worse reminder than the sentence in `config/ci.el`.

The numbers are guarded while there is no code to guard, on purpose. They are
the constants the design rests on, so an Emacs upgrade or a hostile
configuration that changes them is worth learning about before the first
benchmark of our own exists.

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

When `SMTP_HOST` is unset (for example on a fork pull request) the mail is
skipped silently.  That gate lives in `.github/actions/send-mail`, which both
the failure report and the weekly `melpa-stats` report go through, because a
step's `if:` cannot read `secrets`: putting the check in one action beats
turning an empty secret into an output at every call site.

## Shared actions

Five things are used by more than one job and are defined once:

| action | why it exists |
|---|---|
| `setup-emacs` | the guarded apt install and the version line were copied into several actions and drifting |
| `send-mail` | the only place that knows an empty `SMTP_HOST` means silence |
| `melpa-status` | the only place that knows what "published" means |
| `notify-failure` | builds the report, reads the run's identity from the `github` context, hands it to `send-mail` |
| `perf` | the pull request run and the nightly run execute the same thing, differing only in whether a budget that is over fails |

`notify-failure` takes `stage` and the SMTP inputs and nothing else.  The
repository, ref, commit and run URL come from `github`, which a composite
action can read — passing them in only gave six call sites a chance to
disagree with the run they were reporting on.

What is deliberately not shared: the six `if: failure()` blocks stay at each
job, because the report attaches the `build/*.log` files of *that* runner.  A
single `needs:`-based notify job cannot see another job's filesystem, so
merging them would cost the logs, which are the useful part.
