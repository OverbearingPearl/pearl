# ci-rust

A check suite for Rust workspaces, shared between projects as a git subtree.
It is the Rust sibling of [ci-elisp](https://github.com/OverbearingPearl/ci-elisp):
the same ownership model, the same generated check table, the same
report-versus-gate contract, the same commit-message rules. What changes is
the tooling underneath — `cargo`, `clippy`, `rustfmt`, `cargo-deny`,
`cargo-audit`, `cargo-llvm-cov`, criterion — and a small Rust helper,
`ci-tools`, that does the parsing the shell should not.

## The one idea

A project does not fork this suite. It pulls it into `ci/` as a subtree and
runs `ci/bootstrap.sh` once. After that:

- **`ci/` is shared.** A `git subtree pull` replaces it wholesale. Nothing of
  the project's lives in it.
- **Everything bootstrap writes is the project's.** The `Makefile`,
  `config/ci.toml`, `.gitignore`, `.pi/ai-git-commit.json`, `.github/` and the
  hook config are written once and never overwritten by a pull. `--force`
  replaces only the hook config and `.github/`, and refuses to touch a file
  with uncommitted changes.

So a project keeps its own targets in `config/targets.mk` (which the template
`Makefile` includes), its own policy in `config/ci.toml` (deep-merged over
`ci/config.toml`), and its own prose commit rules in `.pi/ai-git-commit.json`.
A pull can bring new checks and new templates; it cannot take any of that away.

## Quick start

```sh
git subtree add --prefix=ci https://github.com/OverbearingPearl/ci-rust.git main --squash
ci/bootstrap.sh
$EDITOR config/ci.toml      # optional; nothing has to be set
make lint
```

To take a later version:

```sh
git subtree pull --prefix=ci https://github.com/OverbearingPearl/ci-rust.git main --squash
ci/bootstrap.sh             # reports template drift; replaces nothing it owns
```

## The targets

| target | what it does | gate? |
| --- | --- | --- |
| `make lint` | run every check over the files its class admits | yes |
| `make test` | `cargo test` (or `cargo nextest`) across the workspace | yes |
| `make perf` | run criterion benches, print the timing table | no |
| `make perf-check` | the same measurement, failing on a `[perf]` budget | yes (nightly) |
| `make size` | significant lines, large modules, copy-paste runs | no |
| `make coverage` | `cargo llvm-cov` summary | no |
| `make dist` | build release binaries / package crates into `dist/` | no |
| `make release-check` | the git tag must match the declared crate version | yes |
| `make hooks` | install the pre-commit, commit-msg and pre-push hooks | — |

`lint` and `test` are the gates a commit and a push must pass. `perf-check` is
the strict form, run nightly where a stray red mark costs nobody a merge.

### One platform, many jobs

A suite of shell stories can take twenty minutes on Windows and two on Linux,
because starting a process there costs seconds rather than milliseconds. The
workflow therefore runs Windows as a job per workspace member: `test-shards.sh`
reads the members out of `cargo metadata` and prints the matrix, so a crate
added tomorrow is a shard tomorrow with nothing edited by hand.
`CI_TEST_SHARD_SIZE` groups that many members into one job when a workspace of
small crates would pay more in cold builds than it gains in parallelism.

A shard names its packages in `CI_TEST_PACKAGES`; empty means the whole
workspace, which is what `make test` does on a developer's machine. The plain
`cargo test` path also passes `--no-fail-fast`: on a slow platform a run that
stops at the first red suite hides the rest, and every hidden failure costs
another whole run to discover.
`size` and `coverage` are reports: a number is information, not a verdict, and
an empty report is a failure of the tool, not of the code.

## The checks

There is no committed list. The table is generated from the files in
`checks/`, sorted by name:

```sh
ci/run-checks.sh --list        # the check table and the class profiles
ci/run-checks.sh --files       # discovered files, by class
ci/run-checks.sh --explain crates/pearl-queue/src/lib.rs
```

Each check is an executable that answers `--meta` (name, scope, classes,
description) and, when run, emits TSV finding records. The orchestrator
collects them, prints `file:line: [check] message`, and gates on the ERROR
count. WARN records never fail a run. Under GitHub Actions the same records
become workflow annotations.

Three checks shell out to cargo and translate its JSON diagnostics
(`rustdoc`, `cargo-lint`, `build`); the other twenty read Rust, TOML and
Markdown through `ci-tools` — blank lines, header comments, comment width,
banned constructs, crate metadata, module and crate naming, dependency
direction, MSRV, edition, text hygiene, policy self-consistency, project
neutrality, doc links, ADR shape, layer order, and orphaned stubs.

## Classes and layers

A file's **class** says what it is — `main`, `business`, `test`, `tooling`,
`vendor` — decided by where it lives. A class's **profile** says what rules
reach it and what classes a crate of it may depend on. `vendor` is exempt
from style and may depend only on `vendor`; that one rule still reaches it.

A project with an architecture declares **layers** in `config/ci.toml`,
bottom-up. A crate may depend on its own layer and the ones before it. Every
business crate must appear in some layer or `layer-order` reports it — an
undeclared crate is one nobody placed on purpose.

## Configuration

`ci/config.toml` is the generic default; `config/ci.toml` is the project's
override, deep-merged on top. Keys: `msrv`, `edition`, `src-dir`, `test-dir`,
`tooling-dir`, `vendor-dir`, `comment-width`, `crate-prefix`, `main-crate`,
`[layers]`, `[profiles.*]`, `[[banned]]`, `[size]`, `[coverage]`, `[perf.*]`.
See `templates/ci.toml` for a commented starting point and `MANUAL.md` for
what each check wants.

## Commit messages

Two halves. The shape is mechanical and checked by `check-commit-message.sh`:
a capital imperative subject, no `feat:`-style prefix, no trailing period, 60
columns soft and 80 hard, body lines within 80, no forbidden line-openers, no
emoji. The prose is the project's, in `.pi/ai-git-commit.json`: narrative,
first person where natural, explain the why and the trade-offs, no marketing
words, no AI attribution. The default prose rules are Linus Torvalds' uEmacs
style.

## Layout

```
bootstrap.sh            write the project-owned files, once
run-checks.sh           the lint orchestrator and the generated table
run-tests.sh            the test gate
test-shards.sh          the per-member test matrix for a slow platform
run-perf.sh             criterion budgets (report and gate)
run-coverage.sh         coverage report
run-size.sh             size report
build-dist.sh           release artifacts
check-version-tag.sh    tag agrees with the crate version
check-commit-message.sh the mechanical commit rules
config-export.sh        resolved config as CI_* variables
deps.sh                 install the external cargo tools, build ci-tools
config.toml             generic default policy
lib/                    shell glue: util, project, report, registry, cargo
checks/                 the 23 checks, discovered by filename
tools/                  ci-tools, the Rust parsing helper
templates/              what bootstrap writes: Makefile, ci.toml, gitignore,
                        ai-git-commit.json, github/
MANUAL.md               what each check wants, and how to satisfy it
```

## License

MIT. See `LICENSE`.
